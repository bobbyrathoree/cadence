#![cfg(feature = "test-support")]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cadence_lib::api::lifecycle::DiscoveryFile;
use cadence_lib::services::settings_service;
use rusqlite::Connection;

const PHASE_DEADLINE: Duration = Duration::from_secs(10);
const CRASH_DEADLINE: Duration = Duration::from_secs(5);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cadence-api-fatal-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct HarnessChild {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr: Option<ChildStderr>,
}

impl HarnessChild {
    fn spawn(
        phase: &str,
        database_path: &Path,
        discovery_path: &Path,
    ) -> Result<(Self, u16), String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_api_fatal_harness"))
            .args([phase])
            .arg(database_path)
            .arg(discovery_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "harness stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "harness stdout unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "harness stderr unavailable".to_string())?;
        let mut harness = Self {
            child,
            stdin: Some(stdin),
            stderr: Some(stderr),
        };

        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = ready_tx.send(result);
        });
        let ready = ready_rx
            .recv_timeout(PHASE_DEADLINE)
            .map_err(|_| "timed out waiting for READY".to_string())?
            .map_err(|error| error.to_string())?;
        let port = ready
            .strip_prefix("READY ")
            .and_then(|value| value.trim_end().parse::<u16>().ok())
            .ok_or_else(|| format!("unexpected harness output: {ready:?}"))?;
        if harness
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("harness exited immediately after READY".to_string());
        }
        Ok((harness, port))
    }

    fn send(&mut self, command: &str) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "harness stdin already closed".to_string())?;
        writeln!(stdin, "{command}").map_err(|error| error.to_string())?;
        stdin.flush().map_err(|error| error.to_string())
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if self
                .child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return self.child.wait().map_err(|error| error.to_string());
            }
            if Instant::now() >= deadline {
                return Err("timed out waiting for harness exit".to_string());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn read_stderr(&mut self) -> Result<String, String> {
        let mut output = String::new();
        self.stderr
            .take()
            .ok_or_else(|| "harness stderr already consumed".to_string())?
            .read_to_string(&mut output)
            .map_err(|error| error.to_string())?;
        Ok(output)
    }
}

impl Drop for HarnessChild {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
            }
        }
        let _ = self.child.wait();
    }
}

#[test]
fn crash_and_relaunch_preserve_intent_rotate_key_and_shutdown_cleanly() {
    let temp = TempDir::new();
    let database_path = temp.path.join("cadence.db");
    let discovery_path = temp.path.join("api.json");
    assert!(!discovery_path.exists(), "enable must create api.json");

    let (mut enable, old_port) =
        HarnessChild::spawn("enable", &database_path, &discovery_path).unwrap();
    let old_bytes = fs::read(&discovery_path).unwrap();
    let old_discovery = parse_discovery(&old_bytes);
    assert_eq!(old_discovery.port, old_port);
    assert_status(
        &authenticated_get(old_port, &old_discovery.key).unwrap(),
        200,
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&discovery_path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    enable.send("POISON").unwrap();
    let crash_status = enable.wait_for_exit(CRASH_DEADLINE).unwrap();
    assert_eq!(crash_status.code(), Some(1));
    let crash_stderr = enable.read_stderr().unwrap();
    assert!(
        crash_stderr.contains("cadence: fatal: harness trigger"),
        "{crash_stderr}"
    );
    assert_port_refused(old_port, CRASH_DEADLINE);
    assert_eq!(fs::read(&discovery_path).unwrap(), old_bytes);
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&discovery_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let settings = Connection::open(&database_path).unwrap();
    assert!(settings_service::get_api_enabled(&settings).unwrap());
    drop(settings);

    assert!(
        discovery_path.exists(),
        "startup must replace a pre-existing api.json"
    );
    let (mut startup, new_port) =
        HarnessChild::spawn("startup", &database_path, &discovery_path).unwrap();
    let new_bytes = fs::read(&discovery_path).unwrap();
    let new_discovery = parse_discovery(&new_bytes);
    assert_eq!(new_discovery.port, new_port);
    assert_ne!(new_bytes, old_bytes);
    assert_ne!(new_discovery.key, old_discovery.key);
    assert_status(
        &authenticated_get(new_port, &new_discovery.key).unwrap(),
        200,
    );
    assert_status(
        &authenticated_get(new_port, &old_discovery.key).unwrap(),
        401,
    );

    startup.send("SHUTDOWN").unwrap();
    let shutdown_status = startup.wait_for_exit(PHASE_DEADLINE).unwrap();
    assert_eq!(shutdown_status.code(), Some(0));
    assert!(startup.read_stderr().unwrap().is_empty());
}

fn parse_discovery(bytes: &[u8]) -> DiscoveryFile {
    serde_json::from_slice(bytes).unwrap()
}

fn authenticated_get(port: u16, key: &str) -> std::io::Result<String> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET /api/v1/prompts HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Authorization: Bearer {key}\r\n\
         Connection: close\r\n\r\n"
    )?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn assert_status(response: &str, expected: u16) {
    let prefix = format!("HTTP/1.1 {expected}");
    assert!(response.starts_with(&prefix), "{response}");
}

fn assert_port_refused(port: u16, timeout: Duration) {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let deadline = Instant::now() + timeout;
    loop {
        if std::net::TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err() {
            return;
        }
        assert!(Instant::now() < deadline, "old API port remained open");
        std::thread::sleep(Duration::from_millis(25));
    }
}
