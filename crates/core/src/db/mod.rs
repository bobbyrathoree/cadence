pub mod migrations;
pub mod schema;

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rusqlite::{Connection, Error as SqliteError, ErrorCode, OpenFlags};

pub use migrations::{migrate, CURRENT_SCHEMA_VERSION};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateError {
    NoDataDir,
    OverrideInvalid(String),
}

impl fmt::Display for LocateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDataDir => formatter.write_str("cannot determine data directory"),
            Self::OverrideInvalid(detail) => {
                write!(formatter, "invalid database override: {detail}")
            }
        }
    }
}

impl std::error::Error for LocateError {}

pub fn locate_database(override_var: Option<&OsStr>) -> Result<PathBuf, LocateError> {
    let data_dir = dirs::data_dir();
    locate_database_in(override_var, data_dir.as_deref())
}

#[cfg(feature = "test-support")]
pub fn locate_database_with_data_dir(
    override_var: Option<&OsStr>,
    data_dir: Option<&Path>,
) -> Result<PathBuf, LocateError> {
    locate_database_in(override_var, data_dir)
}

fn locate_database_in(
    override_var: Option<&OsStr>,
    data_dir: Option<&Path>,
) -> Result<PathBuf, LocateError> {
    if let Some(override_var) = override_var {
        let value = override_var
            .to_str()
            .ok_or_else(|| LocateError::OverrideInvalid("path must be valid UTF-8".to_string()))?;
        if value.is_empty() {
            return Err(LocateError::OverrideInvalid(
                "path must not be empty".to_string(),
            ));
        }

        let path = PathBuf::from(override_var);
        if !path.is_absolute() {
            return Err(LocateError::OverrideInvalid(
                "path must be absolute".to_string(),
            ));
        }
        return Ok(path);
    }

    let data_dir = data_dir.ok_or(LocateError::NoDataDir)?;
    Ok(data_dir.join("Cadence").join("cadence.db"))
}

#[derive(Clone)]
pub struct Health(Arc<HealthInner>);

struct HealthInner {
    poisoned: AtomicBool,
    on_poison: Box<dyn Fn(&str) + Send + Sync>,
}

impl Health {
    pub fn exit_process(code: i32) -> Self {
        Self(Arc::new(HealthInner {
            poisoned: AtomicBool::new(false),
            on_poison: Box::new(move |message| {
                let _ = writeln!(io::stderr().lock(), "cadence: fatal: {message}");
                std::process::exit(code)
            }),
        }))
    }

    #[cfg(feature = "test-support")]
    pub fn recording() -> (Self, std::sync::mpsc::Receiver<String>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let health = Self(Arc::new(HealthInner {
            poisoned: AtomicBool::new(false),
            on_poison: Box::new(move |message| {
                let _ = sender.send(message.to_string());
            }),
        }));
        (health, receiver)
    }

    pub fn poison(&self, message: &str) {
        self.0.poisoned.store(true, Ordering::SeqCst);
        (self.0.on_poison)(message);
    }

    pub fn is_poisoned(&self) -> bool {
        self.0.poisoned.load(Ordering::SeqCst)
    }
}

pub struct Db {
    pub conn: Connection,
    pub health: Health,
}

pub enum DbOpen {
    Ready(Db),
    NeedsMigration(Db, i64),
    SchemaNewer { db_version: i64, supported: i64 },
    MissingFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbOpenError {
    Io(String),
    Corrupt(String),
    Pragma(String),
}

impl fmt::Display for DbOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(detail) => write!(formatter, "database I/O error: {detail}"),
            Self::Corrupt(detail) => write!(formatter, "database corruption: {detail}"),
            Self::Pragma(detail) => write!(formatter, "database configuration error: {detail}"),
        }
    }
}

impl std::error::Error for DbOpenError {}

pub fn open_app(path: &Path, health: Health) -> Result<DbOpen, DbOpenError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| DbOpenError::Io(error.to_string()))?;
    }

    let conn = open_connection(path, true)?;
    configure_common(&conn)?;
    let version = read_schema_version(&conn)?;
    if let Some(rejected) = reject_schema_version(version) {
        return rejected;
    }

    let journal_mode = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))
        .map_err(map_pragma_error)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(DbOpenError::Pragma(format!(
            "could not enable WAL mode (got {journal_mode})"
        )));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(map_pragma_error)?;

    Ok(classify_admitted(conn, health, version))
}

pub fn open_peer(path: &Path, health: Health) -> Result<DbOpen, DbOpenError> {
    match fs::metadata(path) {
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(DbOpen::MissingFile);
        }
        Err(error) => return Err(DbOpenError::Io(error.to_string())),
    }

    let conn = open_connection(path, false)?;
    configure_common(&conn)?;
    let version = read_schema_version(&conn)?;
    if let Some(rejected) = reject_schema_version(version) {
        return rejected;
    }

    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(map_pragma_error)?;
    if version < CURRENT_SCHEMA_VERSION {
        return Ok(DbOpen::NeedsMigration(Db { conn, health }, version));
    }

    let journal_mode = conn
        .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
        .map_err(map_pragma_error)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(DbOpenError::Corrupt("database not in WAL mode".to_string()));
    }

    Ok(DbOpen::Ready(Db { conn, health }))
}

fn open_connection(path: &Path, create: bool) -> Result<Connection, DbOpenError> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    Connection::open_with_flags(path, flags).map_err(map_open_error)
}

fn configure_common(conn: &Connection) -> Result<(), DbOpenError> {
    conn.pragma_update(None, "busy_timeout", 5000_i64)
        .map_err(map_pragma_error)?;
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(map_pragma_error)
}

fn read_schema_version(conn: &Connection) -> Result<i64, DbOpenError> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_pragma_error)
}

fn reject_schema_version(version: i64) -> Option<Result<DbOpen, DbOpenError>> {
    if version > CURRENT_SCHEMA_VERSION {
        return Some(Ok(DbOpen::SchemaNewer {
            db_version: version,
            supported: CURRENT_SCHEMA_VERSION,
        }));
    }
    if version < 0 {
        return Some(Err(DbOpenError::Corrupt(format!(
            "invalid schema version {version}"
        ))));
    }
    None
}

fn classify_admitted(conn: Connection, health: Health, version: i64) -> DbOpen {
    let db = Db { conn, health };
    if version < CURRENT_SCHEMA_VERSION {
        DbOpen::NeedsMigration(db, version)
    } else {
        DbOpen::Ready(db)
    }
}

fn map_open_error(error: SqliteError) -> DbOpenError {
    if is_corrupt(&error) {
        DbOpenError::Corrupt(error.to_string())
    } else {
        DbOpenError::Io(error.to_string())
    }
}

fn map_pragma_error(error: SqliteError) -> DbOpenError {
    if is_corrupt(&error) {
        DbOpenError::Corrupt(error.to_string())
    } else {
        DbOpenError::Pragma(error.to_string())
    }
}

fn is_corrupt(error: &SqliteError) -> bool {
    matches!(
        error,
        SqliteError::SqliteFailure(sqlite_error, _)
            if matches!(
                sqlite_error.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            )
    )
}

/// Open a database connection with the settings required by Cadence's
/// main-connection plus optional API-connection topology.
pub fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;

    // WAL permits concurrent readers, while busy_timeout lets the two
    // connections serialize their short write transactions instead of
    // immediately surfacing SQLITE_BUSY. Both settings are required on
    // every connection.
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA synchronous = NORMAL;
        ",
    )?;

    schema::create_tables(&conn)?;
    Ok(conn)
}
