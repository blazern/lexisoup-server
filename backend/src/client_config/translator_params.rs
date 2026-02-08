use async_graphql::SimpleObject;

#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(rename_fields = "camelCase")]
pub struct TranslatorParams {
    pub translator_id: String,
    pub text_length_max: i32,
    pub text_length_min: i32,
    pub batch_size_limit: i32,
}
