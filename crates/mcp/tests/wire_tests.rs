use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cadence_core::db::{schema, CURRENT_SCHEMA_VERSION};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const LEGACY_VERSION: &str = "2025-11-25";
const CURRENT_VERSION: &str = "2026-07-28";
const FIXTURE_PROMPT_ID: &str = "11111111-1111-4111-8111-111111111111";
const FIXTURE_VARIANT_ID: &str = "22222222-2222-4222-8222-222222222222";
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "cadence-mcp-wire-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create wire temp directory");
        Self { path }
    }

    fn database(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WireClient {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<Result<String, String>>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_thread: Option<JoinHandle<()>>,
    next_id: u64,
}

impl WireClient {
    fn spawn(database: &Path, allow_writes: bool) -> Self {
        Self::spawn_with_fault(database, allow_writes, None)
    }

    fn spawn_with_fault(database: &Path, allow_writes: bool, fault: Option<&str>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cadence-mcp"));
        command
            .env("CADENCE_DB_PATH", database)
            .env_remove("CADENCE_MCP_FAULT")
            .env_remove("CADENCE_MCP_ALLOW_WRITES")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if allow_writes {
            command.env("CADENCE_MCP_ALLOW_WRITES", "1");
        }
        if let Some(fault) = fault {
            command.env("CADENCE_MCP_FAULT", fault);
        }
        let mut child = command.spawn().expect("spawn cadence-mcp");
        let stdin = child.stdin.take().expect("capture child stdin");
        let stdout = child.stdout.take().expect("capture child stdout");
        let stderr_pipe = child.stderr.take().expect("capture child stderr");

        let (line_sender, lines) = mpsc::channel();
        let stdout_thread = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let mapped = line.map_err(|error| error.to_string());
                if line_sender.send(mapped).is_err() {
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

    fn initialize(&mut self, version: &str) -> Value {
        let response = self.request(
            "initialize",
            Some(json!({
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {
                    "name": "cadence-wire-tests",
                    "version": "1.0.0"
                }
            })),
            READ_TIMEOUT,
        );
        self.notify("notifications/initialized", None);
        response
    }

    fn tool_call(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            Some(json!({ "name": name, "arguments": arguments })),
            TOOL_TIMEOUT,
        )
    }

    fn request(&mut self, method: &str, parameters: Option<Value>, timeout: Duration) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method
        });
        if let Some(parameters) = parameters {
            request["params"] = parameters;
        }
        self.write_frame(&request);
        let line = self
            .lines
            .recv_timeout(timeout)
            .unwrap_or_else(|error| panic!("timed out waiting for {method}: {error}"))
            .unwrap_or_else(|error| panic!("failed reading {method} response: {error}"));
        let response: Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid JSON response {line:?}: {error}"));
        assert_eq!(response["id"], id);
        response
    }

    fn notify(&mut self, method: &str, parameters: Option<Value>) {
        let mut notification = json!({
            "jsonrpc": "2.0",
            "method": method
        });
        if let Some(parameters) = parameters {
            notification["params"] = parameters;
        }
        self.write_frame(&notification);
    }

    fn write_frame(&mut self, value: &Value) {
        let stdin = self.stdin.as_mut().expect("child stdin is open");
        serde_json::to_writer(&mut *stdin, value).expect("serialize JSON-RPC frame");
        stdin.write_all(b"\n").expect("terminate JSON-RPC frame");
        stdin.flush().expect("flush JSON-RPC frame");
    }

    fn stderr_text(&self) -> String {
        self.stderr
            .lock()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_else(|_| "stderr lock poisoned".to_string())
    }
}

