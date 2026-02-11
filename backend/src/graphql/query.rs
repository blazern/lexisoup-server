use crate::app_state::AppState;
use crate::client_config::ClientConfig;
use crate::data::sources::llm::chatgpt_lexical_items;
use crate::data::sources::panlex::{get_suggestions, get_translations, panlex_lexical_items};
use crate::data::translation::deepl;
use crate::model::{LexicalItemDetail, Suggestion, TranslationResult};
use async_graphql::futures_util::future::join_all;
use async_graphql::{Context, Error, ErrorExtensions, Object};
use tracing::error;

pub struct Query;

#[Object]
impl Query {
    async fn config(&self, ctx: &Context<'_>) -> async_graphql::Result<ClientConfig> {
        let state = ctx.data::<AppState>()?;
        Ok(state.client_config().clone())
    }

    async fn llm(
        &self,
        ctx: &Context<'_>,
        query: String,
        lang_from_iso3: String,
        lang_to_iso3: String,
    ) -> async_graphql::Result<Vec<LexicalItemDetail>> {
        let state = ctx.data::<AppState>()?;
        validate_params(
            &query,
            &lang_from_iso3,
            &lang_to_iso3,
            state.client_config(),
        )?;
        chatgpt_lexical_items::request(
            state.http_client(),
            state.chatgpt_key(),
            &query,
            &lang_from_iso3,
            &lang_to_iso3,
            None,
            None,
        )
        .await
        .map_err(|(status, msg)| {
            Error::new("Upstream LLM error").extend_with(|_, e| {
                e.set("code", "UPSTREAM_LLM");
                e.set("httpStatus", status.as_u16());
                e.set("message", msg);
            })
        })
    }

    async fn panlex(
        &self,
        ctx: &Context<'_>,
        query: String,
        lang_from_iso3: String,
        lang_to_iso3: String,
    ) -> async_graphql::Result<Vec<LexicalItemDetail>> {
        let state = ctx.data::<AppState>()?;
        validate_params(
            &query,
            &lang_from_iso3,
            &lang_to_iso3,
            state.client_config(),
        )?;
        panlex_lexical_items::get(
            state.panlex_sqlite_pool(),
            &query,
            &lang_from_iso3,
            &lang_to_iso3,
        )
        .await
        .map_err(|(status, msg)| {
            Error::new("PanLex SQLite error").extend_with(|_, e| {
                e.set("code", "PANLEX_SQLITE");
                e.set("httpStatus", status.as_u16());
                e.set("message", msg);
            })
        })
    }

    async fn suggestions(
        &self,
        ctx: &Context<'_>,
        query: String,
        lang_from_iso3: String,
        lang_to_iso3: String,
    ) -> async_graphql::Result<Vec<Suggestion>> {
        let state = ctx.data::<AppState>()?;
        validate_params(
            &query,
            &lang_from_iso3,
            &lang_to_iso3,
            state.client_config(),
        )?;
        let pool = state.panlex_sqlite_pool();

        let sql_suggestions = get_suggestions(pool, &query, &lang_from_iso3)
            .await
            .map_err(|(status, msg)| {
                Error::new("PanLex suggestions SQLite error").extend_with(|_, e| {
                    e.set("code", "PANLEX_SQLITE_SUGGESTIONS");
                    e.set("httpStatus", status.as_u16());
                    e.set("message", msg);
                })
            })?;

        let lang_from = lang_from_iso3.clone();
        let lang_to = lang_to_iso3.clone();

        let suggestions = sql_suggestions.into_iter().map(|suggestion| {
            let pool = pool.clone();
            let lang_from = lang_from.clone();
            let lang_to = lang_to.clone();

            async move {
                let translations =
                    match get_translations(&pool, &suggestion.text, &lang_from, &lang_to).await {
                        Ok(opt) => opt
                            .map(|t| t.translations_set.translations)
                            .unwrap_or_default(),
                        Err(e) => {
                            error!(error = %(e.1), "failed to select PanLex translations");
                            vec![]
                        }
                    };
                Suggestion {
                    text: suggestion.text,
                    lang_iso3: lang_from,
                    source: "panlex".to_string(),
                    translations,
                }
            }
        });

        Ok(join_all(suggestions).await)
    }

    async fn translate(
        &self,
        ctx: &Context<'_>,
        texts: Vec<String>,
        lang_from_iso3: String,
        lang_to_iso3: String,
    ) -> async_graphql::Result<TranslationResult> {
        let state = ctx.data::<AppState>()?;
        let config = state.client_config();
        let translate_batch_size_limit = config.translate_batch_size_limit;
        let translate_text_length_min = config.translate_text_length_min;
        let translate_text_length_max = config.translate_text_length_max;
        if config.translate_batch_size_limit < texts.len() {
            return Err(Error::new(format!(
                "texts number must be <={translate_batch_size_limit}"
            ))
            .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")));
        }
        if !(texts.iter().all(|text| {
            (translate_text_length_min..=translate_text_length_max).contains(&text.chars().count())
        })) {
            return Err(Error::new(format!("text length must be within {translate_text_length_min}..={translate_text_length_max}"))
                .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")));
        }
        deepl::request(
            state.http_client(),
            state.deepl_key(),
            state.deepl_endpoint(),
            texts,
            lang_from_iso3,
            lang_to_iso3,
        )
        .await
        .map_err(|(status, msg)| {
            Error::new("Upstream translation error").extend_with(|_, e| {
                e.set("code", "UPSTREAM_TRANSLATION_ERROR");
                e.set("httpStatus", status.as_u16());
                e.set("message", msg);
            })
        })
    }
}

fn validate_params(
    query: &str,
    lang_from_iso3: &str,
    lang_to_iso3: &str,
    config: &ClientConfig,
) -> async_graphql::Result<()> {
    let query = query.trim();
    let min_query_length = config.min_query_length;
    let max_query_length = config.max_query_length;
    if !(min_query_length..=max_query_length).contains(&query.chars().count()) {
        return Err(Error::new(format!(
            "query length must be within {min_query_length}..={max_query_length}"
        ))
        .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")));
    }
    if lang_from_iso3.len() != 3 || lang_to_iso3.len() != 3 {
        return Err(Error::new("languages must be ISO-3 (3 letters)")
            .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")));
    }
    Ok(())
}
