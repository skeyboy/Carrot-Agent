use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use image::{AnimationDecoder, GenericImageView, ImageFormat};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::agent::cancellation::CancellationTree;
use crate::agent::runtime::{AgentRuntime, RuntimeInput};
use crate::credentials::{CredentialStore, SystemCredentialStore};
use crate::domain::attachment::AttachmentDescriptor;
use crate::domain::conversation::{Conversation, ConversationChanges, NewConversation};
use crate::domain::provider::{
    NewProviderProfile, ProviderCatalog, ProviderProfile, ProviderProfileChanges,
};
use crate::domain::run::{PendingInput, PendingInputIntent, RecoveryResolution};
use crate::domain::settings::AppSettings;
use crate::domain::storage::{ConversationStore, RunStore};
use crate::error::AppError;
use crate::mcp::{McpClientManager, McpConfigStore};
use crate::persistence::{
    Database, SqliteAttachmentStore, SqliteConversationStore, SqliteRunStore,
};
use crate::providers::runtime::{ImageDetail, MessageContent, MessageRole, ProviderMessage};
use crate::providers::{LlmProvider, OpenAiChatProvider, OpenAiResponsesProvider, ProviderEvent};
use crate::providers::{OpenAiModelCatalog, ProviderConfigLoader};
use crate::settings::SettingsStore;

pub struct CarrotService {
    conversations: Arc<dyn ConversationStore>,
    runs: Arc<dyn RunStore>,
    attachments: SqliteAttachmentStore,
    credentials: Arc<dyn CredentialStore>,
    provider_loader: ProviderConfigLoader,
    providers: RwLock<ProviderCatalog>,
    settings: SettingsStore,
    mcp: Arc<McpClientManager>,
    database_path: PathBuf,
    attachment_path: PathBuf,
    cancellation: CancellationTree,
    runtime_instance_id: String,
}

