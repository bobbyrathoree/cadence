#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use std::path::Path;
use std::process::{self, Command};

use cadence_core::db_access::DbAccess;
use serde::Serialize;
use tauri::{Emitter, Manager};

use cadence_lib::api::lifecycle::ApiLifecycle;
use cadence_lib::commands;
use cadence_lib::db;
use cadence_lib::models::settings::{DEFAULT_GLOBAL_SEARCH_SHORTCUT, GLOBAL_SEARCH_ACTION};
use cadence_lib::poller;
use cadence_lib::search_window::{
    register_search_shortcut, search_window, Mode as SearchWindowMode,
};
use cadence_lib::seed;
use cadence_lib::state::AppState;

#[tauri::command]
fn hide_search_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("search") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn show_search_window(app: tauri::AppHandle) -> Result<(), String> {
    search_window(&app, SearchWindowMode::Show)
}

#[derive(Serialize)]
struct McpBinaryLocation {
    path: String,
    development: bool,
}

#[tauri::command]
fn get_mcp_binary_location(app: tauri::AppHandle) -> Result<McpBinaryLocation, String> {
    let development = cfg!(debug_assertions);
    let path = if development {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "Cadence could not resolve its workspace root".to_string())?
            .join("target")
            .join("debug")
            .join("cadence-mcp")
    } else {
        app.path()
            .executable_dir()
            .map_err(|error| {
                format!("Cadence could not resolve its executable directory: {error}")
            })?
            .join("cadence-mcp")
    };
    Ok(McpBinaryLocation {
        path: path.to_string_lossy().into_owned(),
        development,
    })
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Cadence failed to start: {error}");
        show_fatal_startup_dialog(&error);
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let database_path = db::locate_database(None)
        .map_err(|error| format!("Cadence could not locate its database: {error}"))?;
    let mut database = match db::open_app(&database_path, db::Health::exit_process(1))
        .map_err(|error| format!("Cadence could not open its database: {error}"))?
    {
        db::DbOpen::Ready(database) => database,
        db::DbOpen::NeedsMigration(mut database, version) => {
            if version == 0 {
                db::schema::create_tables(&database.conn).map_err(|error| {
                    format!("Cadence could not initialize its database schema: {error}")
                })?;
            }
            db::migrate(&mut database.conn)
                .map_err(|error| format!("Cadence could not migrate its database: {error}"))?;
            let migrated_version: i64 = database
                .conn
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .map_err(|error| {
                    format!("Cadence could not verify its database schema: {error}")
                })?;
            if migrated_version != db::CURRENT_SCHEMA_VERSION {
                return Err(format!(
                    "Cadence database migration ended at schema version {migrated_version}, expected {}",
                    db::CURRENT_SCHEMA_VERSION
                ));
            }
            database
        }
        db::DbOpen::SchemaNewer {
            db_version,
            supported,
        } => {
            return Err(format!(
                "Cadence database schema version {db_version} is newer than supported version {supported}"
            ));
        }
        db::DbOpen::MissingFile => {
            return Err("Cadence application database was not created".to_string());
        }
    };

    // Seed starter content on first launch (no-op if data already exists).
    if let Err(e) = seed::seed_if_empty(&mut database) {
        eprintln!("Warning: failed to seed starter kit: {}", e);
    }

    let main = DbAccess::new(database);
    let app_state = AppState {
        main: main.clone(),
        api: tokio::sync::Mutex::new(ApiLifecycle::for_application(database_path.clone(), main)?),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
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
            commands::handlers::add_prompt_to_collection,
            commands::handlers::remove_prompt_from_collection,
            commands::handlers::search_prompts,
            commands::handlers::record_copy,
            commands::handlers::get_prompt_usage,
            commands::handlers::get_prompt_counts,
            commands::handlers::list_playbooks,
            commands::handlers::get_playbook,
            commands::handlers::create_playbook,
            commands::handlers::update_playbook,
            commands::handlers::delete_playbook,
            commands::handlers::add_step,
            commands::handlers::update_step,
            commands::handlers::remove_step,
            commands::handlers::reorder_steps,
            commands::handlers::get_playbook_session,
            commands::handlers::start_playbook_session,
            commands::handlers::advance_playbook_step,
            commands::handlers::end_playbook_session,
            commands::handlers::import_json,
            commands::handlers::export_json,
            commands::handlers::import_markdown_files,
            commands::handlers::get_keyboard_shortcuts,
            commands::handlers::update_keyboard_shortcut,
            commands::handlers::reset_keyboard_shortcuts,
            commands::handlers::get_api_enabled,
            commands::handlers::set_api_enabled,
            get_mcp_binary_location,
            hide_search_window,
            show_search_window,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            poller::start(handle.clone(), &database_path)?;

            // Set up native tray menu
            if let Err(e) = tray::setup_tray(&handle) {
                eprintln!("Failed to setup tray: {}", e);
            }

            let api_startup = tauri::async_runtime::block_on(async {
                let state = app.state::<AppState>();
                let mut api = state.api.lock().await;
                api.set_app_handle(handle.clone());
                api.startup().await
            });
            if let Err(error) = api_startup {
                eprintln!("Failed to start the local API: {error}");
                let _ = app.emit("api-error", error.ipc_message());
            }

            // Register global shortcut dynamically from saved settings
            {
                use cadence_lib::services::settings_service;

                let shortcut_binding = {
                    let state = app.state::<AppState>();
                    let resolved = match state
                        .main
                        .with_sync(|db| settings_service::get_keyboard_shortcuts(&db.conn))
                    {
                        Ok(shortcuts) => shortcuts
                            .iter()
                            .find(|shortcut| shortcut.action == GLOBAL_SEARCH_ACTION)
                            .map(|shortcut| shortcut.binding.clone())
                            .unwrap_or_else(|| DEFAULT_GLOBAL_SEARCH_SHORTCUT.to_string()),
                        Err(error) => {
                            eprintln!("Warning: failed to read global shortcut setting: {error:?}");
                            DEFAULT_GLOBAL_SEARCH_SHORTCUT.to_string()
                        }
                    };
                    resolved
                };

                if let Err(error) = register_search_shortcut(&handle, &shortcut_binding) {
                    eprintln!(
                        "Warning: failed to register global shortcut '{shortcut_binding}': {error}"
                    );
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .map_err(|error| format!("Cadence could not build its application runtime: {error}"))?;

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            let state = app_handle.state::<AppState>();
            let _ = tauri::async_runtime::block_on(async {
                state.api.lock().await.shutdown_on_exit().await
            });
        }
    });
    Ok(())
}

fn show_fatal_startup_dialog(message: &str) {
    let result = Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display alert \"Cadence could not start\" message (item 1 of argv) as critical",
            "-e",
            "end run",
        ])
        .arg(message)
        .status();
    if let Err(error) = result {
        eprintln!("Failed to show startup error dialog: {error}");
    }
}
