use async_graphql::SimpleObject;

#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(rename_fields = "camelCase")]
pub struct TranslationResult {
    pub translations: Vec<String>,
    pub source: String,
}
