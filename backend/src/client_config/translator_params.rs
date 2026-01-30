use async_graphql::SimpleObject;

#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(rename_fields = "camelCase")]
pub struct TranslatorParams {
    pub translator_id: String,
    pub min_query_length: i32,
    pub max_query_length: i32,
}
