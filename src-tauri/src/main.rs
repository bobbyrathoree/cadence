#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use rand::Rng;

use cadence_lib::api;
use cadence_lib::commands;
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
    let api_port: u16 = 0; // Will be determined by the API server

    let app_state = AppState {
        db: Mutex::new(conn),
        api_key: api_key.clone(),
        api_port,
    };

    // Create a separate database connection for the API server
    let api_conn = db::init().expect("Failed to initialize API database connection");
    let api_state = Arc::new(api::server::ApiState {
        db: Mutex::new(api_conn),
        api_key,
    });

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::handlers::list_prompts,
            commands::handlers::get_prompt,
            commands::handlers::create_prompt,
            commands::handlers::update_prompt,
            commands::handlers::delete_prompt,
            commands::handlers::toggle_favorite,
            commands::handlers::add_variant,
            commands::handlers::update_variant,
            commands::handlers::delete_variant,
            commands::handlers::list_tags,
            commands::handlers::create_tag,
            commands::handlers::add_tags_to_prompt,
            commands::handlers::remove_tag_from_prompt,
            commands::handlers::list_collections,
            commands::handlers::create_collection,
            commands::handlers::get_collection_prompts,
            commands::handlers::search_prompts,
            commands::handlers::record_copy,
        ])
        .setup(move |_app| {
            // Spawn the axum API server in a background task
            tauri::async_runtime::spawn(async move {
                api::server::start(api_state).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
