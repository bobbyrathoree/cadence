use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::server::ApiState;
use crate::models::collection::{Collection, CreateCollectionRequest};
use crate::models::playbook::{Playbook, PlaybookWithSteps};
use crate::models::prompt::{
    CreatePromptRequest, PromptListItem, PromptWithVariants, UpdatePromptRequest, Variant,
};
use crate::models::tag::{CreateTagRequest, Tag};
use crate::services::{collection_service, playbook_service, prompt_service, search_service, tag_service};

// ------------------------------------------------------------------
// Router
// ------------------------------------------------------------------

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        // Health
        .route("/api/v1/health", get(health))
        // Prompts
        .route("/api/v1/prompts", get(list_prompts).post(create_prompt))
        .route(
            "/api/v1/prompts/{id}",
            get(get_prompt).put(update_prompt).delete(delete_prompt),
        )
        // Variants
        .route("/api/v1/prompts/{id}/variants", post(add_variant))
        .route(
            "/api/v1/variants/{id}",
            put(update_variant).delete(delete_variant),
        )
        // Tags
        .route("/api/v1/tags", get(list_tags).post(create_tag))
        .route("/api/v1/prompts/{id}/tags", post(add_tags_to_prompt))
        .route(
            "/api/v1/prompts/{prompt_id}/tags/{tag_id}",
            delete(remove_tag_from_prompt),
        )
        // Collections
        .route(
            "/api/v1/collections",
            get(list_collections).post(create_collection),
        )
        .route(
            "/api/v1/collections/{id}/prompts",
            get(get_collection_prompts).post(add_prompt_to_collection),
        )
        // Playbooks
        .route(
            "/api/v1/playbooks",
            get(list_playbooks).post(create_playbook_route),
        )
        .route(
            "/api/v1/playbooks/{id}",
            get(get_playbook).delete(delete_playbook),
        )
        // Search
        .route("/api/v1/search", get(search))
        // Copy
        .route("/api/v1/prompts/{id}/copy", post(record_copy))
}

// ------------------------------------------------------------------
// Shared helpers
// ------------------------------------------------------------------

fn internal_error(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

// ------------------------------------------------------------------
// Health
// ------------------------------------------------------------------

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

// ------------------------------------------------------------------
// Prompts
// ------------------------------------------------------------------

#[derive(Deserialize)]
struct ListPromptsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_prompts(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<ListPromptsQuery>,
) -> Result<Json<Vec<PromptListItem>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let items = prompt_service::list_prompts(&conn, limit, offset).map_err(internal_error)?;
    Ok(Json(items))
}

async fn create_prompt(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreatePromptRequest>,
) -> Result<Json<PromptWithVariants>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let result = prompt_service::create_prompt(&conn, req).map_err(internal_error)?;
    Ok(Json(result))
}

async fn get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<PromptWithVariants>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let result = prompt_service::get_prompt_by_id(&conn, &id).map_err(internal_error)?;
    Ok(Json(result))
}

async fn update_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePromptRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    prompt_service::update_prompt(&conn, &id, req).map_err(internal_error)?;
    Ok(Json(()))
}

async fn delete_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    prompt_service::delete_prompt(&conn, &id).map_err(internal_error)?;
    Ok(Json(()))
}

// ------------------------------------------------------------------
// Variants
// ------------------------------------------------------------------

#[derive(Deserialize)]
struct AddVariantRequest {
    label: String,
    content: String,
}

async fn add_variant(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<AddVariantRequest>,
) -> Result<Json<Variant>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let variant =
        prompt_service::add_variant(&conn, &id, &req.label, &req.content).map_err(internal_error)?;
    Ok(Json(variant))
}

#[derive(Deserialize)]
struct UpdateVariantRequest {
    content: String,
    label: Option<String>,
}

async fn update_variant(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateVariantRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    prompt_service::update_variant(&conn, &id, &req.content, req.label.as_deref())
        .map_err(internal_error)?;
    Ok(Json(()))
}

async fn delete_variant(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    prompt_service::delete_variant(&conn, &id).map_err(internal_error)?;
    Ok(Json(()))
}

// ------------------------------------------------------------------
// Tags
// ------------------------------------------------------------------

