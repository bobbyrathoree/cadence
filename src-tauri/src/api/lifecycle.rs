use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::server::{self, ApiState};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::settings_service;
use cadence_core::db_access::DbAccess;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiStatus {
    pub enabled: bool,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryFile {
    pub port: u16,
    pub key: String,
}

struct RunningApi {
    port: u16,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

pub struct ApiLifecycle {
    database_path: PathBuf,
    discovery_path: PathBuf,
    main: DbAccess,
    running: Option<RunningApi>,
    last_bound_port: Option<u16>,
    app_handle: Option<AppHandle>,
}

impl ApiLifecycle {
    pub fn new(database_path: PathBuf, discovery_path: PathBuf, main: DbAccess) -> Self {
        Self {
            database_path,
            discovery_path,
            main,
            running: None,
            last_bound_port: None,
            app_handle: None,
        }
    }

    pub fn for_application(database_path: PathBuf, main: DbAccess) -> Result<Self, String> {
        let database_dir = database_path
            .parent()
            .ok_or_else(|| "Cadence database path has no parent directory".to_string())?;
        let discovery_path = database_dir.join("api.json");
        Ok(Self::new(database_path, discovery_path, main))
    }

    pub fn status(&self) -> ApiStatus {
        ApiStatus {
            enabled: self.running.is_some(),
            port: self.running.as_ref().map(|runtime| runtime.port),
        }
    }

    pub fn last_bound_port(&self) -> Option<u16> {
        self.last_bound_port
    }

    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    pub async fn set_enabled(&mut self, enabled: bool) -> AppResult<ApiStatus> {
        if enabled {
            let status = self.start().await?;
            let persist_result = self
                .main
                .with_sync(|db| settings_service::set_api_enabled(db, true));
            if let Err(error) = persist_result {
                let _ = self.stop().await;
                return Err(error);
            }
            Ok(status)
        } else {
            self.stop().await?;
            self.main
                .with_sync(|db| settings_service::set_api_enabled(db, false))?;
            Ok(self.status())
        }
    }

    pub async fn startup(&mut self) -> AppResult<ApiStatus> {
        self.remove_discovery_file()?;
        let enabled = self
            .main
            .with_sync(|db| settings_service::get_api_enabled(&db.conn))?;
        if !enabled {
            return Ok(self.status());
        }

        match self.start().await {
            Ok(status) => Ok(status),
            Err(error) => {
                let persist_result = self
                    .main
                    .with_sync(|db| settings_service::set_api_enabled(db, false));
                if persist_result.is_err() {
                    return Err(AppError::internal(
                        "API startup and settings recovery both failed",
                    ));
                }
                Err(error)
            }
        }
    }

    pub async fn shutdown_on_exit(&mut self) -> AppResult<()> {
        self.stop().await
    }

    async fn start(&mut self) -> AppResult<ApiStatus> {
        if self.running.is_some() {
            return Ok(self.status());
        }

        let api_database = match db::open_peer(&self.database_path, db::Health::exit_process(1)) {
            Ok(db::DbOpen::Ready(database)) => database,
            Ok(db::DbOpen::MissingFile) => {
                return Err(AppError::invalid("database file missing; restart Cadence"));
            }
            Ok(db::DbOpen::NeedsMigration(_, _)) => {
                return Err(AppError::invalid(
                    "database schema changed; restart Cadence",
                ));
            }
            Ok(db::DbOpen::SchemaNewer { .. }) => {
                return Err(AppError::invalid(
                    "database belongs to a newer Cadence; check your installations",
                ));
            }
            Err(db::DbOpenError::Io(detail))
            | Err(db::DbOpenError::Corrupt(detail))
            | Err(db::DbOpenError::Pragma(detail)) => {
                return Err(AppError::internal(detail));
            }
        };
        let key = generate_api_key();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(io_error)?;
        let port = listener.local_addr().map_err(io_error)?.port();
        self.last_bound_port = Some(port);

        let state = Arc::new(ApiState {
            db: DbAccess::new(api_database),
            api_key: key.clone(),
            api_port: port,
            app_handle: self.app_handle.clone(),
        });
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(server::start(listener, state, async move {
            let _ = shutdown_rx.await;
        }));

        if let Err(error) = write_discovery_file(&self.discovery_path, port, &key) {
            let _ = shutdown_tx.send(());
            let _ = task.await;
            let _ = self.remove_discovery_file();
            return Err(error);
        }

        self.running = Some(RunningApi {
            port,
            shutdown: shutdown_tx,
            task,
        });
        Ok(self.status())
    }

    async fn stop(&mut self) -> AppResult<()> {
        let task_result = if let Some(runtime) = self.running.take() {
            let _ = runtime.shutdown.send(());
            Some(runtime.task.await)
        } else {
            None
        };

        let remove_result = self.remove_discovery_file();
        if let Some(task_result) = task_result {
            task_result
                .map_err(|error| AppError::internal(format!("API task failed: {error}")))?
                .map_err(io_error)?;
        }
        remove_result
    }

    fn remove_discovery_file(&self) -> AppResult<()> {
        match fs::remove_file(&self.discovery_path) {
            Ok(()) => Ok(()),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                Ok(())
            }
            Err(error) => Err(io_error(error)),
        }
    }
}

fn generate_api_key() -> String {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    format!("cad_{}", hex::encode(key))
}

fn write_discovery_file(path: &Path, port: u16, key: &str) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::internal("Discovery path has no parent"))?;
    fs::create_dir_all(parent).map_err(io_error)?;
    #[cfg(unix)]
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(io_error)?;

    let payload = serde_json::to_vec_pretty(&DiscoveryFile {
        port,
        key: key.to_string(),
    })
    .map_err(|error| AppError::internal(format!("Discovery serialization failed: {error}")))?;
    write_private_file(path, &payload)
}

fn write_private_file(path: &Path, payload: &[u8]) -> AppResult<()> {
    #[cfg(unix)]
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(io_error)?;
        file.write_all(payload).map_err(io_error)?;
        file.flush().map_err(io_error)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(io_error)
    }

    #[cfg(not(unix))]
    {
        fs::write(path, payload).map_err(io_error)
    }
}

fn io_error(error: std::io::Error) -> AppError {
    AppError::internal(format!("I/O operation failed: {error}"))
}
