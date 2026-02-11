use crate::model::lexical_item_detail::Synonyms;
use crate::model::{Sentence, TranslationsSet};
use axum::http::StatusCode;
use sqlx::SqlitePool;
use tracing::error;

pub async fn get_synonyms(
    db_pool: &SqlitePool,
    query: &str,
    lang_from_iso3: &str,
) -> Result<Option<Synonyms>, (StatusCode, String)> {
    let uid_from = format!("{lang_from_iso3}-000");
    let query = query.trim();

    let sql = r#"
        WITH src_expr AS (
          SELECT ex.ex AS src_ex, dnx.mn
          FROM ex
          JOIN lv   ON lv.lv = ex.lv
          JOIN dnx  ON dnx.ex = ex.ex
          WHERE lv.uid = ?1
            AND ex.tt = ?2
        )
        SELECT
          ex_syn.tt                    AS txt,
          MAX(COALESCE(d_syn.uq, 0))   AS quality
        FROM src_expr
        JOIN dnx  AS d_syn  ON d_syn.mn = src_expr.mn
        JOIN lv   AS lv_syn ON lv_syn.lv = d_syn.lv AND lv_syn.uid = ?1
        JOIN ex   AS ex_syn ON ex_syn.ex = d_syn.ex
        WHERE ex_syn.ex NOT IN (SELECT src_ex FROM src_expr)
          AND ex_syn.tt <> ?2
        GROUP BY ex_syn.tt
        ORDER BY ex_syn.tt
    "#;

    let rows: Vec<(String, i64)> = sqlx::query_as::<_, (String, i64)>(sql)
        .bind(&uid_from)
        .bind(query)
        .fetch_all(db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, "failed to execute PanLex synonyms query");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    if rows.is_empty() {
        return Ok(None);
    }

    let source = "panlex".to_string();
    let mut syns = Vec::with_capacity(rows.len());
    let mut quals = Vec::with_capacity(rows.len());
    for (txt, q) in rows {
        syns.push(Sentence::new(txt, lang_from_iso3, &source));
        let q = (q as i8).clamp(0, 9);
        quals.push(q);
    }

    let ts = TranslationsSet {
        original: Sentence::new(query, lang_from_iso3, &source),
        translations: syns,
        translations_qualities: Some(quals),
    };

    Ok(Some(Synonyms {
        translations_set: ts,
        source,
    }))
}

#[cfg(test)]
mod tests {
    use crate::data::sources::panlex::sqlite::tests_common::new_test_pool;
    use crate::model::lexical_item_detail::Synonyms;
    use crate::model::{Sentence, TranslationsSet};

    #[tokio::test]
    async fn synonyms_happy_path_same_language() {
        let pool = new_test_pool().await;
        sqlx::query(
            r#"-- noinspection SqlNoDataSourceInspectionForFile
            INSERT INTO langvar(id, lang_code, var_code, uid) VALUES
              (100,'deu',0,'deu-000');
            INSERT INTO expr(id, langvar, txt) VALUES
              (1000,100,'Imker'),
              (1001,100,'Bienenhalter'),
              (1002,100,'Bienenzüchter');
            INSERT INTO denotationx(meaning, source, grp, quality, expr, langvar) VALUES
              (9999, 1, 1, 7, 1000, 100),   -- source "Imker"
              (9999, 1, 1, 12, 1001, 100),  -- "Bienenhalter" -> clamp to 9
              (9999, 1, 1, 5,  1002, 100),  -- "Bienenzüchter" quality 5
              (9999, 1, 1, 3,  1002, 100);  -- duplicate lower quality -> MAX keeps 5
        "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = super::get_synonyms(&pool, " Imker ", "deu")
            .await
            .expect("ok")
            .expect("some");

        let source = "panlex".to_string();
        let expected = Synonyms {
            translations_set: TranslationsSet {
                original: Sentence::new("Imker", "deu", &source),
                translations: vec![
                    Sentence::new("Bienenhalter", "deu", &source),
                    Sentence::new("Bienenzüchter", "deu", &source),
                ],
                translations_qualities: Some(vec![9_i8, 5_i8]),
            },
            source: source.clone(),
        };

        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn synonyms_none_when_only_source_exists() {
        let pool = new_test_pool().await;
        sqlx::query(
            r#"-- noinspection SqlNoDataSourceInspectionForFile
            INSERT INTO langvar(id, lang_code, var_code, uid) VALUES
              (100,'deu',0,'deu-000');
            INSERT INTO expr(id, langvar, txt) VALUES
              (1000,100,'Imker');
            INSERT INTO denotationx(meaning, source, grp, quality, expr, langvar) VALUES
              (9999, 1, 1, 7, 1000, 100);
        "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = super::get_synonyms(&pool, "Imker", "deu")
            .await
            .expect("ok");

        assert_eq!(result, None);
    }
}
