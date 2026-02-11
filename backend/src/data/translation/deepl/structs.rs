use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct DeepLRequest<'a> {
    pub text: &'a [String],
    pub target_lang: &'a str,
    pub source_lang: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct DeepLResponse {
    pub translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
pub struct DeepLTranslation {
    pub text: String,
}
