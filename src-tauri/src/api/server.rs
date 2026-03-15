use axum::{middleware, Router};
use rusqlite::Connection;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

use super::auth::auth_middleware;
use super::routes;

/// State shared with axum route handlers.
/// Uses a separate SQLite connection from the Tauri-managed one.
pub struct ApiState {
    pub db: Mutex<Connection>,
    pub api_key: String,
}

/// Start the axum HTTP API server.
///
/// Binds to `127.0.0.1:0` (dynamic port), writes a discovery file
/// to `~/Library/Application Support/Cadence/api.json` with the port and key,
/// and runs the server indefinitely.
pub async fn start(state: Arc<ApiState>) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::router())
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind API server");

    let addr: SocketAddr = listener.local_addr().expect("Failed to get local address");
    let port = addr.port();

    // Write discovery file
    if let Err(e) = write_discovery_file(port, &state.api_key) {
        eprintln!("Failed to write API discovery file: {}", e);
    }

    println!("Cadence API server listening on 127.0.0.1:{}", port);

    axum::serve(listener, app)
        .await
        .expect("API server error");
}

/// Write the API discovery file so external tools (Raycast, Shortcuts) can find the server.
fn write_discovery_file(port: u16, api_key: &str) -> std::io::Result<()> {
    let dir = crate::db::db_path();
    std::fs::create_dir_all(&dir)?;

    let discovery = serde_json::json!({
        "port": port,
        "key": api_key,
    });

    let path = dir.join("api.json");
    std::fs::write(&path, serde_json::to_string_pretty(&discovery).unwrap())?;

    Ok(())
}
