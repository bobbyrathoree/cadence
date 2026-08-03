#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    #[cfg(feature = "test-faults")]
    PanicInTool,
    FailWriteScopeClose,
    #[cfg(feature = "test-faults")]
    CommitBusyOnce,
    #[cfg(feature = "test-faults")]
    BeginBusyAlways,
}

#[cfg(feature = "test-faults")]
pub(crate) fn active() -> Option<Fault> {
    match std::env::var("CADENCE_MCP_FAULT").ok().as_deref() {
        Some("panic_in_tool") => Some(Fault::PanicInTool),
        Some("fail_write_scope_close") => Some(Fault::FailWriteScopeClose),
        Some("commit_busy_once") => Some(Fault::CommitBusyOnce),
        Some("begin_busy_always") => Some(Fault::BeginBusyAlways),
        _ => None,
    }
}

#[cfg(not(feature = "test-faults"))]
pub(crate) fn active() -> Option<Fault> {
    None
}

pub(crate) fn panic_in_tool() -> bool {
    #[cfg(feature = "test-faults")]
    {
        active() == Some(Fault::PanicInTool)
    }
    #[cfg(not(feature = "test-faults"))]
    {
        false
    }
}

#[cfg(feature = "test-faults")]
struct CommitBusyOnce {
    failed: Cell<bool>,
}

#[cfg(feature = "test-faults")]
impl TxDriver for CommitBusyOnce {
    fn begin_immediate(&self, connection: &Connection) -> rusqlite::Result<()> {
        RealDriver.begin_immediate(connection)
    }

    fn commit(&self, connection: &Connection) -> rusqlite::Result<()> {
        if self.failed.replace(true) {
            RealDriver.commit(connection)
        } else {
            Err(busy_error())
        }
    }

    fn rollback(&self, connection: &Connection) -> rusqlite::Result<()> {
        RealDriver.rollback(connection)
    }
}

#[cfg(feature = "test-faults")]
struct BeginBusyAlways;

#[cfg(feature = "test-faults")]
impl TxDriver for BeginBusyAlways {
    fn begin_immediate(&self, _connection: &Connection) -> rusqlite::Result<()> {
        Err(busy_error())
    }

    fn commit(&self, _connection: &Connection) -> rusqlite::Result<()> {
        Err(rusqlite::Error::InvalidQuery)
    }

    fn rollback(&self, _connection: &Connection) -> rusqlite::Result<()> {
        Err(rusqlite::Error::InvalidQuery)
    }
}

#[cfg(feature = "test-faults")]
fn busy_error() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY), None)
}

#[cfg(feature = "test-faults")]
pub(crate) fn with_tx_driver<T>(
    operation: impl FnOnce(&dyn TxDriver, &RetryPolicy) -> AppResult<T>,
) -> AppResult<T> {
    let policy = RetryPolicy::production();
    match active() {
        Some(Fault::CommitBusyOnce) => operation(
            &CommitBusyOnce {
                failed: Cell::new(false),
            },
            &policy,
        ),
        Some(Fault::BeginBusyAlways) => operation(&BeginBusyAlways, &policy),
        _ => operation(&RealDriver, &policy),
    }
}
#[cfg(feature = "test-faults")]
use std::cell::Cell;

#[cfg(feature = "test-faults")]
use cadence_core::error::AppResult;
#[cfg(feature = "test-faults")]
use cadence_core::services::transaction::{RealDriver, RetryPolicy, TxDriver};
#[cfg(feature = "test-faults")]
use rusqlite::Connection;
