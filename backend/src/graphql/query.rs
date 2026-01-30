use crate::app_state::AppState;
use crate::client_config::ClientConfig;
use crate::llm::chatgpt_lexical_items;
use crate::model::{LexicalItemDetail, Suggestion};
use crate::panlex::{get_suggestions, get_translations, panlex_lexical_items};
use async_graphql::futures_util::future::join_all;
use async_graphql::{Context, Error, ErrorExtensions, Object};
use tracing::error;

pub struct Query;

#[Object]
impl Query {
    async fn config(&self) -> async_graphql::Result<ClientConfig> {
        Ok(ClientConfig::default())
    }

    async fn llm(
        &self,
        ctx: &Context<'_>,
        query: String,
        lang_from_iso3: String,
        lang_to_iso3: String,
    ) -> async_graphql::Result<Vec<LexicalItemDetail>> {
        validate_params(&query, &lang_from_iso3, &lang_to_iso3)?;
        let state = ctx.data::<AppState>()?;
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
        validate_params(&query, &lang_from_iso3, &lang_to_iso3)?;
        let state = ctx.data::<AppState>()?;
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
        validate_params(&query, &lang_from_iso3, &lang_to_iso3)?;
        let state = ctx.data::<AppState>()?;
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
}

fn validate_params(
    query: &str,
    lang_from_iso3: &str,
    lang_to_iso3: &str,
) -> async_graphql::Result<()> {
    let query = query.trim();
    if query.is_empty() {
        return Err(Error::new("query must not be empty")
            .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")));
    } else if MAX_QUERY_LEN < query.len() {
        return Err(
            Error::new(format!("query must not longer than {MAX_QUERY_LEN}"))
                .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")),
        );
    }
    if lang_from_iso3.len() != 3 || lang_to_iso3.len() != 3 {
        return Err(Error::new("languages must be ISO-3 (3 letters)")
            .extend_with(|_, e| e.set("code", "BAD_USER_INPUT")));
    }
    Ok(())
}

const MAX_QUERY_LEN: usize = 50;
