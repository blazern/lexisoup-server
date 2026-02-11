use super::chatgpt;
use crate::model::{
    LexicalItemDetail, Sentence, TranslationsSet,
    lexical_item_detail::{Example, Explanation, Forms, Synonyms, WordTranslations},
};
use crate::utils::truncate;
use axum::http::StatusCode;
use reqwest::Client;
use serde::Deserialize;
use tracing::error;

pub async fn request(
    http_client: &Client,
    chatgpt_key: &str,
    query: &str,
    lang_from_iso3: &str,
    lang_to_iso3: &str,
    model: Option<&str>,
    url: Option<&str>,
) -> Result<Vec<LexicalItemDetail>, (StatusCode, String)> {
    let prompt = build_prompt(query, lang_from_iso3, lang_to_iso3);
    let raw = chatgpt::request(http_client, chatgpt_key, &prompt, model, url).await?;
    let json = extract_json_object(&raw).unwrap_or_else(|| raw.trim().to_string());

    let resp: ChatGPTLexicalResponse = serde_json::from_str(&json).map_err(|e| {
        error!(error = %e, sample = %truncate(&json), "invalid JSON from model");
        (
            StatusCode::BAD_GATEWAY,
            format!("invalid JSON from ChatGPT: {e}"),
        )
    })?;

    let source = "chatgpt".to_string();
    let mut out = Vec::<LexicalItemDetail>::new();

    out.push(LexicalItemDetail::Forms(Forms {
        text: resp.forms,
        source: source.clone(),
    }));

    out.push(LexicalItemDetail::Explanation(Explanation {
        text: resp.explanation,
        source: source.clone(),
    }));

    let translations_set = TranslationsSet {
        original: Sentence::new(query, lang_from_iso3, &source),
        translations: resp
            .translations
            .into_iter()
            .map(|t| Sentence::new(t, lang_to_iso3, &source))
            .collect(),
        translations_qualities: None,
    };
    out.push(LexicalItemDetail::WordTranslations(WordTranslations {
        translations_set,
        source: source.clone(),
    }));

    let synonyms_set = TranslationsSet {
        original: Sentence::new(query, lang_from_iso3, &source),
        translations: resp
            .synonyms
            .into_iter()
            .map(|s| Sentence::new(s, lang_from_iso3, &source))
            .collect(),
        translations_qualities: None,
    };
    out.push(LexicalItemDetail::Synonyms(Synonyms {
        translations_set: synonyms_set,
        source: source.clone(),
    }));

    for ex in resp.examples {
        if let Some((lhs, rhs)) = ex.split_once('|') {
            let examples_ts = TranslationsSet {
                original: Sentence::new(lhs.trim(), lang_from_iso3, &source),
                translations: vec![Sentence::new(rhs.trim(), lang_to_iso3, &source)],
                translations_qualities: None,
            };
            out.push(LexicalItemDetail::Example(Example {
                translations_set: examples_ts,
                source: source.clone(),
            }));
        }
    }

    Ok(out)
}

#[derive(Deserialize)]
struct ChatGPTLexicalResponse {
    forms: String,
    translations: Vec<String>,
    synonyms: Vec<String>,
    explanation: String,
    examples: Vec<String>,
}

/// Prompt builder (adapted from your Android code).
fn build_prompt(query: &str, lang_from_iso3: &str, lang_to_iso3: &str) -> String {
    let forms_explanation = r#"
if noun: article, singular form, plural form changes, e.g.:
der Hund, -e
der Platz, -äe
der Wurm, -(ü)e
if verb: follow next examples:
gehen, geht, ging, ist gegangen
lieben, liebt, liebte, hat geliebt
for others (adverb, adjective) make it as simple as possible
"#;

    format!(
        r#"
You are called from a language learning app. Your goal is to reply with **JSON only**, no prose, no code fences.

The JSON format must be exactly:
{{
  "forms": "<FORMS>",
  "translations": ["<TRANSLATION>", "<TRANSLATION>", "<TRANSLATION>"],
  "synonyms": ["<SYNONYM>", "<SYNONYM>", "<SYNONYM>"],
  "explanation": "<EXPLANATION_TARGET_LANG>",
  "examples": [
    "<EXAMPLE>",
    "<EXAMPLE>",
    "<EXAMPLE>",
    "<EXAMPLE>",
    "<EXAMPLE>"
  ]
}}

Word to explain: {query}
Source language (ISO-3): {lang_from_iso3}
Target language (ISO-3): {lang_to_iso3}

Placeholders:
<FORMS>: {forms_explanation}
<TRANSLATION>: a translation into lang {lang_to_iso3}
<SYNONYM>: a synonym in lang {lang_from_iso3}
<EXPLANATION_TARGET_LANG>: short (2-3 sentences) explanation of the word, in lang {lang_to_iso3}
<EXAMPLE>: example sentence in lang {lang_from_iso3} | example sentence in lang {lang_to_iso3}
The '|' is a required delimiter. Example sentences must be short. Translations and synonyms may contain 1-6 entries.
"#
    )
}

