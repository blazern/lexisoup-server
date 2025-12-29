use crate::app_state::AppState;
use crate::llm::chatgpt_lexical_items;
use crate::model::{LexicalItemDetail, Suggestion};
use crate::panlex::{get_suggestions, panlex_lexical_items};
use async_graphql::{Context, Error, ErrorExtensions, Object};

pub struct Query;

#[Object]
impl Query {
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
        get_suggestions(state.panlex_sqlite_pool(), &query, &lang_from_iso3)
            .await
            .map(|suggestions| {
                suggestions
                    .iter()
                    .map(|suggestion| Suggestion {
                        text: suggestion.text.clone(),
                        lang_iso3: lang_from_iso3.to_string(),
                        source: "panlex".to_string(),
                    })
                    .collect()
            })
            .map_err(|(status, msg)| {
                Error::new("PanLex suggestions SQLite error").extend_with(|_, e| {
                    e.set("code", "PANLEX_SQLITE_SUGGESTIONS");
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
