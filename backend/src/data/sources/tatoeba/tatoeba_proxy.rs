use axum::{
    body::Body,
    extract::{Query, State},
    http::{Response, StatusCode, header},
};
use std::collections::HashMap;
use tracing::error;

use crate::app_state::AppState;

pub async fn tatoeba_proxy(
    State(state): State<AppState>,
    Query(mut params): Query<HashMap<String, String>>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let query = match params.get_mut("query") {
        Some(q) => {
            let trimmed = q.trim().to_string();
            if trimmed.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "query must not be empty".into()));
            }
            *q = trimmed.clone();
            trimmed
        }
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "missing required parameter `query`".into(),
            ));
        }
    };

    let url = "https://tatoeba.org/en/api_v0/search";
    let upstream = match state.http_client().get(url).query(&params).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, %url, %query, "network error talking to Tatoeba");
            return Err((StatusCode::BAD_GATEWAY, "Tatoeba upstream error".into()));
        }
    };

    let status = upstream.status();
    let headers = upstream.headers().clone();

    let body_bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            error!(error = %e, "failed to read Tatoeba response body");
            return Err((StatusCode::BAD_GATEWAY, "Tatoeba upstream error".into()));
        }
    };

    let mut builder = Response::builder().status(status);
    // Preserve content-type if present
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }

    let resp = builder
        .body(Body::from(body_bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(resp)
}
