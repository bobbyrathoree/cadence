#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use std::sync::Mutex;

use tauri::{Emitter, Manager};

use cadence_lib::api::lifecycle::ApiLifecycle;
use cadence_lib::commands;
use cadence_lib::db;
use cadence_lib::search_window::{search_window, Mode as SearchWindowMode};
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

fn main() {
    // Initialize the database (creates dir + schema if needed).
    let mut conn = db::init().expect("Failed to initialize database");

    // Migrations run exactly once on the main connection before seeding,
    // opening the API connection, or loading any windows.
    db::migrate(&mut conn).expect("Failed to migrate database");

    // Seed starter content on first launch (no-op if data already exists).
    if let Err(e) = seed::seed_if_empty(&mut conn) {
        eprintln!("Warning: failed to seed starter kit: {}", e);
    }

    let app_state = AppState {
        db: Mutex::new(conn),
        api: tokio::sync::Mutex::new(ApiLifecycle::for_application()),
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
            commands::handlers::search_prompts,
            commands::handlers::record_copy,
            commands::handlers::list_playbooks,
            commands::handlers::get_playbook,
            commands::handlers::create_playbook,
            commands::handlers::add_playbook_step,
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
            hide_search_window,
            show_search_window,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Set up native tray menu
            if let Err(e) = tray::setup_tray(&handle) {
                eprintln!("Failed to setup tray: {}", e);
            }

            let api_startup = tauri::async_runtime::block_on(async {
                let state = app.state::<AppState>();
                let mut api = state.api.lock().await;
                api.set_app_handle(handle.clone());
                api.startup(&state.db).await
            });
            if let Err(error) = api_startup {
                eprintln!("Failed to start the local API: {error}");
                let _ = app.emit("api-error", error.ipc_message());
            }

            // Register global shortcut dynamically from saved settings
            {
                use cadence_lib::services::settings_service;
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

                let shortcut_binding = {
                    let state = app.state::<AppState>();
                    let conn = state
                        .db
                        .lock()
                        .expect("Failed to lock DB for shortcut init");
                    let shortcuts =
                        settings_service::get_keyboard_shortcuts(&conn).unwrap_or_default();
                    shortcuts
                        .iter()
                        .find(|s| s.action == "global_toggle_search")
                        .map(|s| s.binding.clone())
                        .unwrap_or_else(|| "CommandOrControl+Shift+P".to_string())
                };

                let handle_for_shortcut = handle.clone();

                app.global_shortcut()
                    .on_shortcut(shortcut_binding.as_str(), move |_app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            if let Err(error) =
                                search_window(&handle_for_shortcut, SearchWindowMode::Toggle)
                            {
                                eprintln!("Failed to toggle search window: {error}");
                            }
                        }
                    })
                    .expect("Failed to register global shortcut");
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            let state = app_handle.state::<AppState>();
            let _ = tauri::async_runtime::block_on(async {
                state.api.lock().await.shutdown_on_exit().await
            });
        }
    });
}
