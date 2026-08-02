use std::sync::{Arc, MutexGuard};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use rusqlite::Connection;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::server::ApiState;
use crate::error::{AppError, AppResult};
use crate::models::collection::CreateCollectionRequest;
use crate::models::playbook::{StepSpec, UpdatePlaybookRequest};
use crate::models::prompt::{CreatePromptRequest, UpdatePromptRequest};
use crate::models::tag::CreateTagRequest;
use crate::services::{
    collection_service, import_export, playbook_service, prompt_service, search_service,
    tag_service,
};

const MAX_SEARCH_QUERY_CHARS: usize = 512;

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/prompts", get(list_prompts).post(create_prompt))
        .route("/api/v1/prompts/counts", get(get_prompt_counts))
        .route(
            "/api/v1/prompts/{id}",
            get(get_prompt).put(update_prompt).delete(delete_prompt),
        )
        .route("/api/v1/prompts/{id}/usage", get(get_prompt_usage))
        .route("/api/v1/prompts/{id}/variants", post(add_variant))
        .route(
            "/api/v1/variants/{id}",
            put(update_variant).delete(delete_variant),
        )
        .route("/api/v1/tags", get(list_tags).post(create_tag))
        .route("/api/v1/prompts/{id}/tags", post(add_tags_to_prompt))
        .route(
            "/api/v1/prompts/{prompt_id}/tags/{tag_id}",
            delete(remove_tag_from_prompt),
        )
        .route(
            "/api/v1/collections",
            get(list_collections).post(create_collection),
        )
        .route(
            "/api/v1/collections/{id}/prompts",
            get(get_collection_prompts).post(add_prompt_to_collection),
        )
        .route(
            "/api/v1/collections/{id}/prompts/{prompt_id}",
            delete(remove_prompt_from_collection),
        )
        .route(
            "/api/v1/playbooks",
            get(list_playbooks).post(create_playbook),
        )
        .route(
            "/api/v1/playbooks/{id}",
            get(get_playbook)
                .put(update_playbook)
                .delete(delete_playbook),
        )
        .route("/api/v1/playbooks/{id}/steps", post(add_step))
        .route("/api/v1/playbooks/{id}/steps/order", put(reorder_steps))
        .route(
            "/api/v1/playbooks/{id}/steps/{step_id}",
            put(update_step).delete(remove_step),
        )
        .route("/api/v1/import", post(import_prompts))
        .route("/api/v1/export", get(export_prompts))
        .route("/api/v1/search", get(search))
        .route("/api/v1/prompts/{id}/copy", post(record_copy))
}

async fn run_blocking<T, F>(operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| AppError::internal(format!("Blocking task failed: {error}")))?
}

async fn run_mutation<T, F>(state: Arc<ApiState>, operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
{
    let operation_state = state.clone();
    let result = run_blocking(move || {
        let mut conn = lock_db(&operation_state)?;
        operation(&mut conn)
    })
    .await;
    if result.is_ok() {
        state.emit_db_changed();
    }
    result
}

fn lock_db(state: &ApiState) -> AppResult<MutexGuard<'_, Connection>> {
    state
        .db
        .lock()
        .map_err(|_| AppError::internal("Database lock poisoned"))
}

fn json_result<T: Serialize>(result: AppResult<T>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

fn empty_result(result: AppResult<()>) -> Response {
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error_response(error),
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(error: AppError) -> Response {
    let (status, message) = match error {
        AppError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
        AppError::Invalid(message) => (StatusCode::BAD_REQUEST, message),
        AppError::Conflict(message) => (StatusCode::CONFLICT, message),
        AppError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        ),
    };
    (status, Json(ErrorResponse { error: message })).into_response()
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Deserialize)]
struct ListPromptsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    filter: Option<String>,
}

async fn list_prompts(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ListPromptsQuery>,
) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            prompt_service::list_prompts_page(
                &conn,
                query.filter.as_deref(),
                query.limit,
                query.offset,
            )
        })
        .await,
    )
}

async fn create_prompt(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreatePromptRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            prompt_service::create_prompt(conn, request)
        })
        .await,
    )
}

async fn get_prompt(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            prompt_service::get_prompt_by_id(&conn, &id)
        })
        .await,
    )
}

async fn update_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdatePromptRequest>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            prompt_service::update_prompt(conn, &id, request)
        })
        .await,
    )
}

async fn delete_prompt(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> Response {
    empty_result(run_mutation(state, move |conn| prompt_service::delete_prompt(conn, &id)).await)
}

async fn get_prompt_usage(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            prompt_service::get_prompt_usage(&conn, &id)
        })
        .await,
    )
}

async fn get_prompt_counts(State(state): State<Arc<ApiState>>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            prompt_service::get_prompt_counts(&conn)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct AddVariantRequest {
    label: String,
    content: String,
}

async fn add_variant(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<AddVariantRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            prompt_service::add_variant(conn, &id, &request.label, &request.content)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct UpdateVariantRequest {
    content: String,
    label: Option<String>,
}

async fn update_variant(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateVariantRequest>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            prompt_service::update_variant(conn, &id, &request.content, request.label.as_deref())
        })
        .await,
    )
}

