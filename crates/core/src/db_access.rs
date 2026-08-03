#![deny(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use crate::db::{Db, Health};
use crate::error::{AppError, AppResult};
use crate::services::transaction::UNRECOVERABLE_DB_MSG;

struct DbInner {
    db: Mutex<Db>,
}

/// Serialized access to one Cadence database connection.
#[cfg_attr(
    feature = "test-support",
    doc = r#"
The synchronization primitive is intentionally private:

```compile_fail
use cadence_core::db_access::DbAccess;
fn expose(access: DbAccess) {
    let _ = access.0;
}
```
"#
)]
#[derive(Clone)]
pub struct DbAccess(Arc<DbInner>);

impl DbAccess {
    pub fn new(db: Db) -> Self {
        Self(Arc::new(DbInner { db: Mutex::new(db) }))
    }

    pub fn with_sync<T>(&self, operation: impl FnOnce(&mut Db) -> AppResult<T>) -> AppResult<T> {
        let mut guard = match self.0.db.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let guard = poisoned.into_inner();
                let health = guard.health.clone();
                health.poison("mutex poisoned");
                drop(guard);
                return Err(AppError::internal(UNRECOVERABLE_DB_MSG));
            }
        };
        let health = guard.health.clone();

        match catch_unwind(AssertUnwindSafe(|| operation(&mut guard))) {
            Ok(result) => result,
            Err(_) => {
                health.poison("panic in db closure");
                Err(AppError::internal(UNRECOVERABLE_DB_MSG))
            }
        }
    }

    pub async fn with_async<T, F>(&self, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Db) -> AppResult<T> + Send + 'static,
    {
        let access = self.clone();
        let health_access = self.clone();
        let result = tokio::task::spawn_blocking(move || access.with_sync(operation)).await;
        health_access.resolve_join(result)
    }

    fn resolve_join<T>(
        &self,
        result: Result<AppResult<T>, tokio::task::JoinError>,
    ) -> AppResult<T> {
        match result {
            Ok(result) => result,
            Err(error) if error.is_panic() => {
                self.health().poison("infrastructure panic");
                Err(AppError::internal(UNRECOVERABLE_DB_MSG))
            }
            Err(error) => Err(AppError::internal(format!(
                "database task cancelled: {error}"
            ))),
        }
    }

    fn health(&self) -> Health {
        match self.0.db.lock() {
            Ok(guard) => guard.health.clone(),
            Err(poisoned) => poisoned.into_inner().health.clone(),
        }
    }

    #[cfg(feature = "test-support")]
    pub fn poison_mutex_for_test(&self) {
        let access = self.clone();
        let _ = std::thread::spawn(move || {
            let _guard = match access.0.db.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            panic!("test mutex poison");
        })
        .join();
    }

    #[cfg(feature = "test-support")]
    pub async fn cancelled_join_for_test(&self) -> AppResult<()> {
        let handle = tokio::spawn(async {
            std::future::pending::<()>().await;
            Ok(())
        });
        handle.abort();
        self.resolve_join(handle.await)
    }
}
