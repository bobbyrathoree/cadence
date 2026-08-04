use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Barrier, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cadence_core::db::{schema, Db, Health, CURRENT_SCHEMA_VERSION};
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::prompt_service;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const LEGACY_VERSION: &str = "2025-11-25";
const CURRENT_VERSION: &str = "2026-07-28";
const FIXTURE_PROMPT_ID: &str = "11111111-1111-4111-8111-111111111111";
const FIXTURE_VARIANT_ID: &str = "22222222-2222-4222-8222-222222222222";
const EMPTY_PROMPT_ID: &str = "30000000-0000-4000-8000-000000000001";
const HOSTILE_PROMPT_ID: &str = "40000000-0000-4000-8000-000000000001";
const GOLDEN_PLAYBOOK_ID: &str = "50000000-0000-4000-8000-000000000001";
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

#[allow(clippy::too_many_arguments)]
fn insert_prompt(
    conn: &Connection,
    id: &str,
    variant_id: &str,
    title: &str,
    description: Option<&str>,
    content: &str,
    label: &str,
    favorite: bool,
    pinned: bool,
) {
    conn.execute(
        "INSERT INTO prompts
            (id, title, description, primary_variant_id, is_favorite, is_pinned,
             copy_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, 0, '2026-08-03T12:00:00Z',
                 '2026-08-03T12:00:00Z')",
        params![id, title, description, favorite as i64, pinned as i64],
    )
    .expect("insert prompt fixture");
    conn.execute(
        "INSERT INTO variants
            (id, prompt_id, label, content, content_type, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'static', 0, '2026-08-03T12:00:00Z',
                 '2026-08-03T12:00:00Z')",
        params![variant_id, id, label, content],
    )
    .expect("insert variant fixture");
    conn.execute(
        "UPDATE prompts SET primary_variant_id = ?1 WHERE id = ?2",
        params![variant_id, id],
    )
    .expect("set fixture primary variant");
}

fn create_prompt_catalog_fixture(path: &Path) {
    create_fixture(path, false);
    let conn = Connection::open(path).expect("open prompt catalog fixture");
    insert_prompt(
        &conn,
        EMPTY_PROMPT_ID,
        "31000000-0000-4000-8000-000000000001",
        "",
        None,
        "Hello {{who}} [LEGACY]",
        "Primary",
        true,
        false,
    );
    insert_prompt(
        &conn,
        "30000000-0000-4000-8000-000000000002",
        "31000000-0000-4000-8000-000000000002",
        "Duplicate",
        None,
        "No variables",
        "Primary",
        false,
        true,
    );
    insert_prompt(
        &conn,
        "30000000-0000-4000-8000-000000000003",
        "31000000-0000-4000-8000-000000000003",
        "Duplicate",
        None,
        "{{first}} then {{second}} then {{first}}",
        "Primary",
        true,
        false,
    );
}

