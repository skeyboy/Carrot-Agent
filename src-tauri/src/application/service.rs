use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::domain::conversation::{Conversation, ConversationChanges, NewConversation};
use crate::domain::provider::ProviderProfile;
use crate::domain::storage::ConversationStore;
use crate::error::AppError;
use crate::persistence::{Database, SqliteConversationStore};
use crate::providers::ProviderConfigLoader;

pub struct CarrotService {
    conversations: Arc<dyn ConversationStore>,
    provider_loader: ProviderConfigLoader,
    providers: RwLock<Vec<ProviderProfile>>,
}

impl CarrotService {
    pub async fn initialize(
        database_path: PathBuf,
        provider_config_path: PathBuf,
    ) -> Result<Self, AppError> {
        let database = Database::connect(&database_path)
            .await
            .map_err(AppError::from)?;
        let provider_loader = ProviderConfigLoader::new(provider_config_path);
        let providers = provider_loader
            .ensure_and_load()
            .await
            .map_err(AppError::from)?;

        Ok(Self {
            conversations: Arc::new(SqliteConversationStore::new(database)),
            provider_loader,
            providers: RwLock::new(providers),
        })
    }

    pub async fn list_conversations(&self) -> Result<Vec<Conversation>, AppError> {
        self.conversations.list().await.map_err(AppError::from)
    }

    pub async fn get_conversation(&self, id: &str) -> Result<Conversation, AppError> {
        validate_id(id)?;
        self.conversations
            .get(id)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("conversation", id))
    }

    pub async fn create_conversation(
        &self,
        title: String,
        provider_profile_id: Option<String>,
        model: Option<String>,
    ) -> Result<Conversation, AppError> {
        let title = validate_title(title)?;
        let providers = self.providers.read().await;
        let profile = match provider_profile_id {
            Some(id) => providers
                .iter()
                .find(|profile| profile.id == id)
                .ok_or_else(|| AppError::not_found("provider profile", &id))?,
            None => providers.first().ok_or_else(|| AppError::Configuration {
                message: "no provider profiles are configured".to_owned(),
            })?,
        };
        let model = validate_model(model.unwrap_or_else(|| profile.default_model.clone()))?;
        let input = NewConversation {
            title,
            default_provider_profile_id: profile.id.clone(),
            default_model: model,
        };
        drop(providers);

        self.conversations
            .create(input)
            .await
            .map_err(AppError::from)
    }

    pub async fn update_conversation(
        &self,
        id: &str,
        changes: ConversationChanges,
    ) -> Result<Conversation, AppError> {
        validate_id(id)?;
        if changes.expected_version < 1 {
            return Err(AppError::invalid_input("expectedVersion must be positive"));
        }

        let title = changes.title.map(validate_title).transpose()?;
        let model = changes.default_model.map(validate_model).transpose()?;
        if let Some(provider_id) = changes.default_provider_profile_id.as_deref() {
            let providers = self.providers.read().await;
            if !providers.iter().any(|profile| profile.id == provider_id) {
                return Err(AppError::not_found("provider profile", provider_id));
            }
        }

        self.conversations
            .update(
                id,
                ConversationChanges {
                    expected_version: changes.expected_version,
                    title,
                    default_provider_profile_id: changes.default_provider_profile_id,
                    default_model: model,
                },
            )
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("conversation", id))
    }

    pub async fn delete_conversation(
        &self,
        id: &str,
        expected_version: i64,
    ) -> Result<(), AppError> {
        validate_id(id)?;
        if expected_version < 1 {
            return Err(AppError::invalid_input("expectedVersion must be positive"));
        }
        let deleted = self
            .conversations
            .delete(id, expected_version)
            .await
            .map_err(AppError::from)?;
        if deleted {
            Ok(())
        } else {
            Err(AppError::not_found("conversation", id))
        }
    }

    pub async fn provider_profiles(&self) -> Vec<ProviderProfile> {
        self.providers.read().await.clone()
    }

    pub async fn reload_provider_profiles(&self) -> Result<Vec<ProviderProfile>, AppError> {
        let profiles = self.provider_loader.load().await.map_err(AppError::from)?;
        *self.providers.write().await = profiles.clone();
        Ok(profiles)
    }

    pub fn provider_config_path(&self) -> String {
        self.provider_loader.path().display().to_string()
    }
}

fn validate_id(id: &str) -> Result<(), AppError> {
    if id.trim().is_empty() {
        return Err(AppError::invalid_input("id cannot be blank"));
    }
    Ok(())
}

fn validate_title(title: String) -> Result<String, AppError> {
    let title = title.trim().to_owned();
    if title.is_empty() {
        return Err(AppError::invalid_input("title cannot be blank"));
    }
    if title.chars().count() > 200 {
        return Err(AppError::invalid_input(
            "title cannot be longer than 200 characters",
        ));
    }
    Ok(title)
}

fn validate_model(model: String) -> Result<String, AppError> {
    let model = model.trim().to_owned();
    if model.is_empty() {
        return Err(AppError::invalid_input("model cannot be blank"));
    }
    if model.chars().count() > 200 {
        return Err(AppError::invalid_input(
            "model cannot be longer than 200 characters",
        ));
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::CarrotService;

    #[tokio::test(flavor = "multi_thread")]
    async fn initializes_from_local_config_and_manages_conversations() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let service = CarrotService::initialize(
            temp.path().join("carrot.sqlite3"),
            temp.path().join("providers.toml"),
        )
        .await
        .expect("service should initialize");

        assert_eq!(service.provider_profiles().await.len(), 2);
        let conversation = service
            .create_conversation("New chat".to_owned(), None, None)
            .await
            .expect("conversation should be created");
        assert_eq!(conversation.default_provider_profile_id, "openai");
        assert_eq!(service.list_conversations().await.unwrap().len(), 1);
    }
}
