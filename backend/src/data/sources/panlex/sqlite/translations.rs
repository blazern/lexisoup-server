use crate::model::{Sentence, TranslationsSet, WordTranslations};
use axum::http::StatusCode;
use sqlx::SqlitePool;
use tracing::error;

pub async fn get_translations(
    db_pool: &SqlitePool,
    query: &str,
    lang_from_iso3: &str,
    lang_to_iso3: &str,
) -> Result<Option<WordTranslations>, (StatusCode, String)> {
    let uid_from = format!("{lang_from_iso3}-000");
    let uid_to = format!("{lang_to_iso3}-000");
    let query = query.trim();

    let sql = r#"
        WITH src_meanings AS (
          SELECT dnx.mn
          FROM ex
          JOIN lv   ON lv.lv = ex.lv
          JOIN dnx  ON dnx.ex = ex.ex
          WHERE lv.uid = ?1
            AND ex.tt = ?2
        )
        SELECT
          ex_ru.tt               AS txt,
          MAX(COALESCE(d_ru.uq, 0)) AS quality
        FROM src_meanings
        JOIN dnx  AS d_ru  ON d_ru.mn = src_meanings.mn
        JOIN lv   AS lv_ru ON lv_ru.lv = d_ru.lv AND lv_ru.uid = ?3
        JOIN ex   AS ex_ru ON ex_ru.ex = d_ru.ex
        GROUP BY ex_ru.tt
        ORDER BY ex_ru.tt
    "#;

    let rows: Vec<(String, i64)> = sqlx::query_as::<_, (String, i64)>(sql)
        .bind(&uid_from)
        .bind(query)
        .bind(&uid_to)
        .fetch_all(db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to execute PanLex translation+quality query");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if rows.is_empty() {
        return Ok(None);
    }

    let source = "panlex".to_string();

    let mut translations = Vec::with_capacity(rows.len());
    let mut qualities = Vec::with_capacity(rows.len());
    for (txt, q) in rows {
        translations.push(Sentence::new(txt, lang_to_iso3, &source));
        // PanLex quality is 0–9; clamp to i32 just in case.
        let q = (q as i8).clamp(0, 9);
        qualities.push(q);
    }

    let ts = TranslationsSet {
        original: Sentence::new(query, lang_from_iso3, &source),
        translations,
        translations_qualities: Some(qualities),
    };

    Ok(Some(WordTranslations {
        translations_set: ts,
        source,
    }))
}

#[cfg(test)]
mod tests {
    use crate::data::sources::panlex::sqlite::tests_common::new_test_pool;
    use crate::model::{Sentence, TranslationsSet, WordTranslations};

    #[tokio::test]
    async fn translations_happy_path() {
        let pool = new_test_pool().await;
        // Prepare
        sqlx::query(
            r#"-- noinspection SqlNoDataSourceInspectionForFile
                INSERT INTO langvar(id, lang_code, var_code, uid) VALUES
                  (100,'deu',0,'deu-000'),
                  (300,'eng',0,'eng-000');
                INSERT INTO expr(id, langvar, txt) VALUES
                  (1000,100,'Imker'),
                  (3000,300,'beekeeper'),
                  (3001,300,'apiarist');
                INSERT INTO denotationx(meaning, source, grp, quality, expr, langvar) VALUES
                  (9999, 1, 1, 7, 1000, 100),   -- DE "Imker"
                  (9999, 1, 1, 5, 3000, 300),   -- EN "beekeeper" quality 5
                  (9999, 1, 1, 3, 3000, 300),   -- duplicate lower quality -> MAX keeps 5
                  (9999, 1, 1, 12, 3001, 300);  -- EN "apiarist" -> clamp to 9
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = super::get_translations(&pool, " Imker ", "deu", "eng")
            .await
            .expect("ok")
            .expect("some");

        let source = "panlex".to_string();
        let expected = WordTranslations {
            translations_set: TranslationsSet {
                original: Sentence::new("Imker", "deu", &source),
                translations: vec![
                    Sentence::new("apiarist", "eng", &source),
                    Sentence::new("beekeeper", "eng", &source),
                ],
                translations_qualities: Some(vec![9_i8, 5_i8]),
            },
            source: source.clone(),
        };

        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn translations_return_none_when_no_match() {
        let pool = new_test_pool().await;
        let out = super::get_translations(&pool, "Nope", "deu", "eng")
            .await
            .expect("ok");
        assert_eq!(out, None);
    }
}
