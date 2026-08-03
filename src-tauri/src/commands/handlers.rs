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

fn with_db<T>(
    state: &AppState,
    operation: impl FnOnce(&mut cadence_core::db::Db) -> crate::error::AppResult<T>,
) -> Result<T, String> {
    state
        .main
        .with_sync(operation)
        .map_err(|error| error.ipc_message())
}

fn with_db_string<T>(
    state: &AppState,
    operation: impl FnOnce(&mut cadence_core::db::Db) -> Result<T, String>,
) -> Result<T, String> {
    state
        .main
        .with_sync(|db| Ok(operation(db)))
        .map_err(|error| error.ipc_message())?
}

#[tauri::command]
pub fn get_api_enabled(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    with_db(&state, |db| settings_service::get_api_enabled(&db.conn))
}

#[tauri::command]
pub async fn set_api_enabled(
    enabled: bool,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ApiStatus, String> {
    let mut api = state.api.lock().await;
    let status = api
        .set_enabled(enabled)
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
    with_db(&state, |db| {
        prompt_service::list_prompts_page(&db.conn, filter.as_deref(), limit, offset)
    })
}

#[tauri::command]
pub fn get_prompt(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PromptWithVariants, String> {
    with_db(&state, |db| prompt_service::get_prompt_by_id(&db.conn, &id))
}

#[tauri::command]
pub fn create_prompt(
    request: CreatePromptRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PromptWithVariants, String> {
    let result = with_db(&state, |db| prompt_service::create_prompt(db, request))?;
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
    with_db(&state, |db| prompt_service::update_prompt(db, &id, request))?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_prompt(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    with_db(&state, |db| prompt_service::delete_prompt(db, &id))?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    let new_state = with_db(&state, |db| prompt_service::toggle_favorite(db, &id))?;
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
    let result = with_db(&state, |db| {
        prompt_service::add_variant(db, &prompt_id, &label, &content)
    })?;
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
    with_db(&state, |db| {
        prompt_service::update_variant(db, &id, &content, label.as_deref())
    })?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_variant(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    with_db(&state, |db| prompt_service::delete_variant(db, &id))?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<Tag>, String> {
    with_db(&state, |db| tag_service::list_tags(&db.conn))
}

#[tauri::command]
pub fn create_tag(
    request: CreateTagRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Tag, String> {
    let tag = with_db(&state, |db| tag_service::create_or_update_tag(db, request))?;
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
    let result = with_db(&state, |db| {
        tag_service::add_tags_to_prompt(db, &prompt_id, &tags)
    })?;
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
    with_db(&state, |db| {
        tag_service::remove_tag_from_prompt(db, &prompt_id, &tag_id)
    })?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_collections(state: tauri::State<'_, AppState>) -> Result<Vec<Collection>, String> {
    with_db(&state, |db| collection_service::list_collections(&db.conn))
}

#[tauri::command]
pub fn create_collection(
    request: CreateCollectionRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Collection, String> {
    let result = with_db(&state, |db| {
        collection_service::create_collection(db, request)
    })?;
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
    with_db(&state, |db| {
        collection_service::add_prompt_to_collection(db, &collection_id, &prompt_id)
    })?;
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
    with_db(&state, |db| {
        collection_service::remove_prompt_from_collection(db, &collection_id, &prompt_id)
    })?;
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
    with_db(&state, |db| {
        collection_service::get_collection_prompts_page(&db.conn, &collection_id, limit, offset)
    })
}

#[tauri::command]
pub fn search_prompts(
    query: String,
    limit: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    with_db(&state, |db| {
        search_service::search_prompts_page(&db.conn, &query, limit)
    })
}

#[tauri::command]
pub fn record_copy(
    prompt_id: String,
    variant_id: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let result = with_db(&state, |db| {
        prompt_service::record_copy(db, &prompt_id, variant_id.as_deref())
    })?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn get_prompt_usage(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PromptUsage, String> {
    with_db(&state, |db| prompt_service::get_prompt_usage(&db.conn, &id))
}

#[tauri::command]
pub fn get_prompt_counts(state: tauri::State<'_, AppState>) -> Result<PromptCounts, String> {
    with_db(&state, |db| prompt_service::get_prompt_counts(&db.conn))
}

// ------------------------------------------------------------------
// Playbooks
// ------------------------------------------------------------------

#[tauri::command]
pub fn list_playbooks(state: tauri::State<'_, AppState>) -> Result<Vec<Playbook>, String> {
    with_db(&state, |db| playbook_service::list_playbooks(&db.conn))
}

#[tauri::command]
pub fn get_playbook(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PlaybookWithSteps, String> {
    with_db(&state, |db| playbook_service::get_playbook(&db.conn, &id))
}

#[tauri::command]
pub fn create_playbook(
    title: String,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Playbook, String> {
    let result = with_db(&state, |db| {
        playbook_service::create_playbook(db, &title, description.as_deref())
    })?;
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
    let playbook = with_db(&state, |db| {
        playbook_service::update_playbook(db, &id, request)
    })?;
    let _ = app.emit("db-changed", ());
    Ok(playbook)
}

#[tauri::command]
pub fn delete_playbook(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    with_db(&state, |db| playbook_service::delete_playbook(db, &id))?;
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
    let result = with_db(&state, |db| {
        playbook_service::add_step(db, &playbook_id, spec)
    })?;
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
    let result = with_db(&state, |db| {
        playbook_service::update_step(db, &playbook_id, &step_id, spec)
    })?;
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
    with_db(&state, |db| {
        playbook_service::remove_step(db, &playbook_id, &step_id)
    })?;
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
    with_db(&state, |db| {
        playbook_service::reorder_steps(db, &playbook_id, &ordered_step_ids)
    })?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn get_playbook_session(state: tauri::State<'_, AppState>) -> Result<PlaybookSession, String> {
    with_db(&state, |db| playbook_service::get_session(&db.conn))
}

#[tauri::command]
pub fn start_playbook_session(
    playbook_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookSession, String> {
    let result = with_db(&state, |db| {
        playbook_service::start_session(db, &playbook_id)
    })?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn advance_playbook_step(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookSession, String> {
    let result = with_db(&state, playbook_service::advance_step)?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn end_playbook_session(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    with_db(&state, playbook_service::end_session)?;
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
    let result = with_db(&state, |db| import_export::import_json(db, &json_data))?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn export_json(state: tauri::State<'_, AppState>) -> Result<String, String> {
    with_db(&state, |db| import_export::export_json(&db.conn))
}

#[tauri::command]
pub fn import_markdown_files(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    files: Vec<(String, String)>,
) -> Result<ImportResult, String> {
    let result = with_db(&state, |db| import_export::import_markdown_batch(db, files))?;
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
    with_db(&state, |db| {
        settings_service::get_keyboard_shortcuts(&db.conn)
    })
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
    let result = with_db_string(&state, |db| {
        if action == GLOBAL_SEARCH_ACTION {
            let old_shortcuts = settings_service::get_keyboard_shortcuts(&db.conn)
                .map_err(|error| error.to_string())?;
            let old_binding = old_shortcuts
                .iter()
                .find(|shortcut| shortcut.action == GLOBAL_SEARCH_ACTION)
                .map(|shortcut| shortcut.binding.clone())
                .ok_or_else(|| "Global search shortcut is unavailable".to_string())?;

            persist_global_shortcut_change(&app, &old_binding, &binding, || {
                settings_service::update_shortcut(db, &action, &binding)
                    .map_err(|error| error.to_string())
            })
        } else {
            settings_service::update_shortcut(db, &action, &binding)
                .map_err(|error| error.to_string())
        }
    })?;
    let _ = app.emit("shortcuts-changed", ());
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn reset_keyboard_shortcuts(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<KeyboardShortcut>, String> {
    let result = with_db_string(&state, |db| {
        let old_shortcuts = settings_service::get_keyboard_shortcuts(&db.conn)
            .map_err(|error| error.to_string())?;
        let old_binding = old_shortcuts
            .iter()
            .find(|shortcut| shortcut.action == GLOBAL_SEARCH_ACTION)
            .map(|shortcut| shortcut.binding.clone())
            .ok_or_else(|| "Global search shortcut is unavailable".to_string())?;

        persist_global_shortcut_change(&app, &old_binding, DEFAULT_GLOBAL_SEARCH_SHORTCUT, || {
            settings_service::reset_shortcuts(db).map_err(|error| error.to_string())
        })
    })?;

    let _ = app.emit("shortcuts-changed", ());
    let _ = app.emit("db-changed", ());
    Ok(result)
}
