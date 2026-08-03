#![cfg(feature = "test-support")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use cadence_core::db::{Db, Health};
use cadence_core::error::{AppError, AppResult};
use cadence_core::services::transaction::{
    immediate_with, RealDriver, RetryEvent, RetryPolicy, TxDriver, UNRECOVERABLE_DB_MSG,
};
use rusqlite::{Connection, Error};

fn test_db() -> (Db, std::sync::mpsc::Receiver<String>) {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE items (value TEXT NOT NULL UNIQUE);")
        .unwrap();
    let (health, receiver) = Health::recording();
    (Db { conn, health }, receiver)
}

fn sqlite_error(code: i32) -> Error {
    Error::SqliteFailure(rusqlite::ffi::Error::new(code), None)
}

fn policy(events: Rc<RefCell<Vec<RetryEvent>>>) -> RetryPolicy {
    RetryPolicy {
        delays: vec![Duration::ZERO, Duration::ZERO],
        observe: Box::new(move |event| events.borrow_mut().push(event)),
    }
}

struct BeginBusyOnce {
    calls: Cell<u32>,
}

impl TxDriver for BeginBusyOnce {
    fn begin_immediate(&self, connection: &Connection) -> rusqlite::Result<()> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if call == 1 {
            Err(sqlite_error(rusqlite::ffi::SQLITE_BUSY))
        } else {
            RealDriver.begin_immediate(connection)
        }
    }

    fn commit(&self, connection: &Connection) -> rusqlite::Result<()> {
        RealDriver.commit(connection)
    }

    fn rollback(&self, connection: &Connection) -> rusqlite::Result<()> {
        RealDriver.rollback(connection)
    }
}

struct BeginAlwaysBusy;

impl TxDriver for BeginAlwaysBusy {
    fn begin_immediate(&self, _: &Connection) -> rusqlite::Result<()> {
        Err(sqlite_error(rusqlite::ffi::SQLITE_BUSY))
    }

    fn commit(&self, _: &Connection) -> rusqlite::Result<()> {
        unreachable!()
    }

    fn rollback(&self, _: &Connection) -> rusqlite::Result<()> {
        unreachable!()
    }
}

struct CommitFault {
    commit_calls: Cell<u32>,
    rollback_calls: Cell<u32>,
    error_code: i32,
    rollback_fails: bool,
    once: bool,
}

impl TxDriver for CommitFault {
    fn begin_immediate(&self, connection: &Connection) -> rusqlite::Result<()> {
        RealDriver.begin_immediate(connection)
    }

    fn commit(&self, connection: &Connection) -> rusqlite::Result<()> {
        let call = self.commit_calls.get() + 1;
        self.commit_calls.set(call);
        if !self.once || call == 1 {
            Err(sqlite_error(self.error_code))
        } else {
            RealDriver.commit(connection)
        }
    }

    fn rollback(&self, connection: &Connection) -> rusqlite::Result<()> {
        self.rollback_calls.set(self.rollback_calls.get() + 1);
        if self.rollback_fails {
            Err(Error::InvalidQuery)
        } else {
            RealDriver.rollback(connection)
        }
    }
}

#[test]
fn begin_contention_succeeds_on_cycle_two() {
    let (mut db, _) = test_db();
    let events = Rc::new(RefCell::new(Vec::new()));
    immediate_with(
        &mut db,
        &BeginBusyOnce {
            calls: Cell::new(0),
        },
        &policy(events.clone()),
        |connection| {
            connection.execute("INSERT INTO items VALUES ('saved')", [])?;
            Ok(())
        },
    )
    .unwrap();

    let events = events.borrow();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, RetryEvent::Attempt(_)))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, RetryEvent::Sleep(_)))
            .count(),
        1
    );
}

