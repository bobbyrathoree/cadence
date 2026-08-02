use tauri::{Emitter, Manager};

use crate::models::collection::{Collection, CreateCollectionRequest};
use crate::models::playbook::{Playbook, PlaybookSession, PlaybookStep, PlaybookWithSteps};
use crate::models::prompt::{
    CreatePromptRequest, PromptListItem, PromptWithVariants, UpdatePromptRequest, Variant,
};
use crate::models::settings::KeyboardShortcut;
use crate::models::tag::{CreateTagRequest, Tag};
use crate::services::import_export::ImportResult;
use crate::services::{
    collection_service, import_export, playbook_service, prompt_service, search_service,
    settings_service, tag_service,
};
use crate::state::AppState;

#[tauri::command]
pub fn list_prompts(state: tauri::State<'_, AppState>) -> Result<Vec<PromptListItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::list_prompts(&conn, 100, 0).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_prompt(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PromptWithVariants, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::get_prompt_by_id(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_prompt(
    request: CreatePromptRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PromptWithVariants, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = prompt_service::create_prompt(&mut conn, request).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn update_prompt(
    id: String,
    request: UpdatePromptRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::update_prompt(&mut conn, &id, request).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_prompt(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::delete_prompt(&mut conn, &id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|_| "Database is unavailable")?;
    let new_state = prompt_service::toggle_favorite(&mut conn, &id).map_err(|e| e.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(new_state)
}

#[tauri::command]
pub fn add_variant(
    prompt_id: String,
    label: String,
    content: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Variant, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = prompt_service::add_variant(&mut conn, &prompt_id, &label, &content)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn update_variant(
    id: String,
    content: String,
    label: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::update_variant(&mut conn, &id, &content, label.as_deref())
        .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_variant(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::delete_variant(&mut conn, &id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    tag_service::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_tag(
    request: CreateTagRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Tag, String> {
    let mut conn = state.db.lock().map_err(|_| "Database is unavailable")?;
    let tag = tag_service::create_or_update_tag(&mut conn, request)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(tag)
}

#[tauri::command]
pub fn add_tags_to_prompt(
    prompt_id: String,
    tags: Vec<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<Tag>, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result =
        tag_service::add_tags_to_prompt(&mut conn, &prompt_id, &tags).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn remove_tag_from_prompt(
    prompt_id: String,
    tag_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    tag_service::remove_tag_from_prompt(&mut conn, &prompt_id, &tag_id)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_collections(state: tauri::State<'_, AppState>) -> Result<Vec<Collection>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    collection_service::list_collections(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(
    request: CreateCollectionRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Collection, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result =
        collection_service::create_collection(&mut conn, request).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn get_collection_prompts(
    collection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    collection_service::get_collection_prompts(&conn, &collection_id, 100, 0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_prompts(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    search_service::search_prompts(&conn, &query, 50).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn record_copy(
    prompt_id: String,
    variant_id: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = prompt_service::record_copy(&mut conn, &prompt_id, variant_id.as_deref())
        .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

// ------------------------------------------------------------------
// Playbooks
// ------------------------------------------------------------------

#[tauri::command]
pub fn list_playbooks(state: tauri::State<'_, AppState>) -> Result<Vec<Playbook>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::list_playbooks(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_playbook(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PlaybookWithSteps, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::get_playbook(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_playbook(
    title: String,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Playbook, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = playbook_service::create_playbook(&mut conn, &title, description.as_deref())
        .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn add_playbook_step(
    playbook_id: String,
    prompt_id: Option<String>,
    step_type: String,
    instructions: Option<String>,
    choice_prompt_ids: Option<Vec<String>>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookStep, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = playbook_service::add_step(
        &mut conn,
        &playbook_id,
        prompt_id.as_deref(),
        &step_type,
        instructions.as_deref(),
        choice_prompt_ids,
    )
    .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn get_playbook_session(state: tauri::State<'_, AppState>) -> Result<PlaybookSession, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::get_session(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_playbook_session(
    playbook_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookSession, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result =
        playbook_service::start_session(&mut conn, &playbook_id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn advance_playbook_step(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookSession, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = playbook_service::advance_step(&mut conn).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn end_playbook_session(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::end_session(&mut conn).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// ------------------------------------------------------------------
// Import / Export
// ------------------------------------------------------------------

#[tauri::command]
pub fn import_json(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    json_data: String,
) -> Result<ImportResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = import_export::import_json(&mut conn, &json_data).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn export_json(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    import_export::export_json(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_markdown_files(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    files: Vec<(String, String)>,
) -> Result<ImportResult, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result =
        import_export::import_markdown_batch(&mut conn, files).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

// ------------------------------------------------------------------
// Settings / Keyboard Shortcuts
// ------------------------------------------------------------------

#[tauri::command]
pub fn get_keyboard_shortcuts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<KeyboardShortcut>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    settings_service::get_keyboard_shortcuts(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_keyboard_shortcut(
    action: String,
    binding: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<KeyboardShortcut>, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;

    // If updating the global shortcut, handle re-registration
    if action == "global_toggle_search" {
        // Get old binding first
        let old_shortcuts =
            settings_service::get_keyboard_shortcuts(&conn).map_err(|e| e.to_string())?;
        let old_binding = old_shortcuts
            .iter()
            .find(|s| s.action == "global_toggle_search")
            .map(|s| s.binding.clone());

        // Update in DB
        let result = settings_service::update_shortcut(&mut conn, &action, &binding)
            .map_err(|e| e.to_string())?;

        // Re-register global shortcut
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

        // Unregister old
        if let Some(ref old) = old_binding {
            let _ = app.global_shortcut().unregister(old.as_str());
        }

        // Register new
        let handle = app.clone();
        let register_result =
            app.global_shortcut()
                .on_shortcut(binding.as_str(), move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window) = handle.get_webview_window("search") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });

        // If registration failed, rollback: re-register old and revert DB
        if let Err(e) = register_result {
            if let Some(ref old) = old_binding {
                let rollback_handle = app.clone();
                let _ = app.global_shortcut().on_shortcut(
                    old.as_str(),
                    move |_app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            if let Some(window) = rollback_handle.get_webview_window("search") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    },
                );
                let _ = settings_service::update_shortcut(&mut conn, &action, old);
            }
            return Err(format!(
                "Failed to register shortcut '{}': {}. Reverted to previous binding.",
                binding, e
            ));
        }

        let _ = app.emit("shortcuts-changed", ());
        Ok(result)
    } else {
        let result = settings_service::update_shortcut(&mut conn, &action, &binding)
            .map_err(|e| e.to_string())?;
        let _ = app.emit("shortcuts-changed", ());
        Ok(result)
    }
}

#[tauri::command]
pub fn reset_keyboard_shortcuts(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<KeyboardShortcut>, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = settings_service::reset_shortcuts(&mut conn).map_err(|e| e.to_string())?;

    // Re-register the default global shortcut
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let _ = app.global_shortcut().unregister_all();

    let handle = app.clone();
    let _ = app.global_shortcut().on_shortcut(
        "CommandOrControl+Shift+P",
        move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = handle.get_webview_window("search") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        },
    );

    let _ = app.emit("shortcuts-changed", ());
    Ok(result)
}
