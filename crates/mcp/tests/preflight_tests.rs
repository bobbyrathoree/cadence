use std::ffi::OsStr;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use cadence_core::db::schema;
use cadence_core::db::{open_peer, DbOpen, Health, CURRENT_SCHEMA_VERSION};
use cadence_core::db_access::DbAccess;
use cadence_mcp::write_scope::WriteScope;
use cadence_mcp::{enable_query_only, McpState, SERVER_INSTRUCTIONS, SUPPORTED_PROTOCOL_VERSIONS};
use rmcp::model::ProtocolVersion;
use rmcp::ServerHandler;
use rusqlite::Connection;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "cadence-mcp-preflight-{label}-{}-{}",
            std::process::id(),
            cadence_id()
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        Self { path }
    }

    fn database(&self) -> PathBuf {
        self.path.join("cadence.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn cadence_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

fn run_with_path(path: &OsStr) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cadence-mcp"))
        .env("CADENCE_DB_PATH", path)
        .env_remove("CADENCE_MCP_FAULT")
        .output()
        .expect("run cadence-mcp")
}

fn assert_failure(output: Output, code: i32, phrase: &str) {
    assert_eq!(output.status.code(), Some(code));
    assert!(output.stdout.is_empty(), "stdout must remain empty");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains(phrase),
        "stderr {stderr:?} did not contain {phrase:?}"
    );
    assert_eq!(stderr.lines().count(), 1, "preflight writes one line");
}

fn create_database(path: &Path, version: i64, wal: bool) -> Connection {
    let conn = Connection::open(path).expect("create SQLite database");
    schema::create_tables(&conn).expect("create schema");
    if wal {
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("enable WAL");
    }
    conn.pragma_update(None, "user_version", version)
        .expect("set schema version");
    conn
}

#[test]
fn help_and_version_use_stdout_and_exit_zero() {
    for (argument, phrase) in [
        ("--help", "Usage: cadence-mcp"),
        ("--version", env!("CARGO_PKG_VERSION")),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cadence-mcp"))
            .arg(argument)
            .output()
            .expect("run informational command");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8(output.stdout)
            .expect("stdout is UTF-8")
            .contains(phrase));
    }
}

#[test]
fn invalid_override_exits_six_without_stdout() {
    for path in [OsStr::new("relative.db"), OsStr::new("")] {
        assert_failure(
            run_with_path(path),
            6,
            "cadence-mcp: invalid CADENCE_DB_PATH (must be an absolute path)",
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_override_exits_six_without_stdout() {
    assert_failure(
        run_with_path(OsStr::from_bytes(b"/tmp/cadence-\xff.db")),
        6,
        "cadence-mcp: invalid CADENCE_DB_PATH (must be an absolute path)",
    );
}

#[test]
fn missing_database_exits_two_and_creates_nothing() {
    let temp = TempDir::new("missing");
    let missing_parent = temp.path.join("absent");
    let path = missing_parent.join("cadence.db");

    assert_failure(
        run_with_path(path.as_os_str()),
        2,
        "no Cadence database found",
    );
    assert!(!path.exists());
    assert!(!missing_parent.exists());
}

#[test]
fn old_database_exits_three_without_stdout() {
    let temp = TempDir::new("old");
    let path = temp.database();
    drop(create_database(&path, 0, false));

    assert_failure(
        run_with_path(path.as_os_str()),
        3,
        "database schema v0 is older than supported v3",
    );
}

#[test]
fn newer_database_exits_four_without_stdout() {
    let temp = TempDir::new("newer");
    let path = temp.database();
    drop(create_database(&path, CURRENT_SCHEMA_VERSION + 1, false));

    assert_failure(
        run_with_path(path.as_os_str()),
        4,
        "is newer than this binary supports",
    );
}

#[test]
fn corrupt_database_exits_five_without_stdout() {
    let temp = TempDir::new("corrupt");
    let path = temp.database();
    fs::write(&path, b"not a SQLite database").expect("write corrupt fixture");

    assert_failure(
        run_with_path(path.as_os_str()),
        5,
        "cadence-mcp: database is corrupt:",
    );
}

#[test]
fn io_failure_exits_six_without_stdout() {
    let temp = TempDir::new("io");

    assert_failure(run_with_path(temp.path.as_os_str()), 6, "cadence-mcp:");
}

#[test]
fn locked_pragma_exits_eight_without_stdout() {
    let temp = TempDir::new("pragma");
    let path = temp.database();
    let conn = create_database(&path, 0, false);
    conn.execute_batch("BEGIN EXCLUSIVE")
        .expect("hold exclusive lock");

    let output = run_with_path(path.as_os_str());
    conn.execute_batch("ROLLBACK").expect("release lock");

    assert_failure(output, 8, "cadence-mcp: pragma failure:");
}

#[test]
fn query_only_is_enabled_and_server_metadata_is_pinned() {
    let temp = TempDir::new("query-only");
    let path = temp.database();
    drop(create_database(&path, CURRENT_SCHEMA_VERSION, true));
    let mut db = match open_peer(&path, Health::exit_process(10)).expect("open peer database") {
        DbOpen::Ready(db) => db,
        _ => panic!("current WAL database must be ready"),
    };

    enable_query_only(&mut db).expect("enable query-only mode");
    let query_only: i64 = db
        .conn
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .expect("read query-only mode");
    assert_eq!(query_only, 1);

    let state = McpState::new(DbAccess::new(db));
    let info = state.get_info();
    assert_eq!(info.server_info.name, "cadence-mcp");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.instructions.as_deref(), Some(SERVER_INSTRUCTIONS));
    assert!(info.capabilities.tools.is_some());
    assert!(info.capabilities.prompts.is_some());
    assert!(info.capabilities.resources.is_some());
    assert!(info.capabilities.completions.is_some());
    assert_eq!(
        info.capabilities
            .tools
            .as_ref()
            .and_then(|capability| capability.list_changed),
        None
    );
    assert_eq!(
        info.capabilities
            .prompts
            .as_ref()
            .and_then(|capability| capability.list_changed),
        None
    );
    assert_eq!(
        info.capabilities
            .resources
            .as_ref()
            .and_then(|capability| capability.list_changed),
        None
    );
    assert_eq!(
        state.supported_protocol_versions().as_ref(),
        [ProtocolVersion::V_2025_11_25, ProtocolVersion::V_2026_07_28]
    );
    assert_eq!(
        SUPPORTED_PROTOCOL_VERSIONS,
        [ProtocolVersion::V_2025_11_25, ProtocolVersion::V_2026_07_28]
    );
}

#[test]
fn write_scope_temporarily_disables_query_only() {
    let temp = TempDir::new("write-scope");
    let path = temp.database();
    drop(create_database(&path, CURRENT_SCHEMA_VERSION, true));
    let mut db = match open_peer(&path, Health::exit_process(10)).expect("open peer database") {
        DbOpen::Ready(db) => db,
        _ => panic!("current WAL database must be ready"),
    };
    enable_query_only(&mut db).expect("enable query-only mode");

    let mut scope = WriteScope::open(&mut db).expect("open write scope");
    let writable: i64 = scope
        .db()
        .conn
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .expect("read query-only mode");
    assert_eq!(writable, 0);
    scope.close().expect("close write scope");

    let query_only: i64 = db
        .conn
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .expect("read restored query-only mode");
    assert_eq!(query_only, 1);
}
