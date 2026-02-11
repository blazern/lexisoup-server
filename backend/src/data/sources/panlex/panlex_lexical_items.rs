use crate::data::sources::panlex::sqlite::synonyms::get_synonyms;
use crate::data::sources::panlex::sqlite::translations::get_translations;
use crate::model::LexicalItemDetail;
use axum::http::StatusCode;
use sqlx::SqlitePool;

pub async fn get(
    db_pool: &SqlitePool,
    query: &str,
    lang_from_iso3: &str,
    lang_to_iso3: &str,
) -> Result<Vec<LexicalItemDetail>, (StatusCode, String)> {
    let mut out = Vec::<LexicalItemDetail>::new();
    if let Some(wt) = get_translations(db_pool, query, lang_from_iso3, lang_to_iso3).await? {
        out.push(LexicalItemDetail::WordTranslations(wt));
    }
    if let Some(syn) = get_synonyms(db_pool, query, lang_from_iso3).await? {
        out.push(LexicalItemDetail::Synonyms(syn));
    }
    Ok(out)
}
