use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use cadence_core::db_access::DbAccess;
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

    fn connection(&self) -> DbAccess {
        let conn = db::connect(&self.database).unwrap();
        conn.pragma_update(None, "user_version", db::CURRENT_SCHEMA_VERSION)
            .unwrap();
        DbAccess::new(db::Db {
            conn,
            health: db::Health::exit_process(1),
        })
    }

    fn lifecycle(&self, main: &DbAccess) -> ApiLifecycle {
        ApiLifecycle::new(self.database.clone(), self.discovery.clone(), main.clone())
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
    let mut lifecycle = temp.lifecycle(&settings_db);

    let status = lifecycle.set_enabled(true).await.unwrap();
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
    assert!(settings_db
        .with_sync(|db| settings_service::get_api_enabled(&db.conn))
        .unwrap());

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
    let mut lifecycle = ApiLifecycle::new(
        temp.database.clone(),
        discovery.clone(),
        settings_db.clone(),
    );

    assert!(lifecycle.set_enabled(true).await.is_err());
    assert!(!settings_db
        .with_sync(|db| settings_service::get_api_enabled(&db.conn))
        .unwrap());
    assert!(!discovery.exists());
    assert_port_closed(lifecycle.last_bound_port().unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn final_settings_failure_removes_discovery_and_closes_listener() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-settings-failure");
    let settings_db = temp.connection();
    settings_db
        .with_sync(|db| {
            db.conn.execute_batch(
                "CREATE TRIGGER fail_api_enable
             BEFORE INSERT ON settings
             WHEN NEW.key = 'api_enabled' AND NEW.value = 'true'
             BEGIN
               SELECT RAISE(ABORT, 'injected settings failure');
             END;",
            )?;
            Ok(())
        })
        .unwrap();
    let mut lifecycle = temp.lifecycle(&settings_db);

    assert!(lifecycle.set_enabled(true).await.is_err());
    assert!(!settings_db
        .with_sync(|db| settings_service::get_api_enabled(&db.conn))
        .unwrap());
    assert!(!temp.discovery.exists());
    assert_port_closed(lifecycle.last_bound_port().unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_awaits_listener_close_and_removes_discovery() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-disable");
    let settings_db = temp.connection();
    let mut lifecycle = temp.lifecycle(&settings_db);
    let port = lifecycle.set_enabled(true).await.unwrap().port.unwrap();

    let status = lifecycle.set_enabled(false).await.unwrap();
    assert_eq!(
        status,
        cadence_lib::api::lifecycle::ApiStatus {
            enabled: false,
            port: None
        }
    );
    assert!(!settings_db
        .with_sync(|db| settings_service::get_api_enabled(&db.conn))
        .unwrap());
    assert!(!temp.discovery.exists());
    assert_port_closed(port).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_startup_removes_stale_discovery() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-stale");
    let settings_db = temp.connection();
    fs::write(&temp.discovery, br#"{"port": 9, "key": "stale"}"#).unwrap();
    let mut lifecycle = temp.lifecycle(&settings_db);

    let status = lifecycle.startup().await.unwrap();
    assert!(!status.enabled);
    assert!(!temp.discovery.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_enabled_startup_flips_setting_off() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-startup-failure");
    let settings_db = temp.connection();
    settings_db
        .with_sync(|db| settings_service::set_api_enabled(db, true))
        .unwrap();
    let blocker = temp.dir.join("startup-blocker");
    fs::write(&blocker, b"block").unwrap();
    let mut lifecycle = ApiLifecycle::new(
        temp.database.clone(),
        blocker.join("api.json"),
        settings_db.clone(),
    );

    assert!(lifecycle.startup().await.is_err());
    assert!(!settings_db
        .with_sync(|db| settings_service::get_api_enabled(&db.conn))
        .unwrap());
    assert_port_closed(lifecycle.last_bound_port().unwrap()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_awaits_listener_close_and_removes_discovery() {
    let _guard = API_TEST_LOCK.lock().await;
    let temp = TempDatabase::new("api-exit");
    let settings_db = temp.connection();
    let mut lifecycle = temp.lifecycle(&settings_db);
    let port = lifecycle.set_enabled(true).await.unwrap().port.unwrap();

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