fn create_resource_fixture(path: &Path) {
    create_fixture(path, false);
    let conn = Connection::open(path).expect("open resource fixture");
    insert_prompt(
        &conn,
        HOSTILE_PROMPT_ID,
        "41000000-0000-4000-8000-000000000001",
        "Hostile # title ```",
        Some("Line one\r\nLine two\rLine three"),
        "before ````` after\r\nnext",
        "Primary",
        false,
        true,
    );
    insert_prompt(
        &conn,
        "40000000-0000-4000-8000-000000000002",
        "41000000-0000-4000-8000-000000000002",
        "Favorite but unpinned",
        None,
        "Not a resource",
        "Primary",
        true,
        false,
    );
    for (id, variant_id, title) in [
        (
            "42000000-0000-4000-8000-000000000001",
            "43000000-0000-4000-8000-000000000001",
            "Alpha",
        ),
        (
            "42000000-0000-4000-8000-000000000002",
            "43000000-0000-4000-8000-000000000002",
            "Beta",
        ),
        (
            "42000000-0000-4000-8000-000000000003",
            "43000000-0000-4000-8000-000000000003",
            "Deleted",
        ),
    ] {
        insert_prompt(
            &conn, id, variant_id, title, None, title, "Primary", false, false,
        );
    }
    conn.execute(
        "UPDATE prompts SET deleted_at = '2026-08-03T13:00:00Z'
         WHERE id = '42000000-0000-4000-8000-000000000003'",
        [],
    )
    .expect("soft-delete partial choice prompt");
    conn.execute(
        "INSERT INTO playbooks (id, title, description)
         VALUES (?1, 'Golden playbook', 'Guide\r\nNow')",
        params![GOLDEN_PLAYBOOK_ID],
    )
    .expect("insert golden playbook");
    conn.execute_batch(
        "INSERT INTO playbook_steps
            (id, playbook_id, prompt_id, position, step_type, instructions, choice_prompt_ids)
         VALUES
            ('51000000-0000-4000-8000-000000000001',
             '50000000-0000-4000-8000-000000000001',
             '42000000-0000-4000-8000-000000000003', 0, 'single', 'Do\r\nthis', NULL),
            ('51000000-0000-4000-8000-000000000002',
             '50000000-0000-4000-8000-000000000001',
             NULL, 1, 'choice', NULL,
             '42000000-0000-4000-8000-000000000001,42000000-0000-4000-8000-000000000002'),
            ('51000000-0000-4000-8000-000000000003',
             '50000000-0000-4000-8000-000000000001',
             NULL, 2, 'choice', NULL,
             '42000000-0000-4000-8000-000000000001,42000000-0000-4000-8000-000000000003');",
    )
    .expect("insert golden playbook steps");
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

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read hand-authored golden {}: {error}", path.display()))
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

#[test]
fn prompt_catalog_and_get_follow_parser_and_stable_name_contracts() {
    for version in [LEGACY_VERSION, CURRENT_VERSION] {
        let temp = TempDir::new("prompt-catalog");
        let database = temp.database("fixture.db");
        create_prompt_catalog_fixture(&database);
        let mut client = WireClient::spawn(&database, false);
        assert_handshake(&client.initialize(version), version);

        let list = client.request("prompts/list", None, READ_TIMEOUT);
        snapshot(&format!("prompts-list-{version}.json"), &list["result"]);
        let name = list["result"]["prompts"][0]["name"]
            .as_str()
            .expect("prompt name");
        let get = client.request(
            "prompts/get",
            Some(json!({
                "name": name,
                "arguments": {
                    "who": "Ada",
                    "unknown": "ignored"
                }
            })),
            READ_TIMEOUT,
        );
        assert_eq!(
            get["result"]["messages"][0],
            json!({
                "role": "user",
                "content": { "type": "text", "text": "Hello Ada [LEGACY]" }
            })
        );
        if version == LEGACY_VERSION {
            assert!(get["result"].get("resultType").is_none());
        } else {
            assert_eq!(get["result"]["resultType"], "complete");
        }
    }
}

#[test]
fn prompt_arguments_coerce_only_strings_numbers_and_booleans() {
    let temp = TempDir::new("prompt-arguments");
    let database = temp.database("fixture.db");
    create_fixture(&database, false);
    let conn = Connection::open(&database).expect("open argument fixture");
    let id = "32000000-0000-4000-8000-000000000001";
    insert_prompt(
        &conn,
        id,
        "32100000-0000-4000-8000-000000000001",
        "Types",
        None,
        "{{s}}|{{n}}|{{b}}|{{nil}}|{{arr}}|{{obj}}|{{missing}}|{{empty}}",
        "Primary",
        true,
        false,
    );
    drop(conn);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.request(
        "prompts/get",
        Some(json!({
            "name": format!("any-prefix-{id}"),
            "arguments": {
                "s": "$&",
                "n": 12.5,
                "b": true,
                "nil": null,
                "arr": ["ignored"],
                "obj": { "ignored": true },
                "empty": "",
                "unknown": "ignored"
            }
        })),
        READ_TIMEOUT,
    );
    assert_eq!(
        response["result"]["messages"][0]["content"]["text"],
        "$&|12.5|true|{{nil}}|{{arr}}|{{obj}}|{{missing}}|{{empty}}"
    );
}

#[test]
fn prompt_catalog_cap_and_uuid_tail_resolution_hold_at_boundaries() {
    for count in [99_u32, 100, 101] {
        let temp = TempDir::new("prompt-cap");
        let database = temp.database("fixture.db");
        create_fixture(&database, false);
        let conn = Connection::open(&database).expect("open cap fixture");
        for index in 0..count {
            let id = format!("60000000-0000-4000-8000-{index:012x}");
            let variant_id = format!("61000000-0000-4000-8000-{index:012x}");
            insert_prompt(
                &conn,
                &id,
                &variant_id,
                &format!("Catalog {index:03}"),
                None,
                &format!("content {index}"),
                "Primary",
                true,
                false,
            );
        }
        drop(conn);
        let mut client = WireClient::spawn(&database, false);
        assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);
        let list = client.request("prompts/list", None, READ_TIMEOUT);
        assert_eq!(
            list["result"]["prompts"]
                .as_array()
                .expect("prompt catalog array")
                .len(),
            usize::try_from(count.min(100)).expect("count fits usize")
        );
        if count == 101 {
            let outside_id = "60000000-0000-4000-8000-000000000064";
            let get = client.request(
                "prompts/get",
                Some(json!({ "name": format!("stale-prefix-{outside_id}") })),
                READ_TIMEOUT,
            );
            assert_eq!(
                get["result"]["messages"][0]["content"]["text"],
                "content 100"
            );
        }
    }
}

