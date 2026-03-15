use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use super::server::ApiState;

/// Axum middleware that validates Bearer token authentication.
/// The `/api/v1/health` endpoint is public and does not require auth.
/// All other endpoints require `Authorization: Bearer <key>` matching the API key.
pub async fn auth_middleware(
    State(state): State<Arc<ApiState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Allow health endpoint without auth
    if req.uri().path() == "/api/v1/health" {
        return Ok(next.run(req).await);
    }

    // Extract and validate the Bearer token
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == state.api_key {
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