#[test]
fn begin_contention_exhaustion_is_exact_conflict_and_writes_nothing() {
    let (mut db, _) = test_db();
    let events = Rc::new(RefCell::new(Vec::new()));
    let result = immediate_with(
        &mut db,
        &BeginAlwaysBusy,
        &policy(events.clone()),
        |connection| {
            connection.execute("INSERT INTO items VALUES ('never')", [])?;
            Ok(())
        },
    );

    assert_eq!(
        result,
        Err(AppError::Conflict(
            "database is busy; try again".to_string()
        ))
    );
    assert_eq!(
        db.conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(db.conn.is_autocommit());
    let events = events.borrow();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, RetryEvent::Attempt(_)))
            .count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, RetryEvent::Sleep(_)))
            .count(),
        2
    );
}

#[test]
fn commit_busy_rolls_back_retries_and_commits_exactly_one_row() {
    let (mut db, _) = test_db();
    let driver = CommitFault {
        commit_calls: Cell::new(0),
        rollback_calls: Cell::new(0),
        error_code: rusqlite::ffi::SQLITE_BUSY,
        rollback_fails: false,
        once: true,
    };
    immediate_with(
        &mut db,
        &driver,
        &policy(Rc::new(RefCell::new(Vec::new()))),
        |connection| {
            connection.execute("INSERT INTO items VALUES ('once')", [])?;
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(driver.rollback_calls.get(), 1);
    assert_eq!(
        db.conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn commit_busy_and_rollback_failure_poison_the_database() {
    assert_commit_double_fault(rusqlite::ffi::SQLITE_BUSY);
}

#[test]
fn non_busy_commit_failure_rolls_back_and_keeps_health_clean() {
    let (mut db, receiver) = test_db();
    let driver = CommitFault {
        commit_calls: Cell::new(0),
        rollback_calls: Cell::new(0),
        error_code: rusqlite::ffi::SQLITE_IOERR,
        rollback_fails: false,
        once: false,
    };
    let result = immediate_with(
        &mut db,
        &driver,
        &policy(Rc::new(RefCell::new(Vec::new()))),
        |_| Ok(()),
    );
    assert!(matches!(result, Err(AppError::Internal(_))));
    assert!(!db.health.is_poisoned());
    assert!(receiver.try_recv().is_err());
    assert!(db.conn.is_autocommit());
}

#[test]
fn non_busy_commit_and_rollback_failure_poison_the_database() {
    assert_commit_double_fault(rusqlite::ffi::SQLITE_IOERR);
}

fn assert_commit_double_fault(error_code: i32) {
    let (mut db, receiver) = test_db();
    let driver = CommitFault {
        commit_calls: Cell::new(0),
        rollback_calls: Cell::new(0),
        error_code,
        rollback_fails: true,
        once: false,
    };
    let result = immediate_with(
        &mut db,
        &driver,
        &policy(Rc::new(RefCell::new(Vec::new()))),
        |_| Ok(()),
    );
    assert_eq!(
        result,
        Err(AppError::Internal(UNRECOVERABLE_DB_MSG.to_string()))
    );
    assert!(db.health.is_poisoned());
    assert_eq!(receiver.recv().unwrap(), UNRECOVERABLE_DB_MSG);
}

#[test]
fn closure_error_rolls_back_after_one_cycle() {
    let (mut db, _) = test_db();
    let driver = CommitFault {
        commit_calls: Cell::new(0),
        rollback_calls: Cell::new(0),
        error_code: rusqlite::ffi::SQLITE_BUSY,
        rollback_fails: false,
        once: true,
    };
    let calls = Cell::new(0);
    let result: AppResult<()> = immediate_with(
        &mut db,
        &driver,
        &policy(Rc::new(RefCell::new(Vec::new()))),
        |connection| {
            calls.set(calls.get() + 1);
            connection.execute("INSERT INTO items VALUES ('rolled-back')", [])?;
            Err(AppError::invalid("stop"))
        },
    );
    assert_eq!(result, Err(AppError::Invalid("stop".to_string())));
    assert_eq!(calls.get(), 1);
    assert_eq!(driver.rollback_calls.get(), 1);
    assert_eq!(
        db.conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
