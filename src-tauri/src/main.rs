#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use rand::Rng;

use cadence_lib::db;
use cadence_lib::state::AppState;

/// Generate an API key in the format `cad_` followed by 16 random hex characters.
fn generate_api_key() -> String {
    let mut buf = [0u8; 8];
    rand::thread_rng().fill(&mut buf);
    format!("cad_{}", hex::encode(buf))
}

fn main() {
    // Initialize the database (creates dir + schema if needed).
    let conn = db::init().expect("Failed to initialize database");

    let api_key = generate_api_key();
    let api_port: u16 = 9849;

    let app_state = AppState {
        db: Mutex::new(conn),
        api_key,
        api_port,
    };

    tauri::Builder::default()
        .manage(app_state)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
