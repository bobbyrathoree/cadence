use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::server::ApiState;
use crate::models::collection::CreateCollectionRequest;
use crate::models::prompt::{CreatePromptRequest, UpdatePromptRequest};
use crate::models::tag::{CreateTagRequest, Tag};
use crate::services::{collection_service, import_export, playbook_service, prompt_service, search_service, tag_service};

const DEFAULT_PAGE_SIZE: i64 = 100;
const MAX_PAGE_SIZE: i64 = 500;
const MAX_SEARCH_QUERY_CHARS: usize = 512;

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
        // Import / Export
        .route("/api/v1/import", post(import_prompts))
        .route("/api/v1/export", get(export_prompts))
        // Search
        .route("/api/v1/search", get(search))
        // Copy
        .route("/api/v1/prompts/{id}/copy", post(record_copy))
}

// ------------------------------------------------------------------
// Shared helpers
// ------------------------------------------------------------------

fn sanitize_pagination(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<(i64, i64), (StatusCode, &'static str)> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let offset = offset.unwrap_or(0);

    if limit < 1 {
        return Err((StatusCode::BAD_REQUEST, "limit must be at least 1"));
    }

    if offset < 0 {
        return Err((StatusCode::BAD_REQUEST, "offset must be at least 0"));
    }

    Ok((limit.min(MAX_PAGE_SIZE), offset))
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
) -> impl IntoResponse {
    let (limit, offset) = match sanitize_pagination(params.limit, params.offset) {
        Ok(values) => values,
        Err(err) => return err.into_response(),
    };

    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::list_prompts(&conn, limit, offset).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(items) => Json(items).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn create_prompt(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreatePromptRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::create_prompt(&conn, req).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(prompt) => Json(prompt).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::get_prompt_by_id(&conn, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(prompt) => Json(prompt).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePromptRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::update_prompt(&conn, &id, req).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::delete_prompt(&conn, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
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
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::add_variant(&conn, &id, &req.label, &req.content)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(variant) => Json(variant).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
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
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::update_variant(&conn, &id, &req.content, req.label.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_variant(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::delete_variant(&conn, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ------------------------------------------------------------------
// Tags
// ------------------------------------------------------------------

async fn list_tags(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        tag_service::list_tags(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(tags) => Json(tags).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn create_tag(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateTagRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let tag = tag_service::get_or_create_tag(&conn, &req.name).map_err(|e| e.to_string())?;
        // If a color was provided, update the tag
        if let Some(ref color) = req.color {
            conn.execute(
                "UPDATE tags SET color = ?1 WHERE id = ?2",
                rusqlite::params![color, tag.id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(Tag {
                color: Some(color.clone()),
                ..tag
            });
        }
        Ok(tag)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(tag) => Json(tag).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
struct AddTagsRequest {
    tags: Vec<String>,
}

async fn add_tags_to_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<AddTagsRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        tag_service::add_tags_to_prompt(&conn, &id, &req.tags).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(tags) => Json(tags).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn remove_tag_from_prompt(
    State(state): State<Arc<ApiState>>,
    Path((prompt_id, tag_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        tag_service::remove_tag_from_prompt(&conn, &prompt_id, &tag_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ------------------------------------------------------------------
// Collections
// ------------------------------------------------------------------

async fn list_collections(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        collection_service::list_collections(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(collections) => Json(collections).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn create_collection(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateCollectionRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        collection_service::create_collection(&conn, req).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(collection) => Json(collection).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_collection_prompts(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        collection_service::get_collection_prompts(&conn, &id, 100, 0)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(items) => Json(items).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
struct AddPromptToCollectionRequest {
    prompt_id: String,
}

async fn add_prompt_to_collection(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<AddPromptToCollectionRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        collection_service::add_prompt_to_collection(&conn, &id, &req.prompt_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
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
) -> impl IntoResponse {
    if params.q.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return (StatusCode::BAD_REQUEST, "query too long").into_response();
    }

    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        search_service::search_prompts(&conn, &params.q, 50).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(items) => Json(items).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
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
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        prompt_service::record_copy(&conn, &id, req.variant_id.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(content) => Json(RecordCopyResponse { content }).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ------------------------------------------------------------------
// Import / Export
// ------------------------------------------------------------------

async fn import_prompts(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<import_export::ImportData>,
) -> impl IntoResponse {
    let json_str = serde_json::to_string(&req).unwrap_or_default();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        import_export::import_json(&conn, &json_str).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(import_result) => Json(import_result).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn export_prompts(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        import_export::export_json(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(json_str) => {
            // Parse the string back to ExportData so axum serializes it as JSON
            match serde_json::from_str::<import_export::ExportData>(&json_str) {
                Ok(data) => Json(data).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ------------------------------------------------------------------
// Playbooks
// ------------------------------------------------------------------

async fn list_playbooks(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        playbook_service::list_playbooks(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(playbooks) => Json(playbooks).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
struct CreatePlaybookRequest {
    title: String,
    description: Option<String>,
}

async fn create_playbook_route(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreatePlaybookRequest>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        playbook_service::create_playbook(&conn, &req.title, req.description.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(playbook) => Json(playbook).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_playbook(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        playbook_service::get_playbook(&conn, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(playbook) => Json(playbook).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_playbook(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        playbook_service::delete_playbook(&conn, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    match result {
        Ok(()) => Json(()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
