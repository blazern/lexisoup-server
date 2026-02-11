use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ChatGPTRequest<'a> {
    pub model: &'a str,
    pub input: &'a str,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ChatGPTResponse {
    pub output: Vec<ChatGPTOutput>,
    pub status: String,
    pub error: Option<String>,
    pub model: String,
    pub usage: ChatGPTUsage,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ChatGPTOutput {
    pub content: Vec<ChatGPTContent>,
    pub id: String,
    pub r#type: String,
    pub status: String,
    pub role: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ChatGPTContent {
    pub text: String,
    pub r#type: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ChatGPTUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
}
