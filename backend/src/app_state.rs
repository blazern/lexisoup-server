use crate::client_config::ClientConfig;
use reqwest::Client;
use sqlx::SqlitePool;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    http_client: Client,
    chatgpt_key: String,
    deepl_key: String,
    deepl_endpoint: String,
    panlex_sqlite_pool: SqlitePool,
    client_config: ClientConfig,
}

impl AppState {
    pub fn new(
        chatgpt_key: String,
        deepl_key: String,
        deepl_endpoint: String,
        panlex_sqlite_pool: SqlitePool,
    ) -> Result<Self, reqwest::Error> {
        let http_client = Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            http_client,
            chatgpt_key,
            deepl_key,
            deepl_endpoint,
            panlex_sqlite_pool,
            client_config: ClientConfig::default(),
        })
    }

    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    pub fn chatgpt_key(&self) -> &str {
        &self.chatgpt_key
    }

    pub fn deepl_key(&self) -> &str {
        &self.deepl_key
    }

    pub fn deepl_endpoint(&self) -> &str {
        &self.deepl_endpoint
    }

    pub fn panlex_sqlite_pool(&self) -> &SqlitePool {
        &self.panlex_sqlite_pool
    }

    pub fn client_config(&self) -> &ClientConfig {
        &self.client_config
    }
}
