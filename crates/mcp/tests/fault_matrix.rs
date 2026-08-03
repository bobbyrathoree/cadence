#![cfg(all(feature = "test-support", feature = "test-faults"))]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cadence_core::db::{schema, CURRENT_SCHEMA_VERSION};
use rusqlite::Connection;
use serde_json::{json, Value};

const VERSION: &str = "2026-07-28";
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "cadence-mcp-fault-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create fault temp directory");
        Self { path }
    }

    fn database(&self) -> PathBuf {
        self.path.join("fixture.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct FaultClient {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<Result<String, String>>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_thread: Option<JoinHandle<()>>,
    next_id: u64,
}

impl FaultClient {
    fn spawn(database: &Path, fault: &str, allow_writes: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cadence-mcp"));
        command
            .env("CADENCE_DB_PATH", database)
            .env("CADENCE_MCP_FAULT", fault)
            .env_remove("CADENCE_MCP_ALLOW_WRITES")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if allow_writes {
            command.env("CADENCE_MCP_ALLOW_WRITES", "1");
        }
        let mut child = command.spawn().expect("spawn fault-enabled cadence-mcp");
        let stdin = child.stdin.take().expect("capture child stdin");
        let stdout = child.stdout.take().expect("capture child stdout");
        let stderr_pipe = child.stderr.take().expect("capture child stderr");

        let (line_sender, lines) = mpsc::channel();
        let stdout_thread = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line_sender
                    .send(line.map_err(|error| error.to_string()))
                    .is_err()
                {
                    break;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let stderr_output = stderr.clone();
        let stderr_thread = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = BufReader::new(stderr_pipe).read_to_end(&mut bytes);
            if let Ok(mut output) = stderr_output.lock() {
                *output = bytes;
            }
        });

        Self {
            child,
            stdin: Some(stdin),
            lines,
            stdout_thread: Some(stdout_thread),
            stderr,
            stderr_thread: Some(stderr_thread),
            next_id: 1,
        }
    }

    fn initialize(&mut self) {
        let response = self.request(
            "initialize",
            json!({
                "protocolVersion": VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "cadence-fault-tests",
                    "version": "1.0.0"
                }
            }),
            READ_TIMEOUT,
        );
        assert_eq!(response["result"]["protocolVersion"], VERSION);
        self.write_frame(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    fn tool_call(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
            TOOL_TIMEOUT,
        )
    }

    fn send_tool_call(&mut self, name: &str, arguments: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.write_frame(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }));
        id
    }

    fn request(&mut self, method: &str, parameters: Value, timeout: Duration) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.write_frame(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": parameters
        }));
        let line = self
            .lines
            .recv_timeout(timeout)
            .unwrap_or_else(|error| panic!("timed out waiting for {method}: {error}"))
            .unwrap_or_else(|error| panic!("failed reading {method} response: {error}"));
        let response: Value =
            serde_json::from_str(&line).expect("response is valid LF-delimited JSON");
        assert_eq!(response["id"], id);
        response
    }

    fn assert_trigger_exits(mut self, diagnostic: &str) {
        match self.lines.recv_timeout(TOOL_TIMEOUT) {
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("poisoning request did not terminate the server")
            }
            Ok(Ok(line)) => panic!("poisoning request unexpectedly completed: {line}"),
            Ok(Err(error)) => panic!("stdout read failed before EOF: {error}"),
        }
        let status = wait_for_exit(&mut self.child, Duration::from_secs(5));
        assert_eq!(status.code(), Some(10));
        self.join_readers();
        let stderr = self.stderr_text();
        assert!(
            stderr.contains(diagnostic),
            "stderr {stderr:?} did not contain {diagnostic:?}"
        );
    }

    fn write_frame(&mut self, value: &Value) {
        let stdin = self.stdin.as_mut().expect("child stdin is open");
        serde_json::to_writer(&mut *stdin, value).expect("serialize JSON-RPC frame");
        stdin.write_all(b"\n").expect("terminate JSON-RPC frame");
        stdin.flush().expect("flush JSON-RPC frame");
    }

    fn join_readers(&mut self) {
        self.stdin.take();
        if let Some(thread) = self.stdout_thread.take() {
            thread.join().expect("join stdout reader");
        }
        if let Some(thread) = self.stderr_thread.take() {
            thread.join().expect("join stderr reader");
        }
    }

    fn stderr_text(&self) -> String {
        self.stderr
            .lock()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_else(|_| "stderr lock poisoned".to_string())
    }
}

impl Drop for FaultClient {
    fn drop(&mut self) {
        self.stdin.take();
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        if let Some(thread) = self.stdout_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join();
        }
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().expect("poll child status") {
            Some(status) => return status,
            None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("child did not exit within {timeout:?}");
            }
        }
    }
}

fn create_fixture(path: &Path) {
    let conn = Connection::open(path).expect("create fault database");
    schema::create_tables(&conn).expect("create fault schema");
    conn.pragma_update(None, "journal_mode", "WAL")
        .expect("enable fixture WAL");
    conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
        .expect("set fixture schema version");
}

fn create_args(title: &str) -> Value {
    json!({
        "title": title,
        "content": "Fault fixture content",
        "description": null,
        "tags": null,
        "variant_label": null
    })
}

fn prompt_count(path: &Path) -> i64 {
    Connection::open(path)
        .expect("open database for prompt count")
        .query_row("SELECT COUNT(*) FROM prompts", [], |row| row.get(0))
        .expect("count prompts")
}

#[test]
fn panic_in_tool_exits_at_the_triggering_request() {
    let temp = TempDir::new("panic");
    let database = temp.database();
    create_fixture(&database);
    let mut client = FaultClient::spawn(&database, "panic_in_tool", false);
    client.initialize();
    client.send_tool_call("get_prompt", json!({ "id_or_title": "missing" }));
    client.assert_trigger_exits("cadence: fatal: panic in db closure");
}

#[test]
fn write_scope_close_failure_exits_at_the_triggering_request() {
    let temp = TempDir::new("scope-close");
    let database = temp.database();
    create_fixture(&database);
    let mut client = FaultClient::spawn(&database, "fail_write_scope_close", true);
    client.initialize();
    client.send_tool_call("create_prompt", create_args("Close failure"));
    client.assert_trigger_exits("cadence: fatal: write scope close failed");
}

#[test]
fn commit_busy_once_retries_and_inserts_exactly_one_row() {
    let temp = TempDir::new("commit-busy");
    let database = temp.database();
    create_fixture(&database);
    let mut client = FaultClient::spawn(&database, "commit_busy_once", true);
    client.initialize();
    let response = client.tool_call("create_prompt", create_args("Retried once"));

    assert_eq!(response["result"]["isError"], false);
    drop(client);
    assert_eq!(prompt_count(&database), 1);
}

#[test]
fn begin_busy_always_returns_conflict_without_writing() {
    let temp = TempDir::new("begin-busy");
    let database = temp.database();
    create_fixture(&database);
    let mut client = FaultClient::spawn(&database, "begin_busy_always", true);
    client.initialize();
    let response = client.tool_call("create_prompt", create_args("Never inserted"));

    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        response["result"]["content"][0]["text"],
        "database is busy; try again"
    );
    drop(client);
    assert_eq!(prompt_count(&database), 0);
}
