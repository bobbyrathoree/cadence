pub mod migrations;
pub mod schema;

use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub use migrations::migrate;

/// Return the path to the Cadence database file.
/// Uses `~/Library/Application Support/Cadence/cadence.db` via `dirs::data_dir()`.
pub fn db_path() -> PathBuf {
    let mut path = dirs::data_dir().expect("Could not determine data directory");
    path.push("Cadence");
    path
}

pub fn database_file() -> PathBuf {
    db_path().join("cadence.db")
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

/// Initialize a connection to the application database.
pub fn init() -> rusqlite::Result<Connection> {
    let dir = db_path();
    fs::create_dir_all(&dir).expect("Failed to create database directory");

    connect(&database_file())
}