impl Drop for WireClient {
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

fn create_fixture(path: &Path, include_prompt: bool) {
    let conn = Connection::open(path).expect("create wire database");
    schema::create_tables(&conn).expect("create wire schema");
    conn.pragma_update(None, "journal_mode", "WAL")
        .expect("enable fixture WAL");
    conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
        .expect("set fixture schema version");
    if include_prompt {
        conn.execute(
            "INSERT INTO prompts
                (id, title, description, primary_variant_id, is_favorite, is_pinned,
                 copy_count, created_at, updated_at)
             VALUES (?1, 'Fixture prompt', NULL, ?2, 1, 0, 0, NULL, '2026-08-03T12:00:00Z')",
            params![FIXTURE_PROMPT_ID, FIXTURE_VARIANT_ID],
        )
        .expect("insert fixture prompt");
        conn.execute(
            "INSERT INTO variants
                (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
             VALUES (?1, ?2, 'Primary', 'Fixture content', 'static', 0, NULL, NULL)",
            params![FIXTURE_VARIANT_ID, FIXTURE_PROMPT_ID],
        )
        .expect("insert fixture variant");
    }
}

fn snapshot(name: &str, actual: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name);
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("read hand-authored snapshot {}: {error}", path.display()));
    let expected: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse snapshot {}: {error}", path.display()));
    assert_eq!(&expected, actual, "snapshot mismatch: {}", path.display());
}

fn read_snapshot(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name);
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("read hand-authored snapshot {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse snapshot {}: {error}", path.display()))
}

fn assert_handshake(response: &Value, version: &str) {
    let result = &response["result"];
    assert_eq!(result["protocolVersion"], version);
    assert_eq!(result["serverInfo"]["name"], "cadence-mcp");
    assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(result.get("resultType").is_none());
    assert_eq!(result["capabilities"]["tools"], json!({}));
    assert_eq!(result["capabilities"]["prompts"], json!({}));
    assert_eq!(result["capabilities"]["resources"], json!({}));
    assert_eq!(result["capabilities"]["completions"], json!({}));
    assert!(!result["capabilities"].to_string().contains("listChanged"));
}

#[test]
fn tools_list_matches_hand_authored_snapshots_for_both_versions_and_gate_states() {
    let temp = TempDir::new("tool-lists");
    let database = temp.database("fixture.db");
    create_fixture(&database, true);

    for version in [LEGACY_VERSION, CURRENT_VERSION] {
        for allow_writes in [false, true] {
            let mut client = WireClient::spawn(&database, allow_writes);
            let initialize = client.initialize(version);
            assert_handshake(&initialize, version);
            let response = client.request("tools/list", None, READ_TIMEOUT);
            let mut expected_tools = read_snapshot("tools-readonly.json")
                .as_array()
                .expect("readonly tools snapshot is an array")
                .clone();
            if allow_writes {
                let write_tools = read_snapshot("tools-write-only.json")
                    .as_array()
                    .expect("write tools snapshot is an array")
                    .clone();
                expected_tools.extend(write_tools);
                expected_tools
                    .sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
            }
            assert_eq!(response["result"]["tools"], Value::Array(expected_tools));
            let mut envelope = response["result"].clone();
            envelope
                .as_object_mut()
                .expect("tools/list result is an object")
                .remove("tools");
            snapshot(&format!("tools-list-envelope-{version}.json"), &envelope);
            assert_eq!(response["result"]["ttlMs"], 0);
            assert_eq!(response["result"]["cacheScope"], "private");
            if version == LEGACY_VERSION {
                assert!(response["result"].get("resultType").is_none());
            } else {
                assert_eq!(response["result"]["resultType"], "complete");
            }
            assert!(
                client.stderr_text().is_empty(),
                "unexpected stderr: {}",
                client.stderr_text()
            );
        }
    }
}

#[test]
fn tool_outcomes_match_hand_authored_snapshots_for_both_versions() {
    for version in [LEGACY_VERSION, CURRENT_VERSION] {
        let temp = TempDir::new("outcomes");
        let database = temp.database("fixture.db");
        create_fixture(&database, true);
        let mut client = WireClient::spawn(&database, false);
        assert_handshake(&client.initialize(version), version);

        let structured =
            client.tool_call("get_prompt", json!({ "id_or_title": FIXTURE_PROMPT_ID }));
        let domain = client.tool_call("get_prompt", json!({ "id_or_title": "missing" }));
        let unknown_field = client.tool_call(
            "get_prompt",
            json!({ "id_or_title": FIXTURE_PROMPT_ID, "extra": true }),
        );
        let wrong_scalar = client.tool_call("get_prompt", json!({ "id_or_title": 7 }));
        let missing_required = client.tool_call("get_prompt", json!({}));
        let unknown = client.tool_call("does_not_exist", json!({}));
        let disabled = client.tool_call("create_prompt", json!({ "title": "No", "content": "No" }));

        let broken_database = temp.database("broken.db");
        create_fixture(&broken_database, false);
        {
            let conn = Connection::open(&broken_database).expect("open broken fixture");
            conn.execute_batch("DROP TABLE tags")
                .expect("remove table for internal error");
        }
        let mut broken = WireClient::spawn(&broken_database, false);
        assert_handshake(&broken.initialize(version), version);
        let internal = broken.tool_call("list_tags", json!({}));

        let outcomes = json!({
            "structured_success": structured.get("result"),
            "domain_error": domain.get("result"),
            "redacted_internal": internal.get("result"),
            "unknown_field": unknown_field.get("result"),
            "wrong_scalar": wrong_scalar.get("result"),
            "missing_required": missing_required.get("result"),
            "unknown_tool": unknown.get("error"),
            "gate_disabled_tool": disabled.get("error")
        });
        snapshot(&format!("tool-outcomes-{version}.json"), &outcomes);
    }
}

