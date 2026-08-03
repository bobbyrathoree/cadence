use std::thread;
use std::time::Duration;

use rusqlite::{Connection, Error as SqliteError, ErrorCode};

use crate::db::Db;
use crate::error::{AppError, AppResult};

pub const UNRECOVERABLE_DB_MSG: &str = "the database connection is in an unrecoverable state";
const BUSY_MSG: &str = "database is busy; try again";

pub trait TxDriver {
    fn begin_immediate(&self, connection: &Connection) -> rusqlite::Result<()>;
    fn commit(&self, connection: &Connection) -> rusqlite::Result<()>;
    fn rollback(&self, connection: &Connection) -> rusqlite::Result<()>;
}

pub struct RealDriver;

impl TxDriver for RealDriver {
    fn begin_immediate(&self, connection: &Connection) -> rusqlite::Result<()> {
        connection.execute_batch("BEGIN IMMEDIATE")
    }

    fn commit(&self, connection: &Connection) -> rusqlite::Result<()> {
        connection.execute_batch("COMMIT")
    }

    fn rollback(&self, connection: &Connection) -> rusqlite::Result<()> {
        connection.execute_batch("ROLLBACK")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryEvent {
    Attempt(u32),
    Sleep(Duration),
}

pub struct RetryPolicy {
    pub delays: Vec<Duration>,
    pub observe: Box<dyn Fn(RetryEvent)>,
}

impl RetryPolicy {
    pub fn production() -> Self {
        Self {
            delays: vec![Duration::from_millis(50), Duration::from_millis(150)],
            observe: Box::new(|event| {
                if let RetryEvent::Sleep(delay) = event {
                    thread::sleep(delay);
                }
            }),
        }
    }
}

pub fn immediate<T>(
    db: &mut Db,
    operation: impl FnMut(&Connection) -> AppResult<T>,
) -> AppResult<T> {
    immediate_with(db, &RealDriver, &RetryPolicy::production(), operation)
}

pub fn immediate_with<T>(
    db: &mut Db,
    driver: &dyn TxDriver,
    policy: &RetryPolicy,
    mut operation: impl FnMut(&Connection) -> AppResult<T>,
) -> AppResult<T> {
    let total_attempts = policy.delays.len() + 1;

    for attempt_index in 0..total_attempts {
        (policy.observe)(RetryEvent::Attempt((attempt_index + 1) as u32));

        match driver.begin_immediate(&db.conn) {
            Ok(()) => {}
            Err(error) if is_busy_or_locked(&error) => {
                if retry(policy, attempt_index) {
                    continue;
                }
                return Err(AppError::Conflict(BUSY_MSG.to_string()));
            }
            Err(error) => return Err(AppError::from(error)),
        }

        let value = match operation(&db.conn) {
            Ok(value) => value,
            Err(error) => {
                rollback_or_poison(db, driver)?;
                return Err(error);
            }
        };

        match driver.commit(&db.conn) {
            Ok(()) => return Ok(value),
            Err(error) => {
                let retryable = is_busy_or_locked(&error);
                rollback_or_poison(db, driver)?;
                if retryable {
                    if retry(policy, attempt_index) {
                        continue;
                    }
                    return Err(AppError::Conflict(BUSY_MSG.to_string()));
                }
                return Err(AppError::internal(error.to_string()));
            }
        }
    }

    Err(AppError::Conflict(BUSY_MSG.to_string()))
}

fn retry(policy: &RetryPolicy, attempt_index: usize) -> bool {
    let Some(delay) = policy.delays.get(attempt_index).copied() else {
        return false;
    };
    (policy.observe)(RetryEvent::Sleep(delay));
    true
}

fn rollback_or_poison(db: &mut Db, driver: &dyn TxDriver) -> Result<(), AppError> {
    if driver.rollback(&db.conn).is_err() || !db.conn.is_autocommit() {
        db.health.poison(UNRECOVERABLE_DB_MSG);
        return Err(AppError::internal(UNRECOVERABLE_DB_MSG));
    }
    Ok(())
}

fn is_busy_or_locked(error: &SqliteError) -> bool {
    matches!(
        error,
        SqliteError::SqliteFailure(sqlite_error, _)
            if matches!(sqlite_error.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

pub(crate) fn expect_one(affected_rows: usize) -> AppResult<()> {
    if affected_rows == 0 {
        Err(AppError::NotFound)
    } else {
        Ok(())
    }
}