#[test]
fn renamed_prompt_keeps_old_name_resolution_and_lists_new_slug() {
    let temp = TempDir::new("prompt-rename");
    let database = temp.database("fixture.db");
    create_fixture(&database, false);
    let id = "33000000-0000-4000-8000-000000000001";
    let conn = Connection::open(&database).expect("open rename fixture");
    insert_prompt(
        &conn,
        id,
        "33100000-0000-4000-8000-000000000001",
        "Old title",
        None,
        "stable content",
        "Primary",
        true,
        false,
    );
    drop(conn);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);
    let old_name = format!("old-title-{id}");

    Connection::open(&database)
        .expect("reopen rename fixture")
        .execute(
            "UPDATE prompts SET title = 'New title' WHERE id = ?1",
            params![id],
        )
        .expect("rename prompt");

    let get = client.request(
        "prompts/get",
        Some(json!({ "name": old_name })),
        READ_TIMEOUT,
    );
    assert_eq!(
        get["result"]["messages"][0]["content"]["text"],
        "stable content"
    );
    let list = client.request("prompts/list", None, READ_TIMEOUT);
    assert_eq!(
        list["result"]["prompts"][0]["name"],
        format!("new-title-{id}")
    );
}

#[test]
fn resource_catalog_templates_and_markdown_match_hand_authored_fixtures() {
    for version in [LEGACY_VERSION, CURRENT_VERSION] {
        let temp = TempDir::new("resource-catalog");
        let database = temp.database("fixture.db");
        create_resource_fixture(&database);
        let mut client = WireClient::spawn(&database, false);
        assert_handshake(&client.initialize(version), version);

        let resources = client.request("resources/list", None, READ_TIMEOUT);
        let templates = client.request("resources/templates/list", None, READ_TIMEOUT);
        snapshot(
            &format!("resources-catalog-{version}.json"),
            &json!({
                "resources_list": resources["result"],
                "templates_list": templates["result"]
            }),
        );

        let prompt_uri = format!("cadence://prompt/{HOSTILE_PROMPT_ID}");
        let prompt = client.request(
            "resources/read",
            Some(json!({ "uri": prompt_uri })),
            READ_TIMEOUT,
        );
        assert_eq!(
            prompt["result"]["contents"][0]["text"],
            golden("hostile-prompt.md")
        );
        assert_eq!(prompt["result"]["contents"][0]["mimeType"], "text/markdown");
        assert_eq!(prompt["result"]["ttlMs"], 0);
        assert_eq!(prompt["result"]["cacheScope"], "private");

        let playbook = client.request(
            "resources/read",
            Some(json!({
                "uri": format!("cadence://playbook/{GOLDEN_PLAYBOOK_ID}")
            })),
            READ_TIMEOUT,
        );
        assert_eq!(
            playbook["result"]["contents"][0]["text"],
            golden("playbook.md")
        );
    }
}

