use super::chatgpt_structs::{ChatGPTRequest, ChatGPTResponse};
use crate::utils::truncate;
use axum::http::StatusCode;
use reqwest::Client;
use tracing::error;

const DEFAULT_MODEL: &str = "gpt-4.1";

pub async fn request(
    http_client: &Client,
    chatgpt_key: &str,
    query: &str,
    model: Option<&str>,
    url: Option<&str>,
) -> Result<String, (StatusCode, String)> {
    let model = model.unwrap_or(DEFAULT_MODEL);
    let query = query.replace('\n', " ").trim().to_string();
    let request_body = ChatGPTRequest {
        model,
        input: &query,
    };

    let url = url.unwrap_or("https://api.openai.com/v1/responses");
    let res = match http_client
        .post(url)
        .bearer_auth(chatgpt_key)
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

    let parsed: ChatGPTResponse = match res.json().await {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "failed to deserialize upstream response");
            return Err((StatusCode::BAD_GATEWAY, e.to_string()));
        }
    };

    let answer = parsed
        .output
        .first()
        .and_then(|o| o.content.first())
        .map(|c| c.text.clone())
        .ok_or_else(|| {
            error!("missing output[0].content[0].text in upstream response");
            (
                StatusCode::BAD_GATEWAY,
                "missing `output[0].content[0].text` in upstream response".into(),
            )
        })?;

    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use mockito::{Matcher, Server};
    use serde_json::json;

    async fn call(
        response_status: usize,
        response_body: &str,
        input_sent: &str,
    ) -> Result<String, (StatusCode, String)> {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/v1/responses")
            .match_body(Matcher::Json(json!({
                "model": DEFAULT_MODEL,
                "input": input_sent
            })))
            .with_status(response_status)
            .with_body(response_body)
            .create();

        let client = Client::new();
        let url = format!("{}/v1/responses", server.url());

        request(&client, "test_key", input_sent, None, Some(&url)).await
    }

    const TEST_RESPONSE: &str = r#"
        {
          "output": [
            {
              "content": [
                { "text": "42 is the answer.", "type": "str" }
              ],
              "id": "123",
              "type": "str",
              "status": "ok",
              "role": "assistant"
            }
          ],
          "status": "ok",
          "model": "4o",
          "usage": {
            "input_tokens": 10,
            "output_tokens": 20,
            "total_tokens": 12345
          }
        }
        "#;

    #[tokio::test]
    async fn request_returns_model_answer() {
        let result = call(200, TEST_RESPONSE, "What is the answer to life?")
            .await
            .unwrap();
        assert_eq!(result, "42 is the answer.");
    }

    #[tokio::test]
    async fn returns_bad_gateway_on_upstream_non_2xx() {
        let err_body = r#"{"error":"boom"}"#;

        let result = call(500, err_body, "Will this fail?").await;

        let Err((status, body)) = result else {
            panic!("expected Err");
        };
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body, err_body);
    }

    #[tokio::test]
    async fn returns_bad_gateway_on_network_error() {
        let server = Server::new_async().await;
        let url = format!("{}/v1/responses", server.url());
        // Let's kill the server - all requests to it will fail
        drop(server);

        let client = Client::new();
        let err = request(&client, "test_key", "any", None, Some(&url))
            .await
            .expect_err("should be Err");

        assert_eq!(err.0, StatusCode::BAD_GATEWAY);
    }
}
