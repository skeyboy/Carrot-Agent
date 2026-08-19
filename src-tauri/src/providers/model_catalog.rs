use openai_oxide::{ClientConfig, OpenAI};

use crate::error::AppError;

pub struct OpenAiModelCatalog {
    client: OpenAI,
}

impl OpenAiModelCatalog {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: OpenAI::with_config(ClientConfig::new(api_key).base_url(base_url)),
        }
    }

    pub async fn list(&self) -> Result<Vec<String>, AppError> {
        let response =
            self.client
                .models()
                .list()
                .await
                .map_err(|error| AppError::Configuration {
                    message: format!("model synchronization failed: {error}"),
                })?;
        let mut models = response
            .data
            .into_iter()
            .map(|model| model.id)
            .filter(|id| !id.trim().is_empty())
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        Ok(models)
    }
}
