use rusqlite::{Connection, TransactionBehavior};

use crate::error::{AppError, AppResult};

pub(crate) fn immediate<T>(
    conn: &mut Connection,
    operation: impl FnOnce(&Connection) -> AppResult<T>,
) -> AppResult<T> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AppError::from)?;
    let result = operation(&tx)?;
    tx.commit().map_err(AppError::from)?;
    Ok(result)
}

pub(crate) fn expect_one(affected_rows: usize) -> AppResult<()> {
    if affected_rows == 0 {
        Err(AppError::NotFound)
    } else {
        Ok(())
    }
}