async fn delete_variant(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> Response {
    empty_result(run_mutation(state, move |conn| prompt_service::delete_variant(conn, &id)).await)
}

async fn list_tags(State(state): State<Arc<ApiState>>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            tag_service::list_tags(&conn)
        })
        .await,
    )
}

async fn create_tag(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateTagRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            tag_service::create_or_update_tag(conn, request)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct AddTagsRequest {
    tags: Vec<String>,
}

async fn add_tags_to_prompt(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<AddTagsRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            tag_service::add_tags_to_prompt(conn, &id, &request.tags)
        })
        .await,
    )
}

async fn remove_tag_from_prompt(
    State(state): State<Arc<ApiState>>,
    Path((prompt_id, tag_id)): Path<(String, String)>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            tag_service::remove_tag_from_prompt(conn, &prompt_id, &tag_id)
        })
        .await,
    )
}

async fn list_collections(State(state): State<Arc<ApiState>>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            collection_service::list_collections(&conn)
        })
        .await,
    )
}

async fn create_collection(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateCollectionRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            collection_service::create_collection(conn, request)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct PaginationQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn get_collection_prompts(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            collection_service::get_collection_prompts_page(&conn, &id, query.limit, query.offset)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct AddPromptToCollectionRequest {
    prompt_id: String,
}

async fn add_prompt_to_collection(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<AddPromptToCollectionRequest>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            collection_service::add_prompt_to_collection(conn, &id, &request.prompt_id)
        })
        .await,
    )
}

async fn remove_prompt_from_collection(
    State(state): State<Arc<ApiState>>,
    Path((id, prompt_id)): Path<(String, String)>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            collection_service::remove_prompt_from_collection(conn, &id, &prompt_id)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<i64>,
}

async fn search(State(state): State<Arc<ApiState>>, Query(query): Query<SearchQuery>) -> Response {
    if query.q.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return error_response(AppError::invalid("query too long"));
    }
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            search_service::search_prompts_page(&conn, &query.q, query.limit)
        })
        .await,
    )
}

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
    Json(request): Json<RecordCopyRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            prompt_service::record_copy(conn, &id, request.variant_id.as_deref())
                .map(|content| RecordCopyResponse { content })
        })
        .await,
    )
}

async fn import_prompts(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<import_export::ImportData>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            let json = serde_json::to_string(&request).map_err(|error| {
                AppError::internal(format!("Import serialization failed: {error}"))
            })?;
            import_export::import_json(conn, &json)
        })
        .await,
    )
}

async fn export_prompts(State(state): State<Arc<ApiState>>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            let json = import_export::export_json(&conn)?;
            deserialize_internal::<import_export::ExportData>(&json)
        })
        .await,
    )
}

fn deserialize_internal<T: DeserializeOwned>(json: &str) -> AppResult<T> {
    serde_json::from_str(json)
        .map_err(|error| AppError::internal(format!("JSON deserialization failed: {error}")))
}

async fn list_playbooks(State(state): State<Arc<ApiState>>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            playbook_service::list_playbooks(&conn)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct CreatePlaybookRequest {
    title: String,
    description: Option<String>,
}

async fn create_playbook(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreatePlaybookRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            playbook_service::create_playbook(conn, &request.title, request.description.as_deref())
        })
        .await,
    )
}

async fn get_playbook(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> Response {
    json_result(
        run_blocking(move || {
            let conn = lock_db(&state)?;
            playbook_service::get_playbook(&conn, &id)
        })
        .await,
    )
}

async fn update_playbook(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdatePlaybookRequest>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            playbook_service::update_playbook(conn, &id, request)
        })
        .await,
    )
}

async fn add_step(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(spec): Json<StepSpec>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            playbook_service::add_step(conn, &id, spec)
        })
        .await,
    )
}

async fn update_step(
    State(state): State<Arc<ApiState>>,
    Path((id, step_id)): Path<(String, String)>,
    Json(spec): Json<StepSpec>,
) -> Response {
    json_result(
        run_mutation(state, move |conn| {
            playbook_service::update_step(conn, &id, &step_id, spec)
        })
        .await,
    )
}

async fn remove_step(
    State(state): State<Arc<ApiState>>,
    Path((id, step_id)): Path<(String, String)>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            playbook_service::remove_step(conn, &id, &step_id)
        })
        .await,
    )
}

#[derive(Deserialize)]
struct ReorderStepsRequest {
    ordered_step_ids: Vec<String>,
}

async fn reorder_steps(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(request): Json<ReorderStepsRequest>,
) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            playbook_service::reorder_steps(conn, &id, &request.ordered_step_ids)
        })
        .await,
    )
}

async fn delete_playbook(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> Response {
    empty_result(
        run_mutation(state, move |conn| {
            playbook_service::delete_playbook(conn, &id)
        })
        .await,
    )
}