async fn list_tags(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Vec<Tag>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let tags = tag_service::list_tags(&conn).map_err(internal_error)?;
    Ok(Json(tags))
}

async fn create_tag(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateTagRequest>,
) -> Result<Json<Tag>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let tag = tag_service::get_or_create_tag(&conn, &req.name).map_err(internal_error)?;
    // If a color was provided, update the tag
    if let Some(ref color) = req.color {
        conn.execute(
            "UPDATE tags SET color = ?1 WHERE id = ?2",
            rusqlite::params![color, tag.id],
        )
        .map_err(internal_error)?;
        return Ok(Json(Tag {
            color: Some(color.clone()),
            ..tag
        }));
    }
    Ok(Json(tag))
}

#[derive(Deserialize)]
struct AddTagsRequest {
    tags: Vec<String>,
}

async fn add_tags_to_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<AddTagsRequest>,
) -> Result<Json<Vec<Tag>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let tags = tag_service::add_tags_to_prompt(&conn, &id, &req.tags).map_err(internal_error)?;
    Ok(Json(tags))
}

async fn remove_tag_from_prompt(
    State(state): State<Arc<ApiState>>,
    Path((prompt_id, tag_id)): Path<(String, String)>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    tag_service::remove_tag_from_prompt(&conn, &prompt_id, &tag_id).map_err(internal_error)?;
    Ok(Json(()))
}

// ------------------------------------------------------------------
// Collections
// ------------------------------------------------------------------

async fn list_collections(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Vec<Collection>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let collections = collection_service::list_collections(&conn).map_err(internal_error)?;
    Ok(Json(collections))
}

async fn create_collection(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<Json<Collection>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let collection = collection_service::create_collection(&conn, req).map_err(internal_error)?;
    Ok(Json(collection))
}

async fn get_collection_prompts(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<PromptListItem>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let items =
        collection_service::get_collection_prompts(&conn, &id, 100, 0).map_err(internal_error)?;
    Ok(Json(items))
}

#[derive(Deserialize)]
struct AddPromptToCollectionRequest {
    prompt_id: String,
}

async fn add_prompt_to_collection(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<AddPromptToCollectionRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    collection_service::add_prompt_to_collection(&conn, &id, &req.prompt_id)
        .map_err(internal_error)?;
    Ok(Json(()))
}

// ------------------------------------------------------------------
// Search
// ------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<PromptListItem>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let items = search_service::search_prompts(&conn, &params.q, 50).map_err(internal_error)?;
    Ok(Json(items))
}

// ------------------------------------------------------------------
// Copy
// ------------------------------------------------------------------

#[derive(Deserialize)]
struct RecordCopyRequest {
    variant_id: Option<String>,
}

#[derive(Serialize)]
struct RecordCopyResponse {
    content: String,
}

async fn record_copy(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<RecordCopyRequest>,
) -> Result<Json<RecordCopyResponse>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let content =
        prompt_service::record_copy(&conn, &id, req.variant_id.as_deref()).map_err(internal_error)?;
    Ok(Json(RecordCopyResponse { content }))
}

// ------------------------------------------------------------------
// Playbooks
// ------------------------------------------------------------------

async fn list_playbooks(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Vec<Playbook>>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let playbooks = playbook_service::list_playbooks(&conn).map_err(internal_error)?;
    Ok(Json(playbooks))
}

#[derive(Deserialize)]
struct CreatePlaybookRequest {
    title: String,
    description: Option<String>,
}

async fn create_playbook_route(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreatePlaybookRequest>,
) -> Result<Json<Playbook>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let playbook =
        playbook_service::create_playbook(&conn, &req.title, req.description.as_deref())
            .map_err(internal_error)?;
    Ok(Json(playbook))
}

async fn get_playbook(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<PlaybookWithSteps>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    let result = playbook_service::get_playbook(&conn, &id).map_err(internal_error)?;
    Ok(Json(result))
}

async fn delete_playbook(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<Json<()>, (StatusCode, String)> {
    let conn = state.db.lock().map_err(internal_error)?;
    playbook_service::delete_playbook(&conn, &id).map_err(internal_error)?;
    Ok(Json(()))
}
