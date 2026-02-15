use crate::data::translation::deepl::lang_mapping::{
    lang_from_iso3_to_deepl, lang_to_iso3_to_deepl,
};
use crate::data::translation::deepl::structs::{DeepLRequest, DeepLResponse};
use crate::model::TranslationResult;
use crate::utils::truncate;
use axum::http::StatusCode;
use reqwest::Client;
use tracing::error;

fn deepl_translate_url(deepl_endpoint: &str) -> String {
    format!("{}/v2/translate", deepl_endpoint.trim_end_matches('/'))
}

pub async fn request(
    http_client: &Client,
    deepl_key: &str,
    deepl_endpoint: &str,
    texts: Vec<String>,
    lang_from_iso3: String,
    lang_to_iso3: String,
) -> Result<TranslationResult, (StatusCode, String)> {
    if texts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "'texts' must not be empty".to_string(),
        ));
    }

    let target_lang = lang_to_iso3_to_deepl(&lang_to_iso3).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!("unsupported target language iso3: {lang_to_iso3}"),
        )
    })?;

    let source_lang = lang_from_iso3_to_deepl(&lang_from_iso3).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!("unsupported source language iso3: {lang_from_iso3}"),
        )
    })?;

    let request_body = DeepLRequest {
        text: &texts,
        target_lang,
        source_lang,
    };

    let url = deepl_translate_url(deepl_endpoint);

    let res = match http_client
        .post(url)
        .header("Authorization", format!("DeepL-Auth-Key {deepl_key}"))
        .json(&request_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "network error talking to upstream");
            return Err((StatusCode::BAD_GATEWAY, e.to_string()));
        }
    };

    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        error!(%status, body = %truncate(&body), "upstream non-success");
        return Err((StatusCode::BAD_GATEWAY, body));
    }

    let parsed: DeepLResponse = match res.json().await {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "failed to deserialize upstream response");
            return Err((StatusCode::BAD_GATEWAY, e.to_string()));
        }
    };

    let translations = parsed.translations.into_iter().map(|t| t.text).collect();

    Ok(TranslationResult {
        translations,
        source: "deepl".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use mockito::{Matcher, Server};
    use serde_json::json;

    const TEST_RESPONSE: &str = r#"
    {
      "translations": [
        { "text": "Hallo, Welt!" }
      ]
    }
    "#;

    #[tokio::test]
    async fn request_returns_translations() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/v2/translate")
            .match_header("authorization", "DeepL-Auth-Key test_key")
            .match_body(Matcher::Json(json!({
                "text": ["Hello, world!"],
                "target_lang": "DE",
                "source_lang": "EN",
            })))
            .with_status(200)
            .with_body(TEST_RESPONSE)
            .create();

        let client = Client::new();
        let result = request(
            &client,
            "test_key",
            &server.url(),
            vec!["Hello, world!".to_string()],
            "eng".to_string(),
            "deu".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(
            result,
            TranslationResult {
                translations: vec!["Hallo, Welt!".to_string()],
                source: "deepl".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn returns_bad_gateway_on_upstream_non_2xx() {
        let mut server = Server::new_async().await;

        let err_body = r#"{"message":"boom"}"#;

        let _m = server
            .mock("POST", "/v2/translate")
            .match_header("authorization", "DeepL-Auth-Key test_key")
            .with_status(500)
            .with_body(err_body)
            .create();

        let client = Client::new();
        let result = request(
            &client,
            "test_key",
            &server.url(),
            vec!["Hello".to_string()],
            "eng".to_string(),
            "deu".to_string(),
        )
        .await;

        let Err((status, body)) = result else {
            panic!("expected Err");
        };

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body, err_body);
    }

    #[tokio::test]
    async fn returns_bad_gateway_on_network_error() {
        let server = Server::new_async().await;
        let url = server.url();
        drop(server); // kill it

        let client = Client::new();
        let err = request(
            &client,
            "test_key",
            &url,
            vec!["Hello".to_string()],
            "eng".to_string(),
            "deu".to_string(),
        )
        .await
        .expect_err("should be Err");

        assert_eq!(err.0, StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn returns_bad_request_on_empty_texts() {
        let client = Client::new();
        let err = request(
            &client,
            "test_key",
            "https://example.invalid",
            vec![],
            "eng".to_string(),
            "deu".to_string(),
        )
        .await
        .expect_err("should be Err");

        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn returns_bad_request_on_unknown_target_language() {
        let client = Client::new();
        let err = request(
            &client,
            "test_key",
            "https://example.invalid",
            vec!["Hello".to_string()],
            "eng".to_string(),
            "zzz".to_string(),
        )
        .await
        .expect_err("should be Err");

        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
}