impl CarrotService {
    pub async fn initialize(
        database_path: PathBuf,
        provider_config_path: PathBuf,
        settings_path: PathBuf,
        attachment_path: PathBuf,
    ) -> Result<Self, AppError> {
        let mcp_config_path = settings_path.with_file_name("mcp-servers.toml");
        let database = Database::connect(&database_path)
            .await
            .map_err(AppError::from)?;
        let provider_loader = ProviderConfigLoader::new(provider_config_path);
        let providers = provider_loader
            .ensure_and_load()
            .await
            .map_err(AppError::from)?;
        let settings = SettingsStore::load(settings_path).await?;
        let runtime_instance_id = uuid::Uuid::now_v7().to_string();
        let run_store = SqliteRunStore::new(database.clone());
        run_store
            .recover_expired_leases(&runtime_instance_id)
            .await
            .map_err(AppError::from)?;
        let runs: Arc<dyn RunStore> = Arc::new(run_store);
        let credentials: Arc<dyn CredentialStore> = Arc::new(SystemCredentialStore);
        let mcp =
            McpClientManager::initialize(McpConfigStore::new(mcp_config_path), credentials.clone())
                .await?;

        Ok(Self {
            conversations: Arc::new(SqliteConversationStore::new(database.clone())),
            runs,
            attachments: SqliteAttachmentStore::new(database),
            credentials,
            provider_loader,
            providers: RwLock::new(providers),
            settings,
            mcp,
            database_path,
            attachment_path,
            cancellation: CancellationTree::default(),
            runtime_instance_id,
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
                .profiles
                .iter()
                .find(|profile| profile.id == id)
                .ok_or_else(|| AppError::not_found("provider profile", &id))?,
            None => providers
                .profiles
                .iter()
                .find(|profile| profile.id == providers.default_provider_id)
                .ok_or_else(|| AppError::Configuration {
                    message: "the default provider profile is unavailable".to_owned(),
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
            if !providers
                .profiles
                .iter()
                .any(|profile| profile.id == provider_id)
            {
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
        self.providers.read().await.profiles.clone()
    }

    pub async fn provider_catalog(&self) -> ProviderCatalog {
        self.providers.read().await.clone()
    }

    pub async fn reload_provider_profiles(&self) -> Result<ProviderCatalog, AppError> {
        let catalog = self.provider_loader.load().await.map_err(AppError::from)?;
        *self.providers.write().await = catalog.clone();
        Ok(catalog)
    }

    pub async fn create_provider_profile(
        &self,
        input: NewProviderProfile,
    ) -> Result<ProviderCatalog, AppError> {
        let mut guard = self.providers.write().await;
        if guard.profiles.iter().any(|profile| profile.id == input.id) {
            return Err(AppError::invalid_input(format!(
                "provider profile '{}' already exists",
                input.id
            )));
        }
        guard.profiles.push(ProviderProfile {
            credential_ref: format!("{}-api-key", input.id),
            id: input.id,
            label: input.label,
            kind: input.kind,
            protocol: input.protocol,
            base_url: input.base_url,
            available_models: vec![input.default_model.clone()],
            enabled_models: vec![input.default_model.clone()],
            default_model: input.default_model,
            models_synced_at_ms: None,
            store_responses: input.store_responses,
            capabilities: input.capabilities,
        });
        let saved = self.provider_loader.save(&guard).await?;
        *guard = saved.clone();
        Ok(saved)
    }

    pub async fn update_provider_profile(
        &self,
        changes: ProviderProfileChanges,
    ) -> Result<ProviderCatalog, AppError> {
        if changes.enabled_models.is_empty() {
            return Err(AppError::invalid_input(
                "at least one model must be enabled",
            ));
        }
        if !changes.enabled_models.contains(&changes.default_model) {
            return Err(AppError::invalid_input("the default model must be enabled"));
        }
        let mut guard = self.providers.write().await;
        let profile = guard
            .profiles
            .iter_mut()
            .find(|profile| profile.id == changes.id)
            .ok_or_else(|| AppError::not_found("provider profile", &changes.id))?;
        profile.label = changes.label;
        profile.base_url = changes.base_url;
        profile.default_model = changes.default_model;
        profile.enabled_models = changes.enabled_models;
        profile.store_responses = changes.store_responses;
        profile.capabilities = changes.capabilities;
        let saved = self.provider_loader.save(&guard).await?;
        *guard = saved.clone();
        Ok(saved)
    }

    pub async fn delete_provider_profile(&self, id: &str) -> Result<ProviderCatalog, AppError> {
        let mut guard = self.providers.write().await;
        if guard.profiles.len() == 1 {
            return Err(AppError::invalid_input("at least one provider is required"));
        }
        if !guard.profiles.iter().any(|profile| profile.id == id) {
            return Err(AppError::not_found("provider profile", id));
        }
        if self
            .conversations
            .list()
            .await?
            .iter()
            .any(|conversation| conversation.default_provider_profile_id == id)
        {
            return Err(AppError::invalid_input(
                "provider is used by an existing conversation",
            ));
        }
        guard.profiles.retain(|profile| profile.id != id);
        if guard.default_provider_id == id {
            guard.default_provider_id = guard.profiles[0].id.clone();
        }
        let saved = self.provider_loader.save(&guard).await?;
        *guard = saved.clone();
        Ok(saved)
    }

    pub async fn set_default_provider(&self, id: &str) -> Result<ProviderCatalog, AppError> {
        let mut guard = self.providers.write().await;
        if !guard.profiles.iter().any(|profile| profile.id == id) {
            return Err(AppError::not_found("provider profile", id));
        }
        guard.default_provider_id = id.to_owned();
        let saved = self.provider_loader.save(&guard).await?;
        *guard = saved.clone();
        Ok(saved)
    }

    pub async fn sync_provider_models(&self, id: &str) -> Result<ProviderCatalog, AppError> {
        let profile = self
            .providers
            .read()
            .await
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("provider profile", id))?;
        let api_key = match self.credential(id).await {
            Ok(secret) => secret,
            Err(_) if is_loopback_base_url(&profile.base_url) => "local-provider".to_owned(),
            Err(error) => return Err(error),
        };
        let timeout = std::time::Duration::from_secs(u64::from(
            self.settings().await.request_timeout_seconds,
        ));
        let models = tokio::time::timeout(
            timeout,
            OpenAiModelCatalog::new(api_key, profile.base_url).list(),
        )
        .await
        .map_err(|_| AppError::Configuration {
            message: "model synchronization timed out".to_owned(),
        })??;
        if models.is_empty() {
            return Err(AppError::Configuration {
                message: "provider returned an empty model catalog".to_owned(),
            });
        }

        let mut guard = self.providers.write().await;
        let profile = guard
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or_else(|| AppError::not_found("provider profile", id))?;
        profile.available_models = models;
        profile.models_synced_at_ms = Some(crate::persistence::now_ms()?);
        let saved = self.provider_loader.save(&guard).await?;
        *guard = saved.clone();
        Ok(saved)
    }

    pub fn provider_config_path(&self) -> String {
        self.provider_loader.path().display().to_string()
    }

    pub async fn settings(&self) -> AppSettings {
        self.settings.get().await
    }

    pub async fn update_settings(&self, settings: AppSettings) -> Result<AppSettings, AppError> {
        self.settings.update(settings).await.map_err(AppError::from)
    }

    pub async fn mcp_snapshot(&self) -> crate::domain::mcp::McpCatalogSnapshot {
        self.mcp.snapshot().await
    }

    pub async fn update_mcp_system_settings(
        &self,
        settings: crate::domain::mcp::McpSystemSettings,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp
            .update_system_settings(settings)
            .await
            .map_err(AppError::from)
    }

    pub async fn create_mcp_server(
        &self,
        config: crate::domain::mcp::McpServerConfig,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.create_server(config).await.map_err(AppError::from)
    }

    pub async fn install_mcp_preset(
        &self,
        preset: crate::domain::mcp::McpPresetKind,
        workspace_path: Option<String>,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp
            .install_preset(preset, workspace_path)
            .await
            .map_err(AppError::from)
    }

    pub async fn update_mcp_server(
        &self,
        config: crate::domain::mcp::McpServerConfig,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.update_server(config).await.map_err(AppError::from)
    }

    pub async fn delete_mcp_server(
        &self,
        id: &str,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.delete_server(id).await.map_err(AppError::from)
    }

    pub async fn connect_mcp_server(
        &self,
        id: &str,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.connect(id).await?;
        Ok(self.mcp.snapshot().await)
    }

    pub async fn disconnect_mcp_server(
        &self,
        id: &str,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.disconnect(id).await?;
        Ok(self.mcp.snapshot().await)
    }

    pub async fn set_mcp_tool_policy(
        &self,
        server_id: &str,
        policy: crate::domain::mcp::McpToolPolicy,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp
            .set_tool_policy(server_id, policy)
            .await
            .map_err(AppError::from)
    }

    pub async fn set_mcp_auth(
        &self,
        server_id: &str,
        secret: String,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.set_auth_secret(server_id, secret).await?;
        Ok(self.mcp.snapshot().await)
    }

    pub async fn clear_mcp_auth(
        &self,
        server_id: &str,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.clear_auth(server_id).await?;
        Ok(self.mcp.snapshot().await)
    }

    pub async fn refresh_mcp_server(
        &self,
        server_id: &str,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp.refresh_server_tools(server_id).await?;
        Ok(self.mcp.snapshot().await)
    }

    pub async fn begin_mcp_oauth(
        &self,
        server_id: &str,
        redirect_uri: String,
    ) -> Result<crate::domain::mcp::McpOAuthStart, AppError> {
        self.mcp
            .begin_oauth(server_id, redirect_uri)
            .await
            .map_err(AppError::from)
    }

    pub async fn complete_mcp_oauth(
        &self,
        server_id: &str,
        callback_url: String,
    ) -> Result<crate::domain::mcp::McpCatalogSnapshot, AppError> {
        self.mcp
            .complete_oauth(server_id, callback_url)
            .await
            .map_err(AppError::from)
    }

    pub fn settings_path(&self) -> String {
        self.settings.path().display().to_string()
    }

    pub fn database_path(&self) -> String {
        self.database_path.display().to_string()
    }

    pub fn attachment_path(&self) -> String {
        self.attachment_path.display().to_string()
    }

    pub async fn list_attachments(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AttachmentDescriptor>, AppError> {
        self.get_conversation(conversation_id).await?;
        self.attachments
            .list(conversation_id)
            .await
            .map_err(AppError::from)
    }

    pub async fn import_attachment(
        &self,
        conversation_id: &str,
        source_path: PathBuf,
    ) -> Result<AttachmentDescriptor, AppError> {
        self.get_conversation(conversation_id).await?;
        let max_bytes = u64::from(self.settings().await.attachment_max_megabytes) * 1024 * 1024;
        let metadata = tokio::fs::metadata(&source_path).await.map_err(|error| {
            AppError::invalid_input(format!("image could not be read: {error}"))
        })?;
        if metadata.len() == 0 || metadata.len() > max_bytes {
            return Err(AppError::invalid_input(format!(
                "image must be between 1 byte and {} MB",
                max_bytes / 1024 / 1024
            )));
        }
        let bytes = tokio::fs::read(&source_path).await.map_err(|error| {
            AppError::invalid_input(format!("image could not be read: {error}"))
        })?;
        let encoded = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AppError> {
            let format = image::guess_format(&bytes)
                .map_err(|error| AppError::invalid_input(format!("unsupported image: {error}")))?;
            if format == ImageFormat::Gif {
                let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes))
                    .map_err(|error| {
                        AppError::invalid_input(format!("unsupported GIF: {error}"))
                    })?;
                if decoder.into_frames().take(2).count() > 1 {
                    return Err(AppError::invalid_input("animated GIF is not supported"));
                }
            }
            let image = image::load_from_memory(&bytes)
                .map_err(|error| AppError::invalid_input(format!("unsupported image: {error}")))?;
            let (width, height) = image.dimensions();
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 40_000_000 {
                return Err(AppError::invalid_input(
                    "image dimensions exceed the 40 megapixel limit",
                ));
            }
            let mut cursor = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut cursor, ImageFormat::Png)
                .map_err(|error| AppError::internal(format!("image encoding failed: {error}")))?;
            let encoded = cursor.into_inner();
            if encoded.len() as u64 > max_bytes {
                return Err(AppError::invalid_input(
                    "normalized image exceeds the configured attachment limit",
                ));
            }
            Ok(encoded)
        })
        .await
        .map_err(|error| AppError::internal(error.to_string()))??;

        let id = uuid::Uuid::now_v7().to_string();
        let relative_path = format!("{id}.png");
        let destination = self.attachment_path.join(&relative_path);
        tokio::fs::create_dir_all(&self.attachment_path)
            .await
            .map_err(|error| AppError::Storage {
                message: error.to_string(),
            })?;
        let temporary = self.attachment_path.join(format!("{id}.tmp"));
        tokio::fs::write(&temporary, &encoded)
            .await
            .map_err(|error| AppError::Storage {
                message: error.to_string(),
            })?;
        tokio::fs::rename(&temporary, &destination)
            .await
            .map_err(|error| AppError::Storage {
                message: error.to_string(),
            })?;

        let source_name = source_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("image")
            .chars()
            .take(120)
            .collect::<String>();
        let descriptor = AttachmentDescriptor {
            id,
            conversation_id: conversation_id.to_owned(),
            media_type: "image/png".to_owned(),
            file_name: format!("{source_name}.png"),
            byte_length: encoded.len() as u64,
            content_hash: format!("{:x}", Sha256::digest(&encoded)),
            relative_path,
            created_at_ms: crate::persistence::now_ms()?,
        };
        if let Err(error) = self.attachments.insert(&descriptor).await {
            let _ = tokio::fs::remove_file(destination).await;
            return Err(AppError::from(error));
        }
        Ok(descriptor)
    }

    pub async fn delete_attachment(&self, id: &str) -> Result<(), AppError> {
        validate_id(id)?;
        let Some(relative_path) = self.attachments.delete(id).await? else {
            return Err(AppError::not_found("attachment", id));
        };
        let path = self.attachment_path.join(relative_path);
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::Storage {
                message: error.to_string(),
            }),
        }
    }

    pub async fn stream_chat(
        &self,
        run_id: String,
        conversation_id: String,
        text: String,
        attachment_ids: Vec<String>,
        replaces_run_id: Option<String>,
        events: tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<(), AppError> {
        let text = text.trim().to_owned();
        if text.is_empty() && attachment_ids.is_empty() {
            return Err(AppError::invalid_input("message or attachment is required"));
        }
        let conversation = self.get_conversation(&conversation_id).await?;
        let profile = self
            .providers
            .read()
            .await
            .profiles
            .iter()
            .find(|profile| profile.id == conversation.default_provider_profile_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "provider profile",
                    &conversation.default_provider_profile_id,
                )
            })?;
        if !attachment_ids.is_empty() && !profile.capabilities.images {
            return Err(AppError::invalid_input(
                "selected provider does not support images",
            ));
        }

        let user_message = self
            .build_user_message(&conversation_id, text, attachment_ids)
            .await?;
        let provider = self.provider_for_profile(&profile).await?;
        let cancellation = self.cancellation.begin_run(run_id.clone()).await;
        let (lease_stop, lease_task) = start_lease_heartbeat(
            self.runs.clone(),
            run_id.clone(),
            self.runtime_instance_id.clone(),
        );
        let settings = self.settings().await;
        let runtime = AgentRuntime::new(self.runs.clone(), self.mcp.tool_registry().await);
        let input = RuntimeInput {
            run_id: run_id.clone(),
            conversation_id,
            strategy: settings.default_strategy,
            provider_profile: profile,
            model: conversation.default_model,
            runtime_instance_id: self.runtime_instance_id.clone(),
            replaces_run_id,
            parent_run_id: None,
            source_pending_input_id: None,
            user_message,
            max_model_steps: settings.max_model_steps,
            request_timeout: std::time::Duration::from_secs(u64::from(
                settings.request_timeout_seconds,
            )),
        };
        let result = runtime
            .run(provider, input, events, cancellation.clone())
            .await;
        lease_stop.cancel();
        let _ = lease_task.await;
        self.cancellation.finish_run(&run_id).await;
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(AppError::Internal {
                message: error.to_string(),
            }),
        }
    }

    pub async fn chat_snapshot(
        &self,
        conversation_id: &str,
    ) -> Result<crate::domain::run::ChatSnapshot, AppError> {
        self.get_conversation(conversation_id).await?;
        self.runs
            .recover_expired_leases(&self.runtime_instance_id)
            .await?;
        self.runs
            .snapshot(conversation_id)
            .await
            .map_err(AppError::from)
    }

    pub async fn cancel_chat(&self, run_id: &str) -> Result<(), AppError> {
        if self.cancellation.cancel_run(run_id).await {
            return Ok(());
        }
        let run = self
            .runs
            .get_run(run_id)
            .await?
            .ok_or_else(|| AppError::not_found("run", run_id))?;
        if !matches!(
            run.status,
            crate::domain::run::RunStatus::Paused
                | crate::domain::run::RunStatus::Suspended
                | crate::domain::run::RunStatus::Interrupted
                | crate::domain::run::RunStatus::RecoveryRequired
        ) {
            return Err(AppError::not_found("active run", run_id));
        }
        self.runs
            .transition(
                run_id,
                crate::domain::run::RunTransition {
                    status: crate::domain::run::RunStatus::Cancelled,
                    phase: crate::domain::run::RunPhase::None,
                    event_kind: "run_recovery_abandoned".to_owned(),
                    payload: serde_json::json!({"fromStatus": run.status}),
                    stop_reason: Some("recovery abandoned by user".to_owned()),
                },
            )
            .await?;
        Ok(())
    }

    pub async fn prepare_for_suspend(&self) -> Result<(), AppError> {
        let run_ids = self.cancellation.active_run_ids().await;
        for run_id in &run_ids {
            self.runs.request_pause(run_id).await?;
        }
        for run_id in run_ids {
            self.cancellation.pause_run(&run_id).await;
        }
        self.mcp.disconnect_all().await;
        Ok(())
    }

    pub async fn resume_from_suspend(&self) {
        self.mcp.reconnect_enabled().await;
    }

    pub async fn pause_chat(&self, run_id: &str) -> Result<(), AppError> {
        self.runs.request_pause(run_id).await?;
        if self.cancellation.pause_run(run_id).await {
            Ok(())
        } else {
            Err(AppError::not_found("active run", run_id))
        }
    }

    pub async fn enqueue_chat_input(
        &self,
        run_id: &str,
        intent: PendingInputIntent,
        text: String,
        attachment_ids: Vec<String>,
    ) -> Result<PendingInput, AppError> {
        let text = text.trim().to_owned();
        if text.is_empty() && attachment_ids.is_empty() {
            return Err(AppError::invalid_input("pending input cannot be empty"));
        }
        let run = self
            .runs
            .get_run(run_id)
            .await?
            .ok_or_else(|| AppError::not_found("run", run_id))?;
        let message = self
            .build_user_message(&run.conversation_id, text, attachment_ids)
            .await?;
        self.runs
            .enqueue_input(
                run_id,
                intent,
                serde_json::to_value(message)
                    .map_err(|error| AppError::internal(error.to_string()))?,
            )
            .await
            .map_err(AppError::from)
    }

    pub async fn branch_chat(
        &self,
        run_id: String,
        pending_input_id: &str,
        conversation_id: &str,
        events: tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<String, AppError> {
        let pending = self
            .runs
            .get_pending_input(pending_input_id)
            .await?
            .ok_or_else(|| AppError::not_found("pending input", pending_input_id))?;
        if pending.status != "pending" || pending.intent == PendingInputIntent::Append {
            return Err(AppError::invalid_input(
                "pending input is not available for branching",
            ));
        }
        let parent = self
            .runs
            .get_run(&pending.run_id)
            .await?
            .ok_or_else(|| AppError::not_found("run", &pending.run_id))?;
        if parent.conversation_id != conversation_id {
            return Err(AppError::invalid_input(
                "pending input does not belong to the requested conversation",
            ));
        }
        let message: ProviderMessage = serde_json::from_value(pending.content.clone())
            .map_err(|error| AppError::invalid_input(error.to_string()))?;
        let profile = parent.provider_snapshot.clone();
        let provider = self.provider_for_profile(&profile).await?;
        let cancellation = self.cancellation.begin_run(run_id.clone()).await;
        let (lease_stop, lease_task) = start_lease_heartbeat(
            self.runs.clone(),
            run_id.clone(),
            self.runtime_instance_id.clone(),
        );
        let settings = self.settings().await;
        let runtime = AgentRuntime::new(self.runs.clone(), self.mcp.tool_registry().await);
        let input = RuntimeInput {
            run_id: run_id.clone(),
            conversation_id: conversation_id.to_owned(),
            strategy: parent.strategy,
            provider_profile: profile,
            model: parent.model,
            runtime_instance_id: self.runtime_instance_id.clone(),
            replaces_run_id: (pending.intent == PendingInputIntent::CancelAndReplace)
                .then(|| parent.id.clone()),
            parent_run_id: (pending.intent == PendingInputIntent::Fork).then(|| parent.id.clone()),
            source_pending_input_id: Some(pending.id),
            user_message: message,
            max_model_steps: settings.max_model_steps,
            request_timeout: std::time::Duration::from_secs(u64::from(
                settings.request_timeout_seconds,
            )),
        };
        let result = runtime
            .run(provider, input, events, cancellation.clone())
            .await;
        lease_stop.cancel();
        let _ = lease_task.await;
        self.cancellation.finish_run(&run_id).await;
        result.map_err(|error| AppError::internal(error.to_string()))?;
        Ok(conversation_id.to_owned())
    }

    pub async fn resolve_tool_approval(
        &self,
        run_id: &str,
        execution_id: &str,
        approved: bool,
    ) -> Result<(), AppError> {
        self.runs
            .resolve_tool_approval(run_id, execution_id, approved)
            .await?;
        Ok(())
    }

    pub async fn resolve_tool_recovery(
        &self,
        run_id: &str,
        execution_id: &str,
        resolution: RecoveryResolution,
        note: Option<String>,
    ) -> Result<(), AppError> {
        self.runs
            .resolve_recovery(run_id, execution_id, resolution, note)
            .await?;
        Ok(())
    }

    pub async fn resume_chat(
        &self,
        run_id: String,
        conversation_id: &str,
        events: tokio::sync::mpsc::Sender<ProviderEvent>,
    ) -> Result<String, AppError> {
        let existing = self
            .runs
            .get_run(&run_id)
            .await?
            .ok_or_else(|| AppError::not_found("run", &run_id))?;
        if existing.conversation_id != conversation_id {
            return Err(AppError::invalid_input(
                "run does not belong to the requested conversation",
            ));
        }
        let profile = existing.provider_snapshot.clone();
        let provider = self.provider_for_profile(&profile).await?;
        let claimed = self
            .runs
            .claim_resume(&run_id, &self.runtime_instance_id)
            .await?;
        let cancellation = self.cancellation.begin_run(run_id.clone()).await;
        let (lease_stop, lease_task) = start_lease_heartbeat(
            self.runs.clone(),
            run_id.clone(),
            self.runtime_instance_id.clone(),
        );
        let settings = self.settings().await;
        let current_registry = self.mcp.tool_registry().await;
        let registry = if claimed.tool_catalog_snapshot.is_empty() {
            current_registry
        } else {
            current_registry.retain_snapshot(&claimed.tool_catalog_snapshot)
        };
        let runtime = AgentRuntime::new(self.runs.clone(), registry);
        let input = RuntimeInput {
            run_id: run_id.clone(),
            conversation_id: claimed.conversation_id.clone(),
            strategy: claimed.strategy,
            provider_profile: profile,
            model: claimed.model,
            runtime_instance_id: self.runtime_instance_id.clone(),
            replaces_run_id: None,
            parent_run_id: None,
            source_pending_input_id: None,
            user_message: ProviderMessage {
                role: MessageRole::User,
                content: Vec::new(),
            },
            max_model_steps: settings.max_model_steps,
            request_timeout: std::time::Duration::from_secs(u64::from(
                settings.request_timeout_seconds,
            )),
        };
        let result = runtime
            .resume(provider, input, events, cancellation.clone())
            .await;
        lease_stop.cancel();
        let _ = lease_task.await;
        self.cancellation.finish_run(&run_id).await;
        result.map_err(|error| AppError::Internal {
            message: error.to_string(),
        })?;
        Ok(claimed.conversation_id)
    }

    pub async fn credential_configured(&self, provider_id: &str) -> Result<bool, AppError> {
        let reference = self.credential_reference(provider_id).await?;
        self.credentials
            .contains(&reference)
            .await
            .map_err(AppError::from)
    }

    pub async fn credential(&self, provider_id: &str) -> Result<String, AppError> {
        let reference = self.credential_reference(provider_id).await?;
        self.credentials
            .get(&reference)
            .await?
            .ok_or_else(|| AppError::Configuration {
                message: format!("provider '{provider_id}' does not have an API key"),
            })
    }

    pub async fn set_credential(&self, provider_id: &str, secret: String) -> Result<(), AppError> {
        let reference = self.credential_reference(provider_id).await?;
        self.credentials.set(&reference, secret).await?;
        Ok(())
    }

    pub async fn delete_credential(&self, provider_id: &str) -> Result<(), AppError> {
        let reference = self.credential_reference(provider_id).await?;
        self.credentials.delete(&reference).await?;
        Ok(())
    }

    async fn build_user_message(
        &self,
        conversation_id: &str,
        text: String,
        attachment_ids: Vec<String>,
    ) -> Result<ProviderMessage, AppError> {
        let available = self.attachments.list(conversation_id).await?;
        let mut content = Vec::new();
        if !text.is_empty() {
            content.push(MessageContent::Text { text });
        }
        for id in attachment_ids {
            let attachment = available
                .iter()
                .find(|attachment| attachment.id == id)
                .ok_or_else(|| AppError::not_found("attachment", &id))?;
            let bytes = tokio::fs::read(self.attachment_path.join(&attachment.relative_path))
                .await
                .map_err(|error| AppError::Storage {
                    message: error.to_string(),
                })?;
            content.push(MessageContent::ImageDataUrl {
                data_url: format!(
                    "data:{};base64,{}",
                    attachment.media_type,
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ),
                detail: ImageDetail::Auto,
            });
        }
        Ok(ProviderMessage {
            role: MessageRole::User,
            content,
        })
    }

    async fn provider_for_profile(
        &self,
        profile: &ProviderProfile,
    ) -> Result<Arc<dyn LlmProvider>, AppError> {
        let api_key = match self.credential(&profile.id).await {
            Ok(secret) => secret,
            Err(_) if is_loopback_base_url(&profile.base_url) => "local-provider".to_owned(),
            Err(error) => return Err(error),
        };
        Ok(match profile.protocol {
            crate::domain::provider::ProviderProtocol::Responses => Arc::new(
                OpenAiResponsesProvider::new(api_key, profile.base_url.clone()),
            ),
            crate::domain::provider::ProviderProtocol::ChatCompletions => {
                Arc::new(OpenAiChatProvider::new(api_key, profile.base_url.clone()))
            }
        })
    }

    async fn credential_reference(&self, provider_id: &str) -> Result<String, AppError> {
        self.providers
            .read()
            .await
            .profiles
            .iter()
            .find(|profile| profile.id == provider_id)
            .map(|profile| profile.credential_ref.clone())
            .ok_or_else(|| AppError::not_found("provider profile", provider_id))
    }
}

