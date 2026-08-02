use std::future::Future;
use std::sync::{Arc, Mutex};

use axum::{extract::DefaultBodyLimit, middleware, Router};
use rusqlite::Connection;
use tokio::net::TcpListener;

use super::auth::auth_middleware;
use super::routes;

const MAX_API_BODY_BYTES: usize = 4 * 1024 * 1024;

/// State shared with axum route handlers.
/// Uses a separate SQLite connection from the Tauri-managed one.
pub struct ApiState {
    pub db: Mutex<Connection>,
    pub api_key: String,
    pub api_port: u16,
}

pub fn build_app(state: Arc<ApiState>) -> Router {
    // Deliberately no CORS layer: browser origins are not API clients.
    Router::new()
        .merge(routes::router())
        .layer(DefaultBodyLimit::max(MAX_API_BODY_BYTES))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

pub async fn start(
    listener: TcpListener,
    state: Arc<ApiState>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, build_app(state))
        .with_graceful_shutdown(shutdown)
        .await
}
