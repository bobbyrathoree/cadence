use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cadence_lib::api::lifecycle::{ApiLifecycle, DiscoveryFile};
use cadence_lib::db;
use cadence_lib::services::settings_service;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

static API_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TempDatabase {
    dir: PathBuf,
    database: PathBuf,
    discovery: PathBuf,
}

impl TempDatabase {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "cadence-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self {
            database: dir.join("cadence.db"),
            discovery: dir.join("api.json"),
            dir,
        }
    }

    fn connection(&self) -> Mutex<rusqlite::Connection> {
        Mutex::new(db::connect(&self.database).unwrap())
    }

    fn lifecycle(&self) -> ApiLifecycle {
        ApiLifecycle::new(self.database.clone(), self.discovery.clone())
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enable_publishes_private_discovery_and_serves_requests() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-enable");
    let settings_db = temp.connection();
    let mut lifecycle = temp.lifecycle();

    let status = lifecycle.set_enabled(&settings_db, true).await.unwrap();
    let port = status.port.unwrap();
    let discovery = read_discovery(&temp.discovery);
    assert_eq!(discovery.port, port);
    assert!(discovery.key.starts_with("cad_"));
    assert_eq!(hex::decode(&discovery.key[4..]).unwrap().len(), 32);
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&temp.discovery).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(settings_service::get_api_enabled(&settings_db.lock().unwrap()).unwrap());

    let response = request(port, "GET", "/api/v1/prompts", Some(&discovery.key), "").await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

    lifecycle.shutdown_on_exit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_write_failure_rolls_back_enablement_and_listener() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-discovery-failure");
    let settings_db = temp.connection();
    let blocker = temp.dir.join("not-a-directory");
    fs::write(&blocker, b"block").unwrap();
    let discovery = blocker.join("api.json");
    let mut lifecycle = ApiLifecycle::new(temp.database.clone(), discovery.clone());

    assert!(lifecycle.set_enabled(&settings_db, true).await.is_err());
    assert!(!settings_service::get_api_enabled(&settings_db.lock().unwrap()).unwrap());
    assert!(!discovery.exists());
    assert_port_closed(lifecycle.last_bound_port().unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn final_settings_failure_removes_discovery_and_closes_listener() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-settings-failure");
    let settings_db = temp.connection();
    settings_db
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_api_enable
             BEFORE INSERT ON settings
             WHEN NEW.key = 'api_enabled' AND NEW.value = 'true'
             BEGIN
               SELECT RAISE(ABORT, 'injected settings failure');
             END;",
        )
        .unwrap();
    let mut lifecycle = temp.lifecycle();

    assert!(lifecycle.set_enabled(&settings_db, true).await.is_err());
    assert!(!settings_service::get_api_enabled(&settings_db.lock().unwrap()).unwrap());
    assert!(!temp.discovery.exists());
    assert_port_closed(lifecycle.last_bound_port().unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_awaits_listener_close_and_removes_discovery() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-disable");
    let settings_db = temp.connection();
    let mut lifecycle = temp.lifecycle();
    let port = lifecycle
        .set_enabled(&settings_db, true)
        .await
        .unwrap()
        .port
        .unwrap();

    let status = lifecycle.set_enabled(&settings_db, false).await.unwrap();
    assert_eq!(
        status,
        cadence_lib::api::lifecycle::ApiStatus {
            enabled: false,
            port: None
        }
    );
    assert!(!settings_service::get_api_enabled(&settings_db.lock().unwrap()).unwrap());
    assert!(!temp.discovery.exists());
    assert_port_closed(port).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_startup_removes_stale_discovery() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-stale");
    let settings_db = temp.connection();
    fs::write(&temp.discovery, br#"{"port": 9, "key": "stale"}"#).unwrap();
    let mut lifecycle = temp.lifecycle();

    let status = lifecycle.startup(&settings_db).await.unwrap();
    assert!(!status.enabled);
    assert!(!temp.discovery.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_enabled_startup_flips_setting_off() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-startup-failure");
    let settings_db = temp.connection();
    settings_service::set_api_enabled(&mut settings_db.lock().unwrap(), true).unwrap();
    let blocker = temp.dir.join("startup-blocker");
    fs::write(&blocker, b"block").unwrap();
    let mut lifecycle = ApiLifecycle::new(temp.database.clone(), blocker.join("api.json"));

    assert!(lifecycle.startup(&settings_db).await.is_err());
    assert!(!settings_service::get_api_enabled(&settings_db.lock().unwrap()).unwrap());
    assert_port_closed(lifecycle.last_bound_port().unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_awaits_listener_close_and_removes_discovery() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-exit");
    let settings_db = temp.connection();
    let mut lifecycle = temp.lifecycle();
    let port = lifecycle
        .set_enabled(&settings_db, true)
        .await
        .unwrap()
        .port
        .unwrap();

    lifecycle.shutdown_on_exit().await.unwrap();
    assert!(!temp.discovery.exists());
    assert_port_closed(port).await;
}

fn read_discovery(path: &Path) -> DiscoveryFile {
    let mut payload = String::new();
    fs::File::open(path)
        .unwrap()
        .read_to_string(&mut payload)
        .unwrap();
    serde_json::from_str(&payload).unwrap()
}

async fn request(port: u16, method: &str, path: &str, key: Option<&str>, body: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let authorization = key
        .map(|key| format!("Authorization: Bearer {key}\r\n"))
        .unwrap_or_default();
    let payload = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         {authorization}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(payload.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8(response).unwrap()
}

async fn assert_port_closed(port: u16) {
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}
