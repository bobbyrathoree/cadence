use rusqlite::Connection;
use std::sync::Mutex;
use tokio::sync::Mutex as AsyncMutex;

use crate::api::lifecycle::ApiLifecycle;

/// Application state shared across Tauri commands and the API server.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub api: AsyncMutex<ApiLifecycle>,
}
