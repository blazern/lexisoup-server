use axum::{
    body::Body,
    extract::{Query, State},
    http::{Response, StatusCode, header},
};
use serde::Deserialize;
use tracing::error;

use crate::app_state::AppState;

#[derive(Deserialize)]
pub struct KaikkiQuery {
    pub query: String,
    pub lang_iso3: String,
}

pub async fn kaikki_proxy(
    State(state): State<AppState>,
    Query(params): Query<KaikkiQuery>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let query = params.query.trim();
    if query.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "query must not be empty".into()));
    }

    let sub = match subwiktionary_of(params.lang_iso3.trim()) {
        Some(s) => s,
        None => {
            return Err((StatusCode::BAD_REQUEST, "unsupported lang".into()));
        }
    };

    let url = format!(
        "https://kaikki.org/{}/meaning/{}",
        sub,
        query_page_postfix(query),
    );

    let upstream = match state.http_client().get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, %url, "network error talking to Kaikki");
            return Err((StatusCode::BAD_GATEWAY, "Kaikki upstream error".into()));
        }
    };

    let status = upstream.status();
    let headers = upstream.headers().clone();

    let body_bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            error!(error = %e, "failed to read Kaikki response body");
            return Err((StatusCode::BAD_GATEWAY, "Kaikki upstream error".into()));
        }
    };

    // Build a response that mirrors Kaikki as closely as is reasonable:
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

fn subwiktionary_of(lang_iso3: &str) -> Option<&'static str> {
    match lang_iso3 {
        "rus" => Some("ruwiktionary/Русский"),
        "eng" => Some("dictionary/English"),
        "deu" => Some("dewiktionary/Deutsch"),
        "fra" => Some("frwiktionary/Français"),
        _ => None,
    }
}

fn query_page_postfix(query: &str) -> String {
    let mut chars = query.chars();

    let first = chars.next().unwrap_or('_');
    let second = chars.next().unwrap_or(first);

    let mut first_two = String::new();
    first_two.push(first);
    first_two.push(second);

    format!("{first}/{first_two}/{query}.jsonl")
}
