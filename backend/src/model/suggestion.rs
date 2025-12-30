use crate::model::Sentence;
use async_graphql::SimpleObject;

#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(rename_fields = "camelCase")]
pub struct Suggestion {
    pub text: String,
    pub lang_iso3: String,
    pub source: String,
    pub translations: Vec<Sentence>,
}