#[test]
fn write_gate_targets_only_the_overridden_database() {
    let temp = TempDir::new("isolation");
    let target = temp.database("target.db");
    let untouched = temp.database("untouched.db");
    create_fixture(&target, false);
    create_fixture(&untouched, false);
    let mut client = WireClient::spawn(&target, true);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.tool_call(
        "create_prompt",
        json!({
            "title": "Isolated write",
            "content": "Only in the override",
            "description": null,
            "tags": ["mcp"],
            "variant_label": null
        }),
    );
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["isError"], false);
    drop(client);

    assert_eq!(prompt_count(&target), 1);
    assert_eq!(prompt_count(&untouched), 0);
}

#[test]
fn duplicate_titles_report_candidates_in_entity_order() {
    let temp = TempDir::new("ambiguous-titles");
    let database = temp.database("fixture.db");
    create_fixture(&database, false);
    {
        let conn = Connection::open(&database).expect("open ambiguity fixture");
        conn.execute_batch(
            "INSERT INTO prompts (id, title, updated_at)
             VALUES
               ('10000000-0000-4000-8000-000000000001', 'Duplicate prompt', NULL),
               ('10000000-0000-4000-8000-000000000002', 'Duplicate prompt', '2026-01-01');
             INSERT INTO playbooks (id, title)
             VALUES
               ('20000000-0000-4000-8000-000000000002', 'Duplicate playbook'),
               ('20000000-0000-4000-8000-000000000001', 'Duplicate playbook');",
        )
        .expect("insert ambiguity fixtures");
    }
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let prompt = client.tool_call("get_prompt", json!({ "id_or_title": "Duplicate prompt" }));
    assert_eq!(
        prompt["result"]["content"][0]["text"],
        "ambiguous title; candidates: [{\"id\":\"10000000-0000-4000-8000-000000000002\",\"title\":\"Duplicate prompt\"},{\"id\":\"10000000-0000-4000-8000-000000000001\",\"title\":\"Duplicate prompt\"}]; call again with an id"
    );

    let playbook = client.tool_call(
        "get_playbook",
        json!({ "id_or_title": "Duplicate playbook" }),
    );
    assert_eq!(
        playbook["result"]["content"][0]["text"],
        "ambiguous title; candidates: [{\"id\":\"20000000-0000-4000-8000-000000000001\",\"title\":\"Duplicate playbook\"},{\"id\":\"20000000-0000-4000-8000-000000000002\",\"title\":\"Duplicate playbook\"}]; call again with an id"
    );
}

#[cfg(not(feature = "test-faults"))]
#[test]
fn shipping_feature_set_ignores_fault_environment() {
    let temp = TempDir::new("fault-disabled");
    let database = temp.database("fixture.db");
    create_fixture(&database, true);
    let mut client = WireClient::spawn_with_fault(&database, false, Some("panic_in_tool"));
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.tool_call("get_prompt", json!({ "id_or_title": FIXTURE_PROMPT_ID }));
    assert_eq!(response["result"]["isError"], false);
}

fn prompt_count(path: &Path) -> i64 {
    Connection::open(path)
        .expect("open database for count")
        .query_row("SELECT COUNT(*) FROM prompts", [], |row| row.get(0))
        .expect("count prompts")
}