#[test]
fn resource_uri_error_matrix_is_versioned_and_data_free() {
    let malformed = [
        "http://prompt/40000000-0000-4000-8000-000000000001",
        "cadence://other/40000000-0000-4000-8000-000000000001",
        "cadence://prompt/not-a-uuid",
        "cadence://prompt/40000000-0000-4000-8000-000000000001/extra",
        "cadence://prompt/40000000-0000-4000-8000-000000000001?query=1",
    ];
    for version in [LEGACY_VERSION, CURRENT_VERSION] {
        let temp = TempDir::new("resource-errors");
        let database = temp.database("fixture.db");
        create_fixture(&database, false);
        let mut client = WireClient::spawn(&database, false);
        assert_handshake(&client.initialize(version), version);

        let mut representative = Value::Null;
        for uri in malformed {
            let response =
                client.request("resources/read", Some(json!({ "uri": uri })), READ_TIMEOUT);
            assert_eq!(response["error"]["code"], -32602);
            assert_eq!(
                response["error"]["message"],
                format!("unknown resource: {uri}")
            );
            assert!(response["error"].get("data").is_none());
            if uri == "cadence://prompt/not-a-uuid" {
                representative = response["error"].clone();
            }
        }
        let missing_uri = "cadence://prompt/99999999-9999-4999-8999-999999999999";
        let missing = client.request(
            "resources/read",
            Some(json!({ "uri": missing_uri })),
            READ_TIMEOUT,
        );
        assert!(missing["error"].get("data").is_none());
        snapshot(
            &format!("resource-errors-{version}.json"),
            &json!({
                "malformed": representative,
                "missing": missing["error"]
            }),
        );
    }
}

#[test]
fn resource_read_survives_catalog_departure_and_accepts_uppercase_uuid() {
    let temp = TempDir::new("resource-departure");
    let database = temp.database("fixture.db");
    create_resource_fixture(&database);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);
    let before = client.request("resources/list", None, READ_TIMEOUT);
    assert_eq!(
        before["result"]["resources"].as_array().map(Vec::len),
        Some(1)
    );

    Connection::open(&database)
        .expect("open departure fixture")
        .execute(
            "UPDATE prompts SET is_pinned = 0 WHERE id = ?1",
            params![HOSTILE_PROMPT_ID],
        )
        .expect("unpin resource");
    let after = client.request("resources/list", None, READ_TIMEOUT);
    assert_eq!(after["result"]["resources"], json!([]));

    let read = client.request(
        "resources/read",
        Some(json!({
            "uri": format!("cadence://prompt/{}", HOSTILE_PROMPT_ID.to_uppercase())
        })),
        READ_TIMEOUT,
    );
    assert_eq!(
        read["result"]["contents"][0]["text"],
        golden("hostile-prompt.md")
    );
}

