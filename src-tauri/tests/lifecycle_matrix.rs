#![cfg(feature = "test-support")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use cadence_core::db::{self, Db, Health};
use cadence_core::db_access::DbAccess;
use cadence_core::error::AppError;
use cadence_core::services::settings_service;
use cadence_lib::api::lifecycle::ApiLifecycle;
use rusqlite::Connection;

static LIFECYCLE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "cadence-lifecycle-matrix-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy)]
enum PeerCase {
    Missing,
    NeedsMigration,
    SchemaNewer,
    Io,
    Corrupt,
    Pragma,
}

impl PeerCase {
    fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::NeedsMigration => "migration",
            Self::SchemaNewer => "newer",
            Self::Io => "io",
            Self::Corrupt => "corrupt",
            Self::Pragma => "pragma",
        }
    }

    fn assert_error(self, error: AppError) {
        match self {
            Self::Missing => assert_eq!(
                error,
                AppError::Invalid("database file missing; restart Cadence".to_string())
            ),
            Self::NeedsMigration => assert_eq!(
                error,
                AppError::Invalid("database schema changed; restart Cadence".to_string())
            ),
            Self::SchemaNewer => assert_eq!(
                error,
                AppError::Invalid(
                    "database belongs to a newer Cadence; check your installations".to_string()
                )
            ),
            Self::Io | Self::Corrupt | Self::Pragma => {
                assert!(matches!(error, AppError::Internal(detail) if !detail.is_empty()))
            }
        }
    }
}

struct PeerFixture {
    _dir: TempDir,
    path: PathBuf,
    blocker: Option<Connection>,
}

impl PeerFixture {
    fn new(case: PeerCase) -> Self {
        let dir = TempDir::new(case.label());
        let path = dir.path.join("peer.db");
        let mut blocker = None;

        match case {
            PeerCase::Missing => {}
            PeerCase::NeedsMigration => create_database(&path, 0, true),
            PeerCase::SchemaNewer => create_database(&path, db::CURRENT_SCHEMA_VERSION + 1, true),
            PeerCase::Io => {
                fs::create_dir(&path).unwrap();
            }
            PeerCase::Corrupt => fs::write(&path, b"not a sqlite database").unwrap(),
            PeerCase::Pragma => {
                create_database(&path, db::CURRENT_SCHEMA_VERSION, false);
                let connection = Connection::open(&path).unwrap();
                connection.execute_batch("BEGIN EXCLUSIVE").unwrap();
                blocker = Some(connection);
            }
        }

        Self {
            _dir: dir,
            path,
            blocker,
        }
    }
}

impl Drop for PeerFixture {
    fn drop(&mut self) {
        if let Some(blocker) = self.blocker.take() {
            let _ = blocker.execute_batch("ROLLBACK");
        }
    }
}

fn create_database(path: &Path, version: i64, wal: bool) {
    let connection = Connection::open(path).unwrap();
    if wal {
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
    }
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();
}

fn main_access(dir: &Path) -> (DbAccess, Health, std::sync::mpsc::Receiver<String>) {
    let path = dir.join("main.db");
    let conn = db::connect(&path).unwrap();
    conn.pragma_update(None, "user_version", db::CURRENT_SCHEMA_VERSION)
        .unwrap();
    let (health, receiver) = Health::recording();
    (
        DbAccess::new(Db {
            conn,
            health: health.clone(),
        }),
        health,
        receiver,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_outcome_matrix_maps_and_compensates_without_poisoning() {
    let _guard = LIFECYCLE_TEST_LOCK.lock().await;
    let cases = [
        PeerCase::Missing,
        PeerCase::NeedsMigration,
        PeerCase::SchemaNewer,
        PeerCase::Io,
        PeerCase::Corrupt,
        PeerCase::Pragma,
    ];

    for case in cases {
        for startup in [false, true] {
            let settings_dir = TempDir::new("settings");
            let (main, health, poison_messages) = main_access(&settings_dir.path);
            if startup {
                main.with_sync(|db| settings_service::set_api_enabled(db, true))
                    .unwrap();
            }
            let peer = PeerFixture::new(case);
            let discovery = settings_dir.path.join("api.json");
            let mut lifecycle = ApiLifecycle::new(peer.path.clone(), discovery, main.clone());

            let error = if startup {
                lifecycle.startup().await.unwrap_err()
            } else {
                lifecycle.set_enabled(true).await.unwrap_err()
            };

            case.assert_error(error);
            assert!(!main
                .with_sync(|db| settings_service::get_api_enabled(&db.conn))
                .unwrap());
            assert!(!health.is_poisoned());
            assert!(poison_messages.try_recv().is_err());
        }
    }
}

struct ReleaseGuard {
    release: Arc<(Mutex<bool>, Condvar)>,
    armed: bool,
}

impl ReleaseGuard {
    fn new(release: Arc<(Mutex<bool>, Condvar)>) -> Self {
        Self {
            release,
            armed: true,
        }
    }

    fn release(&mut self) {
        if self.armed {
            let (flag, condvar) = &*self.release;
            let mut released = flag.lock().unwrap();
            *released = true;
            condvar.notify_all();
            self.armed = false;
        }
    }
}

impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        self.release();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_cleanup_precedes_settings_read() {
    let _test_guard = LIFECYCLE_TEST_LOCK.lock().await;
    let dir = TempDir::new("cleanup-order");
    let database_path = dir.path.join("cadence.db");
    let conn = db::connect(&database_path).unwrap();
    conn.pragma_update(None, "user_version", db::CURRENT_SCHEMA_VERSION)
        .unwrap();
    let (health, poison_messages) = Health::recording();
    let main = DbAccess::new(Db { conn, health });
    let discovery_path = dir.path.join("api.json");
    fs::write(&discovery_path, b"stale").unwrap();

    let lifecycle = Arc::new(tokio::sync::Mutex::new(ApiLifecycle::new(
        database_path,
        discovery_path.clone(),
        main.clone(),
    )));
    let (acquired_tx, acquired_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let mut release_guard = ReleaseGuard::new(release.clone());

    let held_main = main.clone();
    let held_release = release.clone();
    let holder = tokio::task::spawn_blocking(move || {
        held_main.with_sync(|_| {
            acquired_tx.send(()).unwrap();
            let (flag, condvar) = &*held_release;
            let released = flag.lock().unwrap();
            let (released, timeout) = condvar
                .wait_timeout_while(released, Duration::from_secs(10), |value| !*value)
                .unwrap();
            if timeout.timed_out() && !*released {
                return Err(AppError::internal("release timeout"));
            }
            Ok(())
        })
    });

    acquired_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let startup_lifecycle = lifecycle.clone();
    let startup = tokio::spawn(async move { startup_lifecycle.lock().await.startup().await });

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stale_removed_before_release = false;
    while Instant::now() < deadline {
        if !discovery_path.exists() {
            stale_removed_before_release = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    release_guard.release();
    let holder_result = tokio::time::timeout(Duration::from_secs(10), holder)
        .await
        .unwrap()
        .unwrap();
    let startup_result = tokio::time::timeout(Duration::from_secs(10), startup)
        .await
        .unwrap()
        .unwrap();

    assert!(stale_removed_before_release);
    holder_result.unwrap();
    let status = startup_result.unwrap();
    assert!(!status.enabled);
    assert!(!discovery_path.exists());
    assert!(poison_messages.try_recv().is_err());
}
