use crate::client_config::TranslatorParams;
use async_graphql::SimpleObject;

#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(rename_fields = "camelCase")]
pub struct ClientConfig {
    pub backend_redirection_url: Option<String>,
    pub min_query_length: i32,
    pub max_query_length: i32,
    pub translators_params: Vec<TranslatorParams>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            backend_redirection_url: None,
            min_query_length: 3,
            max_query_length: 50,
            translators_params: vec![TranslatorParams {
                translator_id: "google_translate".to_string(),
                text_length_min: 3,
                text_length_max: 50,
                batch_size_limit: 10,
            }],
        }
    }
}
