use async_graphql::SimpleObject;

#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(rename_fields = "camelCase")]
pub struct ClientConfig {
    pub backend_redirection_url: Option<String>,
    pub min_query_length: usize,
    pub max_query_length: usize,
    pub translate_text_length_max: usize,
    pub translate_text_length_min: usize,
    pub translate_batch_size_limit: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            backend_redirection_url: None,
            min_query_length: 3,
            max_query_length: 50,
            translate_text_length_max: 50,
            translate_text_length_min: 3,
            translate_batch_size_limit: 10,
        }
    }
}
