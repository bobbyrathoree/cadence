use rusqlite::Connection;
use std::sync::Mutex;

/// Application state shared across Tauri commands and the API server.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub api_key: String,
    pub api_port: u16,
}