#[test]
fn completion_round_trips_and_treats_prefixes_literally() {
    for version in [LEGACY_VERSION, CURRENT_VERSION] {
        let temp = TempDir::new("completion");
        let database = temp.database("fixture.db");
        create_resource_fixture(&database);
        let conn = Connection::open(&database).expect("open completion fixture");
        for (index, title) in ["% Percent", "_ Under", r"\ Slash"].into_iter().enumerate() {
            let id = format!("70000000-0000-4000-8000-{index:012x}");
            let variant_id = format!("71000000-0000-4000-8000-{index:012x}");
            insert_prompt(
                &conn,
                &id,
                &variant_id,
                title,
                None,
                title,
                "Primary",
                false,
                false,
            );
        }
        for index in 0..11_u32 {
            let id = format!("72000000-0000-4000-8000-{index:012x}");
            let variant_id = format!("73000000-0000-4000-8000-{index:012x}");
            insert_prompt(
                &conn,
                &id,
                &variant_id,
                &format!("Many {index:02}"),
                None,
                "many",
                "Primary",
                false,
                false,
            );
        }
        for (index, title) in ["% Playbook", "_ Playbook", r"\ Playbook"]
            .into_iter()
            .enumerate()
        {
            conn.execute(
                "INSERT INTO playbooks (id, title) VALUES (?1, ?2)",
                params![format!("74000000-0000-4000-8000-{index:012x}"), title],
            )
            .expect("insert hostile-prefix playbook");
        }
        drop(conn);
        let mut client = WireClient::spawn(&database, false);
        assert_handshake(&client.initialize(version), version);
        let templates = client.request("resources/templates/list", None, READ_TIMEOUT);
        let prompt_template = templates["result"]["resourceTemplates"][0]["uriTemplate"]
            .as_str()
            .expect("prompt template URI");
        let playbook_template = templates["result"]["resourceTemplates"][1]["uriTemplate"]
            .as_str()
            .expect("playbook template URI");

        let complete = |client: &mut WireClient, template: &str, prefix: &str, context: bool| {
            let mut params = json!({
                "ref": { "type": "ref/resource", "uri": template },
                "argument": { "name": "id", "value": prefix }
            });
            if context {
                params["context"] = json!({ "arguments": { "ignored": "value" } });
            }
            client.request("completion/complete", Some(params), READ_TIMEOUT)
        };
        let round_trip = complete(&mut client, prompt_template, "Hostile", true);
        assert_eq!(
            round_trip["result"]["completion"]["values"],
            json!([HOSTILE_PROMPT_ID])
        );
        assert!(round_trip["result"]["completion"].get("total").is_none());
        assert_eq!(round_trip["result"]["completion"]["hasMore"], false);
        if version == LEGACY_VERSION {
            assert!(round_trip["result"].get("resultType").is_none());
        } else {
            assert_eq!(round_trip["result"]["resultType"], "complete");
        }
        let read = client.request(
            "resources/read",
            Some(json!({
                "uri": format!(
                    "cadence://prompt/{}",
                    round_trip["result"]["completion"]["values"][0]
                        .as_str()
                        .expect("completed UUID")
                )
            })),
            READ_TIMEOUT,
        );
        assert_eq!(
            read["result"]["contents"][0]["text"],
            golden("hostile-prompt.md")
        );

        for (prefix, expected) in [
            ("%", "70000000-0000-4000-8000-000000000000"),
            ("_", "70000000-0000-4000-8000-000000000001"),
            (r"\", "70000000-0000-4000-8000-000000000002"),
        ] {
            assert_eq!(
                complete(&mut client, prompt_template, prefix, false)["result"]["completion"]
                    ["values"],
                json!([expected])
            );
        }
        let playbook_round_trip = complete(&mut client, playbook_template, "Golden", true);
        assert_eq!(
            playbook_round_trip["result"]["completion"]["values"],
            json!([GOLDEN_PLAYBOOK_ID])
        );
        let playbook_read = client.request(
            "resources/read",
            Some(json!({
                "uri": format!("cadence://playbook/{GOLDEN_PLAYBOOK_ID}")
            })),
            READ_TIMEOUT,
        );
        assert_eq!(
            playbook_read["result"]["contents"][0]["text"],
            golden("playbook.md")
        );
        for (prefix, expected) in [
            ("%", "74000000-0000-4000-8000-000000000000"),
            ("_", "74000000-0000-4000-8000-000000000001"),
            (r"\", "74000000-0000-4000-8000-000000000002"),
        ] {
            assert_eq!(
                complete(&mut client, playbook_template, prefix, false)["result"]["completion"]
                    ["values"],
                json!([expected])
            );
        }

        let many = complete(&mut client, prompt_template, "Many", false);
        assert_eq!(
            many["result"]["completion"]["values"]
                .as_array()
                .map(Vec::len),
            Some(10)
        );
        assert_eq!(many["result"]["completion"]["hasMore"], true);

        let wrong_argument = client.request(
            "completion/complete",
            Some(json!({
                "ref": { "type": "ref/resource", "uri": prompt_template },
                "argument": { "name": "other", "value": "ignored" }
            })),
            READ_TIMEOUT,
        );
        assert_eq!(
            wrong_argument["result"]["completion"],
            json!({ "values": [], "hasMore": false })
        );
        let unknown = client.request(
            "completion/complete",
            Some(json!({
                "ref": { "type": "ref/resource", "uri": "cadence://unknown/{id}" },
                "argument": { "name": "id", "value": "" }
            })),
            READ_TIMEOUT,
        );
        assert_eq!(unknown["error"]["code"], -32602);
        assert_eq!(unknown["error"]["message"], "unknown completion target");
    }
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

fn structured(response: &Value) -> &Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["isError"], false, "{response}");
    &response["result"]["structuredContent"]
}

#[test]
fn record_copy_returns_content_and_persists_usage() {
    let temp = TempDir::new("record-copy");
    let database = temp.database("fixture.db");
    create_fixture(&database, true);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.tool_call(
        "record_copy",
        json!({ "prompt_id": FIXTURE_PROMPT_ID, "variant_id": null }),
    );
    assert_eq!(
        structured(&response),
        &json!({ "content": "Fixture content" })
    );

    let conn = Connection::open(&database).expect("inspect record-copy fixture");
    assert_eq!(
        conn.query_row(
            "SELECT copy_count FROM prompts WHERE id = ?1",
            [FIXTURE_PROMPT_ID],
            |row| row.get::<_, i64>(0),
        )
        .expect("read copy count"),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM copy_history
             WHERE prompt_id = ?1 AND variant_id = ?2",
            params![FIXTURE_PROMPT_ID, FIXTURE_VARIANT_ID],
            |row| row.get::<_, i64>(0),
        )
        .expect("count copy history"),
        1
    );
}

