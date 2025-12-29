use axum::http::StatusCode;
use sqlx::SqlitePool;
use std::cmp::Reverse;
use std::time::Instant;
use tracing::error;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlSuggestion {
    pub text: String,
    pub distance: i32,
    pub denotation_count: i32,
}

// NOTE: the PanLex DB on itself does not contain the necessary tables
// to get suggestions from, so the code belows heavily relies on the CI/CD
// running the necessary migrations (see the "scripts" folder in the repo root).
pub async fn get_suggestions(
    db_pool: &SqlitePool,
    query: &str,
    lang_from_iso3: &str,
) -> Result<Vec<SqlSuggestion>, (StatusCode, String)> {
    let start = Instant::now();

    let query = query.trim();
    let suggestions = select_suggestions(db_pool, lang_from_iso3, query).await?;
    let suggestions = filter_suggestions(suggestions);

    let elapsed = start.elapsed();
    tracing::debug!(
        elapsed_ms = elapsed.as_millis(),
        "get_suggestions completed"
    );

    Ok(suggestions)
}

async fn select_suggestions(
    db_pool: &SqlitePool,
    lang_from_iso3: &str,
    query: &str,
) -> Result<Vec<SqlSuggestion>, (StatusCode, String)> {
    let uid_from = format!("{lang_from_iso3}-000");
    let sql = r#"
        SELECT
          s.word,
          s.distance,
          e.denotation_count
        FROM spell AS s
        JOIN expr  AS e
          ON e.langvar = s.langid
         AND e.txt = s.word
        WHERE s.langid = (SELECT lv FROM lv WHERE uid = ?2)
          AND s.word MATCH ?1
          AND s.scope = 1
        ORDER BY s.score ASC;
    "#;

    let rows = sqlx::query_as::<_, (String, i32, i32)>(sql)
        .bind(query)
        .bind(&uid_from)
        .fetch_all(db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to execute PanLex suggestions query");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    let mut suggestions = Vec::with_capacity(rows.len());
    for (txt, distance, denotation_count) in rows {
        suggestions.push(SqlSuggestion {
            text: txt,
            distance,
            denotation_count,
        });
    }
    Ok(suggestions)
}

fn filter_suggestions(mut suggestions: Vec<SqlSuggestion>) -> Vec<SqlSuggestion> {
    if suggestions.is_empty() {
        return suggestions;
    }

    // The PanLex DB by its nature has a lot of rare or straight trash data, so we filter
    // the found results heavily

    // Let's filter out any suggestions that have [denotation_count] less than the 50th percentile
    // of all suggestions.
    let mut denotation_counts: Vec<i32> = suggestions.iter().map(|s| s.denotation_count).collect();
    denotation_counts.sort_unstable();
    let center_dc_threshold = center(&denotation_counts).expect("suggestions not empty");

    // Let's filter out any suggestions [denotation_count] of which is less than 5% of the
    // first match (the first match should be the best one, so we use it as the reference).
    let first = suggestions.remove(0);
    let reference_dc = first.denotation_count;
    let min_percent_dc = 0.05;
    let dc_percent_threshold = ((reference_dc as f64) * min_percent_dc) as i32;

    let mut result: Vec<SqlSuggestion> = suggestions
        .iter()
        .filter(|&s| {
            dc_percent_threshold <= s.denotation_count && center_dc_threshold <= s.denotation_count
        })
        .cloned()
        .collect();

    // The user is very likely to want more popular words, so we sort them by "popularity".
    result.sort_by_key(|s| Reverse(s.denotation_count));
    // The first match should be the best, so we always keep it.
    result.insert(0, first);
    // If the found word is too far away from the query, remove it (prevents finding any results
    // for queries with random letters like "asdkahsdksad").
    result.retain(|s| s.distance <= 100);
    result
}

fn center(v: &[i32]) -> Option<i32> {
    if v.is_empty() {
        None
    } else {
        Some(v[v.len() / 2])
    }
}

#[cfg(test)]
mod tests {
    use crate::panlex::sqlite::tests_common::new_test_pool;

    #[tokio::test]
    async fn suggestions_include_expected_words() {
        let pool = new_test_pool().await;

        sqlx::query(
            r#"-- noinspection SqlNoDataSourceInspectionForFile
                INSERT INTO langvar(id, lang_code, var_code, uid) VALUES(1, 'eng', 0, 'eng-000');"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"-- noinspection SqlNoDataSourceInspectionForFile
            INSERT INTO expr(id, langvar, txt, denotation_count) VALUES
               (1, 1, 'running', 500),
               (2, 1, 'run',  300),
               (3, 1, 'ran', 300);"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"-- noinspection SqlNoDataSourceInspectionForFile
                INSERT INTO spell(word, rank, langid, soundslike)
                SELECT
                  txt,
                  denotation_count,
                  langvar,
                  lower(spellfix1_translit(txt))
                FROM expr;"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let got = super::get_suggestions(&pool, "rnnning", "eng")
            .await
            .expect("suggestions");

        assert!(got.iter().any(|w| w.text == "running"), "{got:?}");
    }
}
