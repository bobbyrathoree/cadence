use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use cadence_core::db::schema;
use cadence_core::db::{
    locate_database, locate_database_with_data_dir, open_app, open_peer, DbOpen, DbOpenError,
    Health, LocateError, CURRENT_SCHEMA_VERSION,
};
use rusqlite::Connection;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Ready,
    NeedsMigration(i64),
    SchemaNewer { db_version: i64, supported: i64 },
    MissingFile,
    Io,
    Corrupt(String),
    Pragma,
}

#[derive(Clone, Copy)]
enum Opener {
    App,
    Peer,
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "cadence-db-open-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn database(&self) -> PathBuf {
        self.path.join("cadence.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        #[cfg(unix)]
        restore_permissions(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn health() -> Health {
    Health::recording().0
}

fn open(opener: Opener, path: &Path) -> Result<DbOpen, DbOpenError> {
    match opener {
        Opener::App => open_app(path, health()),
        Opener::Peer => open_peer(path, health()),
    }
}

fn outcome(result: Result<DbOpen, DbOpenError>) -> Outcome {
    match result {
        Ok(DbOpen::Ready(_)) => Outcome::Ready,
        Ok(DbOpen::NeedsMigration(_, version)) => Outcome::NeedsMigration(version),
        Ok(DbOpen::SchemaNewer {
            db_version,
            supported,
        }) => Outcome::SchemaNewer {
            db_version,
            supported,
        },
        Ok(DbOpen::MissingFile) => Outcome::MissingFile,
        Err(DbOpenError::Io(_)) => Outcome::Io,
        Err(DbOpenError::Corrupt(detail)) => Outcome::Corrupt(detail),
        Err(DbOpenError::Pragma(_)) => Outcome::Pragma,
    }
}

fn create_database(path: &Path, version: i64, create_tables: bool, wal: bool) {
    let conn = Connection::open(path).unwrap();
    if create_tables {
        schema::create_tables(&conn).unwrap();
    }
    if wal {
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    }
    conn.pragma_update(None, "user_version", version).unwrap();
}

fn file_hash(path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    fs::read(path).unwrap().hash(&mut hasher);
    hasher.finish()
}

fn journal_mode(path: &Path) -> String {
    let conn = Connection::open(path).unwrap();
    conn.pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
        .unwrap()
        .to_lowercase()
}

#[cfg(unix)]
fn restore_permissions(path: &Path) {
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        let _ = fs::set_permissions(path, permissions);
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let mut permissions = metadata.permissions();
                permissions.set_mode(if metadata.is_dir() { 0o700 } else { 0o600 });
                let _ = fs::set_permissions(entry.path(), permissions);
            }
        }
    }
}

#[test]
fn locator_without_override_uses_injected_data_directory() {
    let temp = TempDir::new("locator-default");
    assert_eq!(
        locate_database_with_data_dir(None, Some(&temp.path)).unwrap(),
        temp.path.join("Cadence").join("cadence.db")
    );
}

#[test]
fn locator_without_override_reports_missing_data_directory() {
    assert_eq!(
        locate_database_with_data_dir(None, None),
        Err(LocateError::NoDataDir)
    );
}

#[test]
fn locator_accepts_absolute_utf8_override_verbatim() {
    let temp = TempDir::new("locator-absolute");
    let path = temp.path.join("does-not-need-to-exist.db");
    assert_eq!(locate_database(Some(path.as_os_str())).unwrap(), path);
}

#[test]
fn locator_rejects_relative_and_empty_overrides() {
    assert!(matches!(
        locate_database(Some(OsStr::new("relative.db"))),
        Err(LocateError::OverrideInvalid(_))
    ));
    assert!(matches!(
        locate_database(Some(OsStr::new(""))),
        Err(LocateError::OverrideInvalid(_))
    ));
}

#[cfg(unix)]
#[test]
fn locator_rejects_non_utf8_override() {
    let invalid = OsStr::from_bytes(b"/tmp/cadence-\xff.db");
    assert!(matches!(
        locate_database(Some(invalid)),
        Err(LocateError::OverrideInvalid(_))
    ));
}

#[test]
fn recording_health_shares_poison_state_and_message() {
    let (health, receiver) = Health::recording();
    let clone = health.clone();

    clone.poison("recorded failure");

    assert!(health.is_poisoned());
    assert!(clone.is_poisoned());
    assert_eq!(receiver.recv().unwrap(), "recorded failure");
}

#[test]
fn missing_file_matrix() {
    let app = TempDir::new("missing-file-app");
    let peer = TempDir::new("missing-file-peer");

    assert_eq!(
        outcome(open(Opener::App, &app.database())),
        Outcome::NeedsMigration(0)
    );
    assert!(app.database().exists());
    assert_eq!(
        outcome(open(Opener::Peer, &peer.database())),
        Outcome::MissingFile
    );
    assert!(!peer.database().exists());
}

#[test]
fn missing_parent_matrix() {
    let app = TempDir::new("missing-parent-app");
    let peer = TempDir::new("missing-parent-peer");
    let app_path = app.path.join("missing").join("cadence.db");
    let peer_path = peer.path.join("missing").join("cadence.db");

    assert_eq!(
        outcome(open(Opener::App, &app_path)),
        Outcome::NeedsMigration(0)
    );
    assert!(app_path.exists());
    assert_eq!(
        outcome(open(Opener::Peer, &peer_path)),
        Outcome::MissingFile
    );
    assert!(!peer.path.join("missing").exists());
}