#[test]
fn update_prompt_content_targets_primary_and_explicit_variants() {
    let temp = TempDir::new("update-content");
    let database = temp.database("fixture.db");
    create_fixture(&database, true);
    let alternate_id = "22222222-2222-4222-8222-222222222223";
    Connection::open(&database)
        .expect("open update fixture")
        .execute(
            "INSERT INTO variants
                (id, prompt_id, label, content, content_type, sort_order)
             VALUES (?1, ?2, 'Alternate', 'Alternate content', 'static', 1)",
            params![alternate_id, FIXTURE_PROMPT_ID],
        )
        .expect("insert alternate variant");
    let mut client = WireClient::spawn(&database, true);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let primary = client.tool_call(
        "update_prompt_content",
        json!({
            "prompt_id": FIXTURE_PROMPT_ID,
            "variant_id": null,
            "content": "Primary replacement"
        }),
    );
    let primary_variants = structured(&primary)["variants"]
        .as_array()
        .expect("primary update variants");
    assert!(primary_variants.iter().any(|variant| {
        variant["id"] == FIXTURE_VARIANT_ID
            && variant["content"] == "Primary replacement"
            && variant["is_primary"] == true
    }));

    let alternate = client.tool_call(
        "update_prompt_content",
        json!({
            "prompt_id": FIXTURE_PROMPT_ID,
            "variant_id": alternate_id,
            "content": "Alternate replacement"
        }),
    );
    let alternate_variants = structured(&alternate)["variants"]
        .as_array()
        .expect("explicit update variants");
    assert!(alternate_variants.iter().any(|variant| {
        variant["id"] == alternate_id
            && variant["content"] == "Alternate replacement"
            && variant["is_primary"] == false
    }));
}

