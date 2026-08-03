use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cadence_core::db::{migrate, open_app, schema, DbOpen, Health, CURRENT_SCHEMA_VERSION};
use cadence_core::models::prompt::CreatePromptRequest;
use cadence_core::services::prompt_service;
use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2026-07-28";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const EXPECTED_TOOLS: [&str; 7] = [
    "get_playbook",
    "get_prompt",
    "list_playbooks",
    "list_prompts",
    "list_tags",
    "record_copy",
    "search_prompts",
];

fn main() {
    if let Err(error) = run() {
        eprintln!("cadence-mcp-smoke: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let binary = args
        .next()
        .ok_or_else(|| "usage: cadence-mcp-smoke <binary_path> <db_path>".to_string())?;
    let database = args
        .next()
        .ok_or_else(|| "usage: cadence-mcp-smoke <binary_path> <db_path>".to_string())?;
    if args.next().is_some() {
        return Err("usage: cadence-mcp-smoke <binary_path> <db_path>".to_string());
    }

    let database = Path::new(&database);
    if !database.exists() {
        bootstrap_fixture(database)?;
    }

    let mut server = ServerProcess::spawn(Path::new(&binary), database)?;
    let initialize = server.request(
        1,
        "initialize",
        Some(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "cadence-mcp-smoke",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
    )?;
    let result = initialize
        .get("result")
        .ok_or_else(|| format!("initialize failed: {initialize}"))?;
    if result["serverInfo"]["name"] != "cadence-mcp" {
        return Err(format!(
            "unexpected server name: {}",
            result["serverInfo"]["name"]
        ));
    }
    if result["serverInfo"]["version"] != env!("CARGO_PKG_VERSION") {
        return Err(format!(
            "server version {} does not match smoke version {}",
            result["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        ));
    }

    server.notify("notifications/initialized", None)?;
    let tools = server.request(2, "tools/list", None)?;
    let listed = tools["result"]["tools"]
        .as_array()
        .ok_or_else(|| format!("tools/list returned no tool array: {tools}"))?
        .iter()
        .map(|tool| {
            tool["name"]
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("tool has no string name: {tool}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if listed != EXPECTED_TOOLS {
        return Err(format!(
            "unexpected ungated tool set: expected {:?}, got {listed:?}",
            EXPECTED_TOOLS
        ));
    }

    server.close_stdin();
    let status = server.wait_timeout(EXIT_TIMEOUT)?;
    if !status.success() {
        return Err(format!(
            "server exited with {status}; stderr: {}",
            server.stderr_text()
        ));
    }
    Ok(())
}

fn bootstrap_fixture(path: &Path) -> Result<(), String> {
    let mut db = match open_app(path, Health::exit_process(1))
        .map_err(|error| format!("could not create fixture database: {error}"))?
    {
        DbOpen::NeedsMigration(db, 0) => db,
        DbOpen::Ready(_) => {
            return Err("new fixture database was unexpectedly ready".to_string());
        }
        DbOpen::NeedsMigration(_, version) => {
            return Err(format!(
                "new fixture database reported unexpected schema v{version}"
            ));
        }
        DbOpen::SchemaNewer {
            db_version,
            supported,
        } => {
            return Err(format!(
                "new fixture schema v{db_version} is newer than supported v{supported}"
            ));
        }
        DbOpen::MissingFile => {
            return Err("open_app did not create the fixture database".to_string());
        }
    };
    schema::create_tables(&db.conn)
        .map_err(|error| format!("could not create fixture schema: {error}"))?;
    migrate(&mut db.conn).map_err(|error| format!("could not migrate fixture: {error}"))?;

    let schema_version: i64 = db
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("could not verify fixture schema: {error}"))?;
    if schema_version != CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "fixture schema is v{schema_version}, expected v{CURRENT_SCHEMA_VERSION}"
        ));
    }
    let journal_mode: String = db
        .conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|error| format!("could not verify fixture journal mode: {error}"))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(format!(
            "fixture journal mode is {journal_mode}, expected wal"
        ));
    }

    prompt_service::create_prompt(
        &mut db,
        CreatePromptRequest {
            title: "Smoke fixture".to_string(),
            description: None,
            content: "Smoke fixture content".to_string(),
            variant_label: None,
            tags: Vec::new(),
            is_favorite: false,
        },
    )
    .map_err(|error| format!("could not insert fixture prompt: {error:?}"))?;
    drop(db);
    Ok(())
}

struct ServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<Result<String, String>>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_thread: Option<JoinHandle<()>>,
}

impl ServerProcess {
    fn spawn(binary: &Path, database: &Path) -> Result<Self, String> {
        let mut child = Command::new(binary)
            .env("CADENCE_DB_PATH", database)
            .env_remove("CADENCE_MCP_ALLOW_WRITES")
            .env_remove("CADENCE_MCP_FAULT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not spawn {}: {error}", binary.display()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "could not capture server stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "could not capture server stdout".to_string())?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| "could not capture server stderr".to_string())?;

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

        Ok(Self {
            child,
            stdin: Some(stdin),
            lines,
            stdout_thread: Some(stdout_thread),
            stderr,
            stderr_thread: Some(stderr_thread),
        })
    }

    fn request(
        &mut self,
        id: u64,
        method: &str,
        parameters: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method
        });
        if let Some(parameters) = parameters {
            request["params"] = parameters;
        }
        self.write_frame(&request)?;
        let line = self
            .lines
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(|error| format!("timed out waiting for {method}: {error}"))?
            .map_err(|error| format!("failed reading {method} response: {error}"))?;
        let response: Value = serde_json::from_str(&line)
            .map_err(|error| format!("invalid JSON response {line:?}: {error}"))?;
        if response["id"] != id {
            return Err(format!(
                "response id {} did not match request id {id}",
                response["id"]
            ));
        }
        Ok(response)
    }

    fn notify(&mut self, method: &str, parameters: Option<Value>) -> Result<(), String> {
        let mut notification = json!({
            "jsonrpc": "2.0",
            "method": method
        });
        if let Some(parameters) = parameters {
            notification["params"] = parameters;
        }
        self.write_frame(&notification)
    }

    fn write_frame(&mut self, value: &Value) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "server stdin is closed".to_string())?;
        serde_json::to_writer(&mut *stdin, value)
            .map_err(|error| format!("could not serialize JSON-RPC frame: {error}"))?;
        stdin
            .write_all(b"\n")
            .and_then(|_| stdin.flush())
            .map_err(|error| format!("could not write JSON-RPC frame: {error}"))
    }

    fn close_stdin(&mut self) {
        self.stdin.take();
    }

    fn wait_timeout(&mut self, timeout: Duration) -> Result<std::process::ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.join_output_threads();
                    return Ok(status);
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => return Err("server did not exit after stdin closed".to_string()),
                Err(error) => return Err(format!("could not wait for server: {error}")),
            }
        }
    }

    fn stderr_text(&self) -> String {
        self.stderr
            .lock()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_else(|_| "stderr lock poisoned".to_string())
    }

    fn join_output_threads(&mut self) {
        if let Some(thread) = self.stdout_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.stdin.take();
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        self.join_output_threads();
    }
}