fn start_lease_heartbeat(
    store: Arc<dyn RunStore>,
    run_id: String,
    runtime_instance_id: String,
) -> (
    tokio_util::sync::CancellationToken,
    tokio::task::JoinHandle<()>,
) {
    let stop = tokio_util::sync::CancellationToken::new();
    let task_stop = stop.clone();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = task_stop.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                    match store.renew_lease(&run_id, &runtime_instance_id).await {
                        Ok(true) => {}
                        Ok(false) | Err(_) => break,
                    }
                }
            }
        }
    });
    (stop, task)
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

fn is_loopback_base_url(base_url: &str) -> bool {
    url::Url::parse(base_url).is_ok_and(|url| {
        url.host_str() == Some("localhost")
            || url.host().is_some_and(|host| {
                matches!(host, url::Host::Ipv4(ip) if ip.is_loopback())
                    || matches!(host, url::Host::Ipv6(ip) if ip.is_loopback())
            })
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::codecs::gif::GifEncoder;
    use image::{Delay, DynamicImage, Frame, ImageFormat, RgbaImage};

    use super::CarrotService;

    #[tokio::test(flavor = "multi_thread")]
    async fn initializes_from_local_config_and_manages_conversations() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let service = CarrotService::initialize(
            temp.path().join("carrot.sqlite3"),
            temp.path().join("providers.toml"),
            temp.path().join("settings.toml"),
            temp.path().join("attachments"),
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

    #[tokio::test(flavor = "multi_thread")]
    async fn persists_provider_defaults_updates_and_deletion() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let provider_path = temp.path().join("providers.toml");
        let service = CarrotService::initialize(
            temp.path().join("carrot.sqlite3"),
            provider_path,
            temp.path().join("settings.toml"),
            temp.path().join("attachments"),
        )
        .await
        .unwrap();

        service
            .set_default_provider("local-compatible")
            .await
            .unwrap();
        let local = service
            .provider_profiles()
            .await
            .into_iter()
            .find(|profile| profile.id == "local-compatible")
            .unwrap();
        service
            .update_provider_profile(crate::domain::provider::ProviderProfileChanges {
                id: local.id,
                label: "Local renamed".to_owned(),
                base_url: local.base_url,
                default_model: "local-vision".to_owned(),
                enabled_models: vec!["local-model".to_owned(), "local-vision".to_owned()],
                store_responses: local.store_responses,
                capabilities: local.capabilities,
            })
            .await
            .unwrap();
        service.delete_provider_profile("openai").await.unwrap();
        service.reload_provider_profiles().await.unwrap();

        let catalog = service.provider_catalog().await;
        assert_eq!(catalog.default_provider_id, "local-compatible");
        assert_eq!(catalog.profiles.len(), 1);
        assert_eq!(catalog.profiles[0].label, "Local renamed");
        assert_eq!(catalog.profiles[0].default_model, "local-vision");
        let conversation = service
            .create_conversation("Uses default".to_owned(), None, None)
            .await
            .unwrap();
        assert_eq!(conversation.default_provider_profile_id, "local-compatible");
        assert_eq!(conversation.default_model, "local-vision");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn imports_normalizes_lists_and_deletes_an_image() {
        let temp = tempfile::tempdir().unwrap();
        let service = CarrotService::initialize(
            temp.path().join("carrot.sqlite3"),
            temp.path().join("providers.toml"),
            temp.path().join("settings.toml"),
            temp.path().join("attachments"),
        )
        .await
        .unwrap();
        let conversation = service
            .create_conversation("Images".to_owned(), None, None)
            .await
            .unwrap();
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::new(2, 2))
            .write_to(&mut encoded, ImageFormat::Jpeg)
            .unwrap();
        let source = temp.path().join("source.jpg");
        tokio::fs::write(&source, encoded.into_inner())
            .await
            .unwrap();

        let attachment = service
            .import_attachment(&conversation.id, source)
            .await
            .unwrap();
        assert_eq!(attachment.media_type, "image/png");
        assert_eq!(
            service
                .list_attachments(&conversation.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            temp.path()
                .join("attachments")
                .join(&attachment.relative_path)
                .is_file()
        );

        service.delete_attachment(&attachment.id).await.unwrap();
        assert!(
            service
                .list_attachments(&conversation.id)
                .await
                .unwrap()
                .is_empty()
        );

        let mut animation = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut animation);
            for _ in 0..2 {
                encoder
                    .encode_frame(Frame::from_parts(
                        RgbaImage::new(2, 2),
                        0,
                        0,
                        Delay::from_numer_denom_ms(100, 1),
                    ))
                    .unwrap();
            }
        }
        let animated_source = temp.path().join("animated.gif");
        tokio::fs::write(&animated_source, animation).await.unwrap();
        let error = service
            .import_attachment(&conversation.id, animated_source)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("animated GIF"));
    }
}