#[test]
fn list_prompts_enforces_page_boundaries_clamps_and_validation() {
    let temp = TempDir::new("list-prompts");
    let database = temp.database("fixture.db");
    create_fixture(&database, false);
    let conn = Connection::open(&database).expect("open list fixture");
    for index in 0..101_u32 {
        insert_prompt(
            &conn,
            &format!("80000000-0000-4000-8000-{index:012x}"),
            &format!("81000000-0000-4000-8000-{index:012x}"),
            &format!("Prompt {index:03}"),
            None,
            "content",
            "Primary",
            index % 2 == 0,
            false,
        );
    }
    drop(conn);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let limit_plus_one = client.tool_call(
        "list_prompts",
        json!({ "filter": "all", "limit": 2, "offset": 98 }),
    );
    assert_eq!(
        structured(&limit_plus_one)["prompts"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(structured(&limit_plus_one)["next_offset"], 100);

    let exact_limit = client.tool_call(
        "list_prompts",
        json!({ "filter": "all", "limit": 2, "offset": 99 }),
    );
    assert_eq!(
        structured(&exact_limit)["prompts"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(structured(&exact_limit)["next_offset"], Value::Null);

    let clamped_page = client.tool_call(
        "list_prompts",
        json!({ "filter": "all", "limit": 101, "offset": 0 }),
    );
    assert_eq!(
        structured(&clamped_page)["prompts"]
            .as_array()
            .map(Vec::len),
        Some(100)
    );
    assert_eq!(structured(&clamped_page)["next_offset"], 100);

    let last_page = client.tool_call(
        "list_prompts",
        json!({ "filter": "all", "limit": 100, "offset": 1 }),
    );
    assert_eq!(
        structured(&last_page)["prompts"].as_array().map(Vec::len),
        Some(100)
    );
    assert_eq!(structured(&last_page)["next_offset"], Value::Null);

    let clamped_low = client.tool_call(
        "list_prompts",
        json!({ "filter": "favorites", "limit": 0, "offset": 0 }),
    );
    assert_eq!(
        structured(&clamped_low)["prompts"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(structured(&clamped_low)["next_offset"], 1);

    let clamped_high = client.tool_call(
        "list_prompts",
        json!({ "filter": "all", "limit": 500, "offset": 0 }),
    );
    assert_eq!(
        structured(&clamped_high)["prompts"]
            .as_array()
            .map(Vec::len),
        Some(100)
    );
    assert_eq!(structured(&clamped_high)["next_offset"], 100);

    let overflow = client.tool_call(
        "list_prompts",
        json!({ "filter": "all", "limit": 1, "offset": u32::MAX }),
    );
    assert_eq!(overflow["result"]["isError"], true);
    assert_eq!(
        overflow["result"]["content"][0]["text"],
        "offset plus limit is too large"
    );

    let bogus = client.tool_call(
        "list_prompts",
        json!({ "filter": "bogus", "limit": 10, "offset": 0 }),
    );
    assert_eq!(bogus["result"]["isError"], true);
    assert_eq!(
        bogus["result"]["content"][0]["text"],
        "filter must be one of: all, favorites, recent"
    );
}

#[test]
fn search_prompts_returns_success_shape_and_paginates() {
    let temp = TempDir::new("search-prompts");
    let database = temp.database("fixture.db");
    create_fixture(&database, false);
    let conn = Connection::open(&database).expect("open search fixture");
    let mut db = Db {
        conn,
        health: Health::recording().0,
    };
    for index in 0..3 {
        prompt_service::create_prompt(
            &mut db,
            CreatePromptRequest {
                title: format!("Search {index}"),
                description: None,
                content: format!("shared needle content {index}"),
                variant_label: None,
                tags: Vec::new(),
                is_favorite: false,
            },
        )
        .expect("create searchable prompt");
    }
    drop(db);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.tool_call(
        "search_prompts",
        json!({ "query": "shared needle", "limit": 2, "offset": 0 }),
    );
    let page = structured(&response);
    assert_eq!(page["prompts"].as_array().map(Vec::len), Some(2));
    assert_eq!(page["next_offset"], 2);
    assert!(page["prompts"]
        .as_array()
        .expect("search rows")
        .iter()
        .all(|prompt| prompt["snippet"]
            .as_str()
            .is_some_and(|snippet| snippet.contains("shared needle"))));
}

#[test]
fn list_playbooks_returns_ordered_success_shape() {
    let temp = TempDir::new("list-playbooks");
    let database = temp.database("fixture.db");
    create_fixture(&database, false);
    Connection::open(&database)
        .expect("open playbook fixture")
        .execute_batch(
            "INSERT INTO playbooks (id, title, description) VALUES
                ('playbook-b', 'Beta', NULL),
                ('playbook-a', 'Alpha', 'description');
             INSERT INTO playbook_steps
                (id, playbook_id, position, step_type)
             VALUES
                ('step-1', 'playbook-b', 0, 'single'),
                ('step-2', 'playbook-b', 1, 'single');",
        )
        .expect("insert playbook fixtures");
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.tool_call("list_playbooks", json!({}));
    assert_eq!(
        structured(&response),
        &json!({
            "playbooks": [
                {
                    "id": "playbook-a",
                    "title": "Alpha",
                    "description": "description",
                    "step_count": 0
                },
                {
                    "id": "playbook-b",
                    "title": "Beta",
                    "description": null,
                    "step_count": 2
                }
            ]
        })
    );
}

#[test]
fn list_tags_returns_ordered_success_shape_with_live_counts() {
    let temp = TempDir::new("list-tags");
    let database = temp.database("fixture.db");
    create_fixture(&database, true);
    let conn = Connection::open(&database).expect("open tag fixture");
    conn.execute_batch(
        "INSERT INTO tags (id, name, color) VALUES
            ('tag-z', 'Zulu', NULL),
            ('tag-a', 'Alpha', '#fff');",
    )
    .expect("insert tags");
    conn.execute(
        "INSERT INTO prompt_tags (prompt_id, tag_id) VALUES
            (?1, 'tag-a'), (?1, 'tag-z')",
        [FIXTURE_PROMPT_ID],
    )
    .expect("attach tags");
    drop(conn);
    let mut client = WireClient::spawn(&database, false);
    assert_handshake(&client.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let response = client.tool_call("list_tags", json!({}));
    assert_eq!(
        structured(&response),
        &json!({
            "tags": [
                { "name": "Alpha", "color": "#fff", "prompt_count": 1 },
                { "name": "Zulu", "color": null, "prompt_count": 1 }
            ]
        })
    );
}

#[test]
fn two_children_complete_concurrent_writes_without_busy_errors() {
    let temp = TempDir::new("two-children");
    let database = temp.database("fixture.db");
    create_fixture(&database, true);
    let mut first = WireClient::spawn(&database, false);
    let mut second = WireClient::spawn(&database, false);
    assert_handshake(&first.initialize(CURRENT_VERSION), CURRENT_VERSION);
    assert_handshake(&second.initialize(CURRENT_VERSION), CURRENT_VERSION);

    let lock = Connection::open(&database).expect("open contention lock");
    lock.execute_batch("BEGIN IMMEDIATE")
        .expect("hold external write lock");
    let start = Arc::new(Barrier::new(3));
    let (first_result, second_result) = std::thread::scope(|scope| {
        let first_start = start.clone();
        let first_client = &mut first;
        let first_call = scope.spawn(move || {
            first_start.wait();
            first_client.tool_call(
                "record_copy",
                json!({ "prompt_id": FIXTURE_PROMPT_ID, "variant_id": null }),
            )
        });
        let second_start = start.clone();
        let second_client = &mut second;
        let second_call = scope.spawn(move || {
            second_start.wait();
            second_client.tool_call(
                "record_copy",
                json!({ "prompt_id": FIXTURE_PROMPT_ID, "variant_id": null }),
            )
        });
        start.wait();
        std::thread::sleep(Duration::from_millis(5_500));
        lock.execute_batch("COMMIT")
            .expect("release external write lock");
        (
            first_call.join().expect("join first MCP write"),
            second_call.join().expect("join second MCP write"),
        )
    });

    assert_eq!(
        structured(&first_result),
        &json!({ "content": "Fixture content" })
    );
    assert_eq!(
        structured(&second_result),
        &json!({ "content": "Fixture content" })
    );
    drop(first);
    drop(second);
    let conn = Connection::open(&database).expect("inspect concurrent writes");
    assert_eq!(
        conn.query_row(
            "SELECT copy_count FROM prompts WHERE id = ?1",
            [FIXTURE_PROMPT_ID],
            |row| row.get::<_, i64>(0),
        )
        .expect("read concurrent copy count"),
        2
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM copy_history WHERE prompt_id = ?1",
            [FIXTURE_PROMPT_ID],
            |row| row.get::<_, i64>(0),
        )
        .expect("count concurrent history"),
        2
    );
}
