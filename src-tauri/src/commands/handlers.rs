use tauri::Emitter;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::api::lifecycle::ApiStatus;
use crate::models::collection::{Collection, CreateCollectionRequest};
use crate::models::playbook::{
    Playbook, PlaybookSession, PlaybookStepWithPrompt, PlaybookWithSteps, StepSpec,
    UpdatePlaybookRequest,
};
use crate::models::prompt::{
    CreatePromptRequest, PromptCounts, PromptListItem, PromptUsage, PromptWithVariants,
    UpdatePromptRequest, Variant,
};
use crate::models::settings::{
    KeyboardShortcut, DEFAULT_GLOBAL_SEARCH_SHORTCUT, GLOBAL_SEARCH_ACTION,
};
use crate::models::tag::{CreateTagRequest, Tag};
use crate::search_window::{
    register_search_shortcut, register_shortcut_and_persist, reregister_shortcut,
};
use crate::services::import_export::ImportResult;
use crate::services::{
    collection_service, import_export, playbook_service, prompt_service, search_service,
    settings_service, tag_service,
};
use crate::state::AppState;

#[tauri::command]
pub fn get_api_enabled(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    settings_service::get_api_enabled(&conn).map_err(|error| error.ipc_message())
}

#[tauri::command]
pub async fn set_api_enabled(
    enabled: bool,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ApiStatus, String> {
    let mut api = state.api.lock().await;
    let status = api
        .set_enabled(&state.db, enabled)
        .await
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(status)
}

#[tauri::command]
pub fn list_prompts(
    filter: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::list_prompts_page(&conn, filter.as_deref(), limit, offset)
        .map_err(|error| error.ipc_message())
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
pub fn add_prompt_to_collection(
    collection_id: String,
    prompt_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    collection_service::add_prompt_to_collection(&mut conn, &collection_id, &prompt_id)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn remove_prompt_from_collection(
    collection_id: String,
    prompt_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    collection_service::remove_prompt_from_collection(&mut conn, &collection_id, &prompt_id)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn get_collection_prompts(
    collection_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    collection_service::get_collection_prompts_page(&conn, &collection_id, limit, offset)
        .map_err(|error| error.ipc_message())
}

#[tauri::command]
pub fn search_prompts(
    query: String,
    limit: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    search_service::search_prompts_page(&conn, &query, limit).map_err(|error| error.ipc_message())
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

#[tauri::command]
pub fn get_prompt_usage(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PromptUsage, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::get_prompt_usage(&conn, &id).map_err(|error| error.ipc_message())
}

#[tauri::command]
pub fn get_prompt_counts(state: tauri::State<'_, AppState>) -> Result<PromptCounts, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    prompt_service::get_prompt_counts(&conn).map_err(|error| error.ipc_message())
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
pub fn update_playbook(
    id: String,
    request: UpdatePlaybookRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Playbook, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let playbook = playbook_service::update_playbook(&mut conn, &id, request)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(playbook)
}

#[tauri::command]
pub fn delete_playbook(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::delete_playbook(&mut conn, &id).map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn add_step(
    playbook_id: String,
    spec: StepSpec,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookStepWithPrompt, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = playbook_service::add_step(&mut conn, &playbook_id, spec)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn update_step(
    playbook_id: String,
    step_id: String,
    spec: StepSpec,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookStepWithPrompt, String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    let result = playbook_service::update_step(&mut conn, &playbook_id, &step_id, spec)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn remove_step(
    playbook_id: String,
    step_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::remove_step(&mut conn, &playbook_id, &step_id)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn reorder_steps(
    playbook_id: String,
    ordered_step_ids: Vec<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| "Database is unavailable".to_string())?;
    playbook_service::reorder_steps(&mut conn, &playbook_id, &ordered_step_ids)
        .map_err(|error| error.ipc_message())?;
    let _ = app.emit("db-changed", ());
    Ok(())
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

fn persist_global_shortcut_change<T, Persist>(
    app: &tauri::AppHandle,
    current: &str,
    replacement: &str,
    persist: Persist,
) -> Result<T, String>
where
    Persist: FnOnce() -> Result<T, String>,
{
    let register = |candidate: &str| register_search_shortcut(app, candidate);
    let unregister = |candidate: &str| {
        app.global_shortcut()
            .unregister(candidate)
            .map_err(|error| error.to_string())
    };

    if current == replacement && !app.global_shortcut().is_registered(current) {
        register_shortcut_and_persist(replacement, register, unregister, persist)
    } else {
        reregister_shortcut(current, replacement, register, unregister, persist)
    }
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

    if action == GLOBAL_SEARCH_ACTION {
        let old_shortcuts =
            settings_service::get_keyboard_shortcuts(&conn).map_err(|e| e.to_string())?;
        let old_binding = old_shortcuts
            .iter()
            .find(|shortcut| shortcut.action == GLOBAL_SEARCH_ACTION)
            .map(|shortcut| shortcut.binding.clone())
            .ok_or_else(|| "Global search shortcut is unavailable".to_string())?;

        let result = persist_global_shortcut_change(&app, &old_binding, &binding, || {
            settings_service::update_shortcut(&mut conn, &action, &binding)
                .map_err(|error| error.to_string())
        })?;

        let _ = app.emit("shortcuts-changed", ());
        let _ = app.emit("db-changed", ());
        Ok(result)
    } else {
        let result = settings_service::update_shortcut(&mut conn, &action, &binding)
            .map_err(|e| e.to_string())?;
        let _ = app.emit("shortcuts-changed", ());
        let _ = app.emit("db-changed", ());
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
    let old_shortcuts =
        settings_service::get_keyboard_shortcuts(&conn).map_err(|error| error.to_string())?;
    let old_binding = old_shortcuts
        .iter()
        .find(|shortcut| shortcut.action == GLOBAL_SEARCH_ACTION)
        .map(|shortcut| shortcut.binding.clone())
        .ok_or_else(|| "Global search shortcut is unavailable".to_string())?;

    let result =
        persist_global_shortcut_change(&app, &old_binding, DEFAULT_GLOBAL_SEARCH_SHORTCUT, || {
            settings_service::reset_shortcuts(&mut conn).map_err(|error| error.to_string())
        })?;

    let _ = app.emit("shortcuts-changed", ());
    let _ = app.emit("db-changed", ());
    Ok(result)
}
