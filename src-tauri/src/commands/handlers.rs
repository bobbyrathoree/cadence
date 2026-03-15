use tauri::Emitter;

use crate::models::collection::{Collection, CreateCollectionRequest};
use crate::models::playbook::{Playbook, PlaybookSession, PlaybookStep, PlaybookWithSteps};
use crate::services::import_export::ImportResult;
use crate::models::prompt::{
    CreatePromptRequest, PromptListItem, PromptWithVariants, UpdatePromptRequest, Variant,
};
use crate::models::tag::{CreateTagRequest, Tag};
use crate::services::{collection_service, import_export, playbook_service, prompt_service, search_service, tag_service};
use crate::state::AppState;

#[tauri::command]
pub fn list_prompts(state: tauri::State<'_, AppState>) -> Result<Vec<PromptListItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    prompt_service::list_prompts(&conn, 100, 0).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_prompt(id: String, state: tauri::State<'_, AppState>) -> Result<PromptWithVariants, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    prompt_service::get_prompt_by_id(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_prompt(
    request: CreatePromptRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PromptWithVariants, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = prompt_service::create_prompt(&conn, request).map_err(|e| e.to_string())?;
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    prompt_service::update_prompt(&conn, &id, request).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_prompt(id: String, state: tauri::State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    prompt_service::delete_prompt(&conn, &id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(id: String, state: tauri::State<'_, AppState>, app: tauri::AppHandle) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Get current favorite state
    let current: bool = conn
        .query_row(
            "SELECT is_favorite FROM prompts WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .map_err(|e| e.to_string())?;

    let new_state = !current;

    let req = UpdatePromptRequest {
        title: None,
        description: None,
        is_favorite: Some(new_state),
        is_pinned: None,
        primary_variant_id: None,
    };

    prompt_service::update_prompt(&conn, &id, req).map_err(|e| e.to_string())?;
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = prompt_service::add_variant(&conn, &prompt_id, &label, &content).map_err(|e| e.to_string())?;
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    prompt_service::update_variant(&conn, &id, &content, label.as_deref()).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_variant(id: String, state: tauri::State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    prompt_service::delete_variant(&conn, &id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tag_service::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_tag(
    request: CreateTagRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Tag, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let tag = tag_service::get_or_create_tag(&conn, &request.name).map_err(|e| e.to_string())?;
    // If a color was provided, update the tag
    if let Some(ref color) = request.color {
        conn.execute(
            "UPDATE tags SET color = ?1 WHERE id = ?2",
            rusqlite::params![color, tag.id],
        )
        .map_err(|e| e.to_string())?;
        let _ = app.emit("db-changed", ());
        return Ok(Tag {
            color: Some(color.clone()),
            ..tag
        });
    }
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = tag_service::add_tags_to_prompt(&conn, &prompt_id, &tags).map_err(|e| e.to_string())?;
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tag_service::remove_tag_from_prompt(&conn, &prompt_id, &tag_id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_collections(state: tauri::State<'_, AppState>) -> Result<Vec<Collection>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    collection_service::list_collections(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(
    request: CreateCollectionRequest,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Collection, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = collection_service::create_collection(&conn, request).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn get_collection_prompts(
    collection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    collection_service::get_collection_prompts(&conn, &collection_id, 100, 0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_prompts(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PromptListItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    search_service::search_prompts(&conn, &query, 50).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn record_copy(
    prompt_id: String,
    variant_id: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = prompt_service::record_copy(&conn, &prompt_id, variant_id.as_deref())
        .map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

// ------------------------------------------------------------------
// Playbooks
// ------------------------------------------------------------------

#[tauri::command]
pub fn list_playbooks(state: tauri::State<'_, AppState>) -> Result<Vec<Playbook>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    playbook_service::list_playbooks(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_playbook(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PlaybookWithSteps, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    playbook_service::get_playbook(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_playbook(
    title: String,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Playbook, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = playbook_service::create_playbook(&conn, &title, description.as_deref())
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = playbook_service::add_step(
        &conn,
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
pub fn get_playbook_session(
    state: tauri::State<'_, AppState>,
) -> Result<PlaybookSession, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    playbook_service::get_session(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_playbook_session(
    playbook_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookSession, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = playbook_service::start_session(&conn, &playbook_id).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn advance_playbook_step(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PlaybookSession, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = playbook_service::advance_step(&conn).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn end_playbook_session(state: tauri::State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    playbook_service::end_session(&conn).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// ------------------------------------------------------------------
// Import / Export
// ------------------------------------------------------------------

#[tauri::command]
pub fn import_json(state: tauri::State<'_, AppState>, app: tauri::AppHandle, json_data: String) -> Result<ImportResult, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = import_export::import_json(&conn, &json_data).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn export_json(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    import_export::export_json(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_markdown_files(state: tauri::State<'_, AppState>, app: tauri::AppHandle, files: Vec<(String, String)>) -> Result<ImportResult, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = import_export::import_markdown_batch(&conn, files).map_err(|e| e.to_string())?;
    let _ = app.emit("db-changed", ());
    Ok(result)
}
