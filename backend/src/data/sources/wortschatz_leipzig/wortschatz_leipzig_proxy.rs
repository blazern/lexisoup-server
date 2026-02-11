use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{Response, StatusCode, header},
};
use serde::Deserialize;
use tracing::error;

use crate::app_state::AppState;

#[derive(Deserialize)]
pub struct LeipzigQueryParams {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

/// Proxies:
///   /ws/sentences/{corpus}/sentences/{term}?offset=&limit=
/// to:
///   https://api.wortschatz-leipzig.de/ws/sentences/{corpus}/sentences/{term}?offset=&limit=
pub async fn wortschatz_leipzig_proxy(
    State(state): State<AppState>,
    Path((corpus, term)): Path<(String, String)>,
    Query(params): Query<LeipzigQueryParams>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let corpus = corpus.trim();
    let term = term.trim();

    if corpus.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "corpus must not be empty".into()));
    }

    if term.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "term must not be empty".into()));
    }

    let url = format!("https://api.wortschatz-leipzig.de/ws/sentences/{corpus}/sentences/{term}");

    let mut req = state.http_client().get(&url);

    if let Some(offset) = params.offset {
        req = req.query(&[("offset", offset)]);
    }
    if let Some(limit) = params.limit {
        req = req.query(&[("limit", limit)]);
    }

    let upstream = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, %url, "network error talking to Leipzig");
            return Err((
                StatusCode::BAD_GATEWAY,
                "Wortschatz Leipzig upstream error".into(),
            ));
        }
    };

    let status = upstream.status();
    let headers = upstream.headers().clone();

    let body_bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            error!(error = %e, "failed to read Leipzig response body");
            return Err((
                StatusCode::BAD_GATEWAY,
                "Wortschatz Leipzig upstream error".into(),
            ));
        }
    };

    let mut builder = Response::builder().status(status);
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }

    let resp = builder
        .body(Body::from(body_bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(resp)
}
