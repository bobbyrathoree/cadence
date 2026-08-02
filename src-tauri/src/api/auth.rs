use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use subtle::ConstantTimeEq;

use super::server::ApiState;

/// Axum middleware that validates Bearer token authentication.
/// The `/api/v1/health` endpoint is public and does not require auth.
/// All other endpoints require `Authorization: Bearer <key>` matching the API key.
pub async fn auth_middleware(
    State(state): State<Arc<ApiState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let host = req
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok());
    let loopback = format!("127.0.0.1:{}", state.api_port);
    let localhost = format!("localhost:{}", state.api_port);
    if !matches!(host, Some(value) if value == loopback || value == localhost) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Host validation intentionally runs before this public health bypass.
    if req.uri().path() == "/api/v1/health" {
        return Ok(next.run(req).await);
    }

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) => {
            let Some((scheme, token)) = header.split_once(' ') else {
                return Err(StatusCode::UNAUTHORIZED);
            };
            if !scheme.eq_ignore_ascii_case("bearer") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            if bool::from(token.as_bytes().ct_eq(state.api_key.as_bytes())) {
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
