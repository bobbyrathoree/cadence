use cadence_core::db_access::DbAccess;
use tokio::sync::Mutex as AsyncMutex;

use crate::api::lifecycle::ApiLifecycle;

/// Application state shared across Tauri commands and the API server.
pub struct AppState {
    pub main: DbAccess,
    pub api: AsyncMutex<ApiLifecycle>,
}