#[test]
fn fresh_empty_file_matrix() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("fresh-empty");
        fs::File::create(temp.database()).unwrap();
        assert_eq!(
            outcome(open(opener, &temp.database())),
            Outcome::NeedsMigration(0)
        );
    }
}

#[test]
fn version_zero_with_tables_matrix() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("v0-tables");
        create_database(&temp.database(), 0, true, false);
        assert_eq!(
            outcome(open(opener, &temp.database())),
            Outcome::NeedsMigration(0)
        );
    }
}

#[test]
fn older_schema_matrix() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("older");
        create_database(&temp.database(), CURRENT_SCHEMA_VERSION - 1, true, false);
        assert_eq!(
            outcome(open(opener, &temp.database())),
            Outcome::NeedsMigration(CURRENT_SCHEMA_VERSION - 1)
        );
    }
}

#[test]
fn current_schema_matrix() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("current");
        create_database(&temp.database(), CURRENT_SCHEMA_VERSION, true, true);
        assert_eq!(outcome(open(opener, &temp.database())), Outcome::Ready);
    }
}

#[test]
fn newer_schema_matrix_is_byte_unchanged() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("newer");
        create_database(&temp.database(), CURRENT_SCHEMA_VERSION + 1, true, false);
        let before = file_hash(&temp.database());

        assert_eq!(
            outcome(open(opener, &temp.database())),
            Outcome::SchemaNewer {
                db_version: CURRENT_SCHEMA_VERSION + 1,
                supported: CURRENT_SCHEMA_VERSION,
            }
        );
        assert_eq!(file_hash(&temp.database()), before);
    }
}

#[test]
fn negative_schema_matrix_is_corrupt_and_byte_unchanged() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("negative");
        create_database(&temp.database(), -1, true, false);
        let before = file_hash(&temp.database());

        assert_eq!(
            outcome(open(opener, &temp.database())),
            Outcome::Corrupt("invalid schema version -1".to_string())
        );
        assert_eq!(file_hash(&temp.database()), before);
    }
}

#[test]
fn corrupt_bytes_matrix() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("corrupt");
        fs::write(temp.database(), b"this is not a sqlite database").unwrap();
        assert!(matches!(
            outcome(open(opener, &temp.database())),
            Outcome::Corrupt(_)
        ));
    }
}

#[cfg(unix)]
#[test]
fn unreadable_file_matrix_is_io_error() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("unreadable");
        create_database(&temp.database(), CURRENT_SCHEMA_VERSION, true, true);
        let mut permissions = fs::metadata(temp.database()).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(temp.database(), permissions).unwrap();

        assert_eq!(outcome(open(opener, &temp.database())), Outcome::Io);
        restore_permissions(&temp.path);
    }
}

#[test]
fn fresh_database_opened_by_app_uses_wal() {
    let temp = TempDir::new("app-wal");
    assert_eq!(
        outcome(open(Opener::App, &temp.database())),
        Outcome::NeedsMigration(0)
    );
    assert_eq!(journal_mode(&temp.database()), "wal");
}

#[test]
fn fresh_app_bootstrap_creates_schema_and_migrates_to_current() {
    let temp = TempDir::new("fresh-bootstrap");
    let mut db = match open_app(&temp.database(), health()).unwrap() {
        DbOpen::NeedsMigration(db, 0) => db,
        _ => panic!("fresh app database must require migration from version zero"),
    };

    schema::create_tables(&db.conn).unwrap();
    cadence_core::db::migrate(&mut db.conn).unwrap();

    let version: i64 = db
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_SCHEMA_VERSION);
}

#[test]
fn peer_allows_non_wal_older_database_but_rejects_current_database() {
    let older = TempDir::new("peer-non-wal-v0");
    create_database(&older.database(), 0, true, false);
    assert_eq!(
        outcome(open(Opener::Peer, &older.database())),
        Outcome::NeedsMigration(0)
    );

    let current = TempDir::new("peer-non-wal-current");
    create_database(&current.database(), CURRENT_SCHEMA_VERSION, true, false);
    assert_eq!(
        outcome(open(Opener::Peer, &current.database())),
        Outcome::Corrupt("database not in WAL mode".to_string())
    );
}

#[test]
fn peer_never_creates_database_file_or_parent() {
    let temp = TempDir::new("peer-no-create");
    let path = temp.path.join("absent").join("cadence.db");

    assert_eq!(outcome(open(Opener::Peer, &path)), Outcome::MissingFile);
    assert!(!path.exists());
    assert!(!temp.path.join("absent").exists());
}

#[test]
fn busy_during_pragma_is_classified_as_pragma_for_both_openers() {
    for opener in [Opener::App, Opener::Peer] {
        let temp = TempDir::new("pragma-busy");
        create_database(&temp.database(), CURRENT_SCHEMA_VERSION, true, false);
        let holder = Connection::open(temp.database()).unwrap();
        holder.execute_batch("BEGIN EXCLUSIVE").unwrap();

        assert_eq!(outcome(open(opener, &temp.database())), Outcome::Pragma);

        holder.execute_batch("ROLLBACK").unwrap();
    }
}
