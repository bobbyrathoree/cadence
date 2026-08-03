#![cfg(feature = "test-support")]

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cadence_core::db::{Db, Health};
use cadence_core::db_access::DbAccess;
use cadence_core::error::AppError;
use cadence_core::services::transaction::UNRECOVERABLE_DB_MSG;
use rusqlite::Connection;

fn file_access(
    label: &str,
) -> (
    DbAccess,
    Connection,
    std::sync::mpsc::Receiver<String>,
    std::path::PathBuf,
) {
    let path = std::env::temp_dir().join(format!(
        "cadence-db-access-{label}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE items (value INTEGER NOT NULL);")
        .unwrap();
    let observer = Connection::open(&path).unwrap();
    let (health, receiver) = Health::recording();
    let access = DbAccess::new(Db { conn, health });
    (access, observer, receiver, path)
}

#[test]
fn poisoned_mutex_records_and_never_runs_the_closure() {
    let (access, observer, receiver, path) = file_access("mutex");
    let ran = Arc::new(AtomicBool::new(false));
    access.poison_mutex_for_test();
    let ran_in_closure = ran.clone();
    let result = access.with_sync(|db| {
        ran_in_closure.store(true, Ordering::SeqCst);
        db.conn.execute("INSERT INTO items VALUES (1)", [])?;
        Ok(())
    });

    assert_eq!(
        result,
        Err(AppError::Internal(UNRECOVERABLE_DB_MSG.to_string()))
    );
    assert!(!ran.load(Ordering::SeqCst));
    assert_eq!(receiver.recv().unwrap(), "mutex poisoned");
    assert_eq!(
        observer
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(observer);
    drop(access);
    fs::remove_file(path).unwrap();
}

#[test]
fn closure_panic_records_and_returns_the_sentinel() {
    let (access, observer, receiver, path) = file_access("panic");
    let result = access.with_sync::<()>(|_| panic!("injected closure panic"));
    assert_eq!(
        result,
        Err(AppError::Internal(UNRECOVERABLE_DB_MSG.to_string()))
    );
    assert_eq!(receiver.recv().unwrap(), "panic in db closure");
    drop(observer);
    drop(access);
    fs::remove_file(path).unwrap();
}

#[test]
fn sync_and_async_paths_have_matching_results() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (access, observer, _, path) = file_access("parity");
            let sync_value = access
                .with_sync(|db| {
                    db.conn.execute("INSERT INTO items VALUES (1)", [])?;
                    Ok(1_i64)
                })
                .unwrap();
            let async_value = access
                .with_async(|db| {
                    db.conn.execute("INSERT INTO items VALUES (2)", [])?;
                    Ok(2_i64)
                })
                .await
                .unwrap();

            assert_eq!((sync_value, async_value), (1, 2));
            assert_eq!(
                observer
                    .query_row("SELECT COUNT(*) FROM items", [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                2
            );
            drop(observer);
            drop(access);
            fs::remove_file(path).unwrap();
        });
}

#[test]
fn cancelled_join_is_internal_without_poisoning() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (access, observer, receiver, path) = file_access("cancel");
            let result = access.cancelled_join_for_test().await;
            assert!(matches!(result, Err(AppError::Internal(_))));
            assert!(receiver.try_recv().is_err());
            assert!(!access.with_sync(|db| Ok(db.health.is_poisoned())).unwrap());
            drop(observer);
            drop(access);
            fs::remove_file(path).unwrap();
        });
}
