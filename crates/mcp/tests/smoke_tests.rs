use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rusqlite::Connection;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "cadence-mcp-smoke-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create smoke temp directory");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn smoke_bootstraps_a_nonexistent_database_and_checks_the_server() {
    let temp = TempDir::new();
    let database = temp.path.join("nested").join("smoke.db");
    assert!(!database.exists());

    let mut child = Command::new(env!("CARGO_BIN_EXE_cadence-mcp-smoke"))
        .arg(env!("CARGO_BIN_EXE_cadence-mcp"))
        .arg(&database)
        .env_remove("CADENCE_MCP_ALLOW_WRITES")
        .env_remove("CADENCE_MCP_FAULT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn smoke binary");
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait().expect("poll smoke binary") {
            Some(status) => break status,
            None if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            None => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .expect("reap timed-out smoke binary");
                panic!(
                    "smoke binary timed out; stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    };
    let output = child
        .wait_with_output()
        .expect("collect completed smoke output");
    assert!(
        status.success(),
        "smoke failed with {status}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(database.exists());

    let conn = Connection::open(&database).expect("open bootstrapped fixture");
    let prompt_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prompts WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .expect("count fixture prompts");
    assert_eq!(prompt_count, 1);
}
