use axum::{extract::DefaultBodyLimit, middleware, Router};
use rusqlite::Connection;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use super::auth::auth_middleware;
use super::routes;

const MAX_API_BODY_BYTES: usize = 4 * 1024 * 1024;

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
    let app = Router::new()
        .merge(routes::router())
        .layer(DefaultBodyLimit::max(MAX_API_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
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
    fs::create_dir_all(&dir)?;

    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;

    let discovery = serde_json::json!({
        "port": port,
        "key": api_key,
    });

    let path = dir.join("api.json");
    let payload = serde_json::to_vec_pretty(&discovery).unwrap();
    write_private_file(&path, &payload)?;

    Ok(())
}

fn write_private_file(path: &std::path::Path, payload: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(payload)?;
        file.flush()?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        fs::write(path, payload)
    }
}
