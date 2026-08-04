pub mod dto;
mod fault;
mod prompts;
mod resources;
mod server;
mod tools;
pub mod write_scope;

use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::Path;

use cadence_core::db::{
    locate_database, open_peer, Db, DbOpen, DbOpenError, Health, LocateError,
    CURRENT_SCHEMA_VERSION,
};
use rmcp::{transport::stdio, ServiceExt};

pub use server::{CadenceMcp, McpState, SERVER_INSTRUCTIONS, SUPPORTED_PROTOCOL_VERSIONS};

const HELP: &str = "Usage: cadence-mcp [--help | --version]\n";

pub fn run() -> i32 {
    match command() {
        Command::Help => return write_stdout(HELP),
        Command::Version => {
            return write_stdout(&format!("cadence-mcp {}\n", env!("CARGO_PKG_VERSION")));
        }
        Command::Serve => {}
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            write_stderr(&format!(
                "cadence-mcp: runtime initialization failed: {error}"
            ));
            return 1;
        }
    };
    runtime.block_on(run_server())
}

enum Command {
    Help,
    Version,
    Serve,
}

fn command() -> Command {
    let mut args = env::args_os().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some(arg), None) if arg == OsStr::new("--help") => Command::Help,
        (Some(arg), None) if arg == OsStr::new("--version") => Command::Version,
        _ => Command::Serve,
    }
}

async fn run_server() -> i32 {
    let override_path = env::var_os("CADENCE_DB_PATH");
    let path = match locate_database(override_path.as_deref()) {
        Ok(path) => path,
        Err(error) => {
            let (code, message) = locate_failure(error);
            write_stderr(message);
            return code;
        }
    };

    let mut db = match open_peer(&path, Health::exit_process(10)) {
        Ok(DbOpen::Ready(db)) => db,
        Ok(other) => {
            let (code, message) = open_state_failure(&path, other);
            write_stderr(&message);
            return code;
        }
        Err(error) => {
            let (code, message) = open_error_failure(error);
            write_stderr(&message);
            return code;
        }
    };

    if let Err(error) = enable_query_only(&mut db) {
        write_stderr(&format!("cadence-mcp: pragma failure: {error}"));
        return 8;
    }

    let state = McpState::new(cadence_core::db_access::DbAccess::new(db));
    match state.serve(stdio()).await {
        Ok(service) => match service.waiting().await {
            Ok(_) => 0,
            Err(error) => {
                write_stderr(&format!("cadence-mcp: transport failure: {error}"));
                1
            }
        },
        Err(error) => {
            write_stderr(&format!(
                "cadence-mcp: transport initialization failed: {error}"
            ));
            1
        }
    }
}

pub fn enable_query_only(db: &mut Db) -> rusqlite::Result<()> {
    db.conn.pragma_update(None, "query_only", true)?;
    let enabled: i64 = db
        .conn
        .pragma_query_value(None, "query_only", |row| row.get(0))?;
    if enabled == 1 {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn locate_failure(error: LocateError) -> (i32, &'static str) {
    match error {
        LocateError::NoDataDir => (6, "cadence-mcp: cannot determine data directory"),
        LocateError::OverrideInvalid(_) => (
            6,
            "cadence-mcp: invalid CADENCE_DB_PATH (must be an absolute path)",
        ),
    }
}

fn open_state_failure(path: &Path, state: DbOpen) -> (i32, String) {
    match state {
        DbOpen::MissingFile => (
            2,
            format!(
                "cadence-mcp: no Cadence database found at {}; launch the Cadence app first",
                path.display()
            ),
        ),
        DbOpen::NeedsMigration(_, version) => (
            3,
            format!(
                "cadence-mcp: database schema v{version} is older than supported v{CURRENT_SCHEMA_VERSION}; launch the Cadence app to migrate"
            ),
        ),
        DbOpen::SchemaNewer {
            db_version,
            supported,
        } => (
            4,
            format!(
                "cadence-mcp: database schema v{db_version} is newer than this binary supports (v{supported}); this cadence-mcp is outdated — check which Cadence installation your MCP config points at"
            ),
        ),
        DbOpen::Ready(_) => unreachable!("ready databases are handled before failure mapping"),
    }
}

fn open_error_failure(error: DbOpenError) -> (i32, String) {
    match error {
        DbOpenError::Corrupt(detail) => (5, format!("cadence-mcp: database is corrupt: {detail}")),
        DbOpenError::Io(detail) => (6, format!("cadence-mcp: {detail}")),
        DbOpenError::Pragma(detail) => (8, format!("cadence-mcp: pragma failure: {detail}")),
    }
}

fn write_stdout(message: &str) -> i32 {
    let mut stdout = io::stdout().lock();
    if stdout
        .write_all(message.as_bytes())
        .and_then(|_| stdout.flush())
        .is_ok()
    {
        0
    } else {
        1
    }
}

fn write_stderr(message: &str) {
    let mut stderr = io::stderr().lock();
    let message = message.replace(['\r', '\n'], " ");
    let _ = writeln!(stderr, "{message}");
}
