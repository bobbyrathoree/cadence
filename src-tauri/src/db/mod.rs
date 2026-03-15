pub mod schema;

use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

/// Return the path to the Cadence database file.
/// Uses `~/Library/Application Support/Cadence/cadence.db` via `dirs::data_dir()`.
pub fn db_path() -> PathBuf {
    let mut path = dirs::data_dir().expect("Could not determine data directory");
    path.push("Cadence");
    path
}

/// Initialize the database: create the directory, open the connection,
/// set PRAGMAs, and run schema creation.
pub fn init() -> rusqlite::Result<Connection> {
    let dir = db_path();
    fs::create_dir_all(&dir).expect("Failed to create database directory");

    let db_file = dir.join("cadence.db");
    let conn = Connection::open(&db_file)?;

    // PRAGMA settings
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        ",
    )?;

    schema::create_tables(&conn)?;

    Ok(conn)
}