/// Try to recover a JSON object from a model reply, even if wrapped in prose or ``` fences.
/// Returns `Some(json)` if we can locate `{ ... }`, otherwise `None`.
fn extract_json_object(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut first = None;
    let mut last = None;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' {
            first = Some(i);
            break;
        }
    }
    for (i, &b) in bytes.iter().enumerate().rev() {
        if b == b'}' {
            last = Some(i);
            break;
        }
    }
    match (first, last) {
        (Some(start), Some(end)) if end >= start => Some(s[start..=end].to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use mockito::Server;
    use reqwest::Client;
    use serde_json::json;

    use crate::model::{
        LexicalItemDetail, Sentence, TranslationsSet,
        lexical_item_detail::{Example, Explanation, Forms, Synonyms, WordTranslations},
    };

    fn wrap_in_chatgpt_response(payload: &str) -> String {
        json!({
            "output": [
                {
                    "content": [ { "text": payload, "type": "str" } ],
                    "id": "123",
                    "type": "str",
                    "status": "ok",
                    "role": "assistant"
                }
            ],
            "status": "ok",
            "model": "4o",
            "usage": { "input_tokens": 10, "output_tokens": 20, "total_tokens": 30 }
        })
        .to_string()
    }

    const LEX_JSON: &str = r#"
    {
      "forms": "der Hund, -e",
      "translations": ["dog", "hound"],
      "synonyms": ["Hündin", "Köter"],
      "explanation": "Der Hund ist ein Haustier.",
      "examples": [
        "Hund|Dog",
        "Mein Hund|My dog"
      ]
    }"#;

    pub async fn request_lexical(
        http_client: &Client,
        query: &str,
        lang_from_iso3: &str,
        lang_to_iso3: &str,
        url: Option<&str>,
    ) -> Result<Vec<LexicalItemDetail>, (StatusCode, String)> {
        super::request(
            http_client,
            "key",
            query,
            lang_from_iso3,
            lang_to_iso3,
            None,
            url,
        )
        .await
    }

    #[tokio::test]
    async fn good_scenario() {
        let mut server = Server::new_async().await;

        let body = wrap_in_chatgpt_response(LEX_JSON);
        let _m = server
            .mock("POST", "/v1/responses")
            .with_status(200)
            .with_body(body)
            .create();

        let client = Client::new();
        let url = format!("{}/v1/responses", server.url());

        let items = request_lexical(&client, "Hund", "deu", "eng", Some(&url))
            .await
            .expect("Ok");

        let source = "chatgpt".to_string();

        let expected = {
            let wt = TranslationsSet {
                original: Sentence::new("Hund", "deu", &source),
                translations: vec![
                    Sentence::new("dog", "eng", &source),
                    Sentence::new("hound", "eng", &source),
                ],
                translations_qualities: None,
            };
            let syn = TranslationsSet {
                original: Sentence::new("Hund", "deu", &source),
                translations: vec![
                    Sentence::new("Hündin", "deu", &source),
                    Sentence::new("Köter", "deu", &source),
                ],
                translations_qualities: None,
            };
            let ex1 = TranslationsSet {
                original: Sentence::new("Hund", "deu", &source),
                translations: vec![Sentence::new("Dog", "eng", &source)],
                translations_qualities: None,
            };
            let ex2 = TranslationsSet {
                original: Sentence::new("Mein Hund", "deu", &source),
                translations: vec![Sentence::new("My dog", "eng", &source)],
                translations_qualities: None,
            };
            vec![
                LexicalItemDetail::Forms(Forms {
                    text: "der Hund, -e".into(),
                    source: source.clone(),
                }),
                LexicalItemDetail::Explanation(Explanation {
                    text: "Der Hund ist ein Haustier.".into(),
                    source: source.clone(),
                }),
                LexicalItemDetail::WordTranslations(WordTranslations {
                    translations_set: wt,
                    source: source.clone(),
                }),
                LexicalItemDetail::Synonyms(Synonyms {
                    translations_set: syn,
                    source: source.clone(),
                }),
                LexicalItemDetail::Example(Example {
                    translations_set: ex1,
                    source: source.clone(),
                }),
                LexicalItemDetail::Example(Example {
                    translations_set: ex2,
                    source: source.clone(),
                }),
            ]
        };

        assert_eq!(items, expected);
    }

    #[tokio::test]
    async fn malformed_json_in_model_text() {
        let mut server = Server::new_async().await;

        let body = wrap_in_chatgpt_response("{ error }");
        let _m = server
            .mock("POST", "/v1/responses")
            .with_status(200)
            .with_body(body)
            .create();

        let client = Client::new();
        let url = format!("{}/v1/responses", server.url());

        let err = request_lexical(&client, "dog", "eng", "deu", Some(&url))
            .await
            .expect_err("Err");

        assert_eq!(err.0, StatusCode::BAD_GATEWAY);
        assert!(err.1.starts_with("invalid JSON from ChatGPT:"));
    }

    #[tokio::test]
    async fn json_inside_code_fences_is_accepted() {
        let mut server = Server::new_async().await;

        let fenced = format!("```json\n{LEX_JSON}\n```");
        let body = wrap_in_chatgpt_response(&fenced);
        let _m = server
            .mock("POST", "/v1/responses")
            .with_status(200)
            .with_body(body)
            .create();

        let client = Client::new();
        let url = format!("{}/v1/responses", server.url());

        let items = request_lexical(&client, "Hund", "deu", "eng", Some(&url))
            .await
            .expect("Ok");
        assert_ne!(Vec::<LexicalItemDetail>::new(), items);
    }

    #[tokio::test]
    async fn upstream_non_2xx_bubbles_as_error() {
        let mut server = Server::new_async().await;

        let _m = server
            .mock("POST", "/v1/responses")
            .with_status(500)
            .with_body(r#"{"error":"boom"}"#)
            .create();

        let client = Client::new();
        let url = format!("{}/v1/responses", server.url());

        let err = request_lexical(&client, "any", "eng", "deu", Some(&url))
            .await
            .expect_err("Err");

        assert_eq!(err.0, StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn network_error_is_propagated() {
        let server = Server::new_async().await;
        let url = format!("{}/v1/responses", server.url());
        drop(server); // kill server

        let client = Client::new();

        let err = request_lexical(&client, "any", "eng", "deu", Some(&url))
            .await
            .expect_err("Err");

        assert_eq!(err.0, StatusCode::BAD_GATEWAY);
    }
}
