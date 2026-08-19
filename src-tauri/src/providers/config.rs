use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::domain::provider::{
    ProviderCapabilities, ProviderCatalog, ProviderKind, ProviderProfile, ProviderProtocol,
};

const DEFAULT_CONFIG: &str = include_str!("../../../config/providers.example.toml");

#[derive(Debug, thiserror::Error)]
pub enum ProviderConfigError {
    #[error("provider configuration could not be read: {0}")]
    Read(String),
    #[error("provider configuration could not be written: {0}")]
    Write(String),
    #[error("provider configuration is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProviderConfigFile {
    #[serde(default = "legacy_version")]
    version: u32,
    #[serde(default)]
    default_provider_id: Option<String>,
    providers: Vec<ProviderProfileConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProviderProfileConfig {
    id: String,
    label: String,
    kind: ProviderKind,
    #[serde(default = "default_protocol")]
    protocol: ProviderProtocol,
    base_url: String,
    default_model: String,
    #[serde(default)]
    available_models: Vec<String>,
    #[serde(default)]
    enabled_models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    models_synced_at_ms: Option<i64>,
    credential_ref: String,
    #[serde(default = "default_true")]
    store_responses: bool,
    #[serde(default = "default_true")]
    supports_tools: bool,
    #[serde(default = "default_true")]
    supports_images: bool,
    #[serde(default)]
    supports_files: bool,
}

impl TryFrom<ProviderProfileConfig> for ProviderProfile {
    type Error = ProviderConfigError;

    fn try_from(config: ProviderProfileConfig) -> Result<Self, Self::Error> {
        validate_id(&config.id)?;
        validate_not_blank("label", &config.label)?;
        validate_not_blank("default_model", &config.default_model)?;
        validate_not_blank("credential_ref", &config.credential_ref)?;
        validate_base_url(&config.base_url)?;

        if config.kind == ProviderKind::OpenaiResponses
            && config.protocol != ProviderProtocol::Responses
        {
            return Err(ProviderConfigError::Invalid(format!(
                "provider '{}' uses openai_responses kind with a non-Responses protocol",
                config.id
            )));
        }

        let default_model = config.default_model.trim().to_owned();
        let mut available_models = normalize_models(config.available_models)?;
        let mut enabled_models = normalize_models(config.enabled_models)?;
        if available_models.is_empty() {
            available_models.push(default_model.clone());
        }
        if enabled_models.is_empty() {
            enabled_models.push(default_model.clone());
        }
        if !enabled_models.contains(&default_model) {
            enabled_models.push(default_model.clone());
        }

        Ok(Self {
            id: config.id,
            label: config.label.trim().to_owned(),
            kind: config.kind,
            protocol: config.protocol,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            default_model,
            available_models,
            enabled_models,
            models_synced_at_ms: config.models_synced_at_ms,
            credential_ref: config.credential_ref,
            store_responses: config.store_responses,
            capabilities: ProviderCapabilities {
                tools: config.supports_tools,
                images: config.supports_images,
                files: config.supports_files,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfigLoader {
    path: PathBuf,
}

impl ProviderConfigLoader {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn ensure_and_load(&self) -> Result<ProviderCatalog, ProviderConfigError> {
        if tokio::fs::try_exists(&self.path)
            .await
            .map_err(|error| ProviderConfigError::Read(error.to_string()))?
        {
            return self.load().await;
        }

        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| ProviderConfigError::Write(error.to_string()))?;
        }
        tokio::fs::write(&self.path, DEFAULT_CONFIG)
            .await
            .map_err(|error| ProviderConfigError::Write(error.to_string()))?;
        self.load().await
    }

    pub async fn load(&self) -> Result<ProviderCatalog, ProviderConfigError> {
        let source = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|error| ProviderConfigError::Read(error.to_string()))?;
        Self::parse(&source)
    }

    pub async fn save(
        &self,
        catalog: &ProviderCatalog,
    ) -> Result<ProviderCatalog, ProviderConfigError> {
        let file = ProviderConfigFile::from(catalog);
        let source = toml::to_string_pretty(&file)
            .map_err(|error| ProviderConfigError::Write(error.to_string()))?;
        let normalized = Self::parse(&source)?;
        let temporary = self.path.with_extension("toml.tmp");
        tokio::fs::write(&temporary, source)
            .await
            .map_err(|error| ProviderConfigError::Write(error.to_string()))?;
        tokio::fs::rename(&temporary, &self.path)
            .await
            .map_err(|error| ProviderConfigError::Write(error.to_string()))?;
        Ok(normalized)
    }

    fn parse(source: &str) -> Result<ProviderCatalog, ProviderConfigError> {
        let file: ProviderConfigFile = toml::from_str(source)
            .map_err(|error| ProviderConfigError::Invalid(error.to_string()))?;
        if file.version > current_version() {
            return Err(ProviderConfigError::Invalid(format!(
                "provider configuration version {} is newer than supported version {}",
                file.version,
                current_version()
            )));
        }
        if file.providers.is_empty() {
            return Err(ProviderConfigError::Invalid(
                "at least one provider is required".to_owned(),
            ));
        }

        let mut ids = HashSet::with_capacity(file.providers.len());
        let mut profiles = Vec::with_capacity(file.providers.len());
        for config in file.providers {
            let profile = ProviderProfile::try_from(config)?;
            if !ids.insert(profile.id.clone()) {
                return Err(ProviderConfigError::Invalid(format!(
                    "provider id '{}' is duplicated",
                    profile.id
                )));
            }
            profiles.push(profile);
        }
        let default_provider_id = file
            .default_provider_id
            .unwrap_or_else(|| profiles[0].id.clone());
        if !profiles
            .iter()
            .any(|profile| profile.id == default_provider_id)
        {
            return Err(ProviderConfigError::Invalid(format!(
                "default provider '{default_provider_id}' does not exist"
            )));
        }
        Ok(ProviderCatalog {
            default_provider_id,
            profiles,
        })
    }
}

impl From<&ProviderCatalog> for ProviderConfigFile {
    fn from(catalog: &ProviderCatalog) -> Self {
        Self {
            version: current_version(),
            default_provider_id: Some(catalog.default_provider_id.clone()),
            providers: catalog
                .profiles
                .iter()
                .map(ProviderProfileConfig::from)
                .collect(),
        }
    }
}

impl From<&ProviderProfile> for ProviderProfileConfig {
    fn from(profile: &ProviderProfile) -> Self {
        Self {
            id: profile.id.clone(),
            label: profile.label.clone(),
            kind: profile.kind,
            protocol: profile.protocol,
            base_url: profile.base_url.clone(),
            default_model: profile.default_model.clone(),
            available_models: profile.available_models.clone(),
            enabled_models: profile.enabled_models.clone(),
            models_synced_at_ms: profile.models_synced_at_ms,
            credential_ref: profile.credential_ref.clone(),
            store_responses: profile.store_responses,
            supports_tools: profile.capabilities.tools,
            supports_images: profile.capabilities.images,
            supports_files: profile.capabilities.files,
        }
    }
}

fn current_version() -> u32 {
    2
}

fn legacy_version() -> u32 {
    1
}

fn default_protocol() -> ProviderProtocol {
    ProviderProtocol::Responses
}

fn default_true() -> bool {
    true
}

fn normalize_models(models: Vec<String>) -> Result<Vec<String>, ProviderConfigError> {
    let mut normalized = Vec::new();
    for model in models {
        let model = model.trim().to_owned();
        validate_not_blank("model id", &model)?;
        if model.chars().count() > 200 {
            return Err(ProviderConfigError::Invalid(
                "provider model id cannot be longer than 200 characters".to_owned(),
            ));
        }
        if !normalized.contains(&model) {
            normalized.push(model);
        }
    }
    Ok(normalized)
}

fn validate_id(id: &str) -> Result<(), ProviderConfigError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
    {
        return Err(ProviderConfigError::Invalid(format!(
            "provider id '{id}' must use lowercase ASCII letters, digits, '-' or '_'"
        )));
    }
    Ok(())
}

fn validate_not_blank(field: &str, value: &str) -> Result<(), ProviderConfigError> {
    if value.trim().is_empty() {
        return Err(ProviderConfigError::Invalid(format!(
            "provider {field} cannot be blank"
        )));
    }
    Ok(())
}

fn validate_base_url(value: &str) -> Result<(), ProviderConfigError> {
    let url = Url::parse(value)
        .map_err(|error| ProviderConfigError::Invalid(format!("invalid base_url: {error}")))?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ProviderConfigError::Invalid(
            "base_url cannot include query parameters or fragments".to_owned(),
        ));
    }
    if url.scheme() == "https" {
        return Ok(());
    }
    if url.scheme() == "http"
        && matches!(url.host(), Some(Host::Ipv4(address)) if address.is_loopback())
        || url.scheme() == "http"
            && matches!(url.host(), Some(Host::Ipv6(address)) if address.is_loopback())
        || url.scheme() == "http" && url.host_str() == Some("localhost")
    {
        return Ok(());
    }
    Err(ProviderConfigError::Invalid(
        "base_url must use HTTPS; loopback HTTP is allowed for local providers".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{ProviderConfigError, ProviderConfigLoader};

    #[test]
    fn parses_example_configuration() {
        let profiles = ProviderConfigLoader::parse(super::DEFAULT_CONFIG)
            .expect("example configuration should be valid");
        assert_eq!(profiles.profiles.len(), 2);
        assert!(profiles.profiles[0].store_responses);
        assert_eq!(profiles.default_provider_id, "openai");
    }

    #[test]
    fn rejects_duplicate_ids_and_remote_http() {
        let duplicate = super::DEFAULT_CONFIG.replace("local-compatible", "openai");
        assert!(matches!(
            ProviderConfigLoader::parse(&duplicate),
            Err(ProviderConfigError::Invalid(_))
        ));

        let remote_http = super::DEFAULT_CONFIG
            .replace("http://127.0.0.1:11434/v1", "http://provider.example/v1");
        assert!(matches!(
            ProviderConfigLoader::parse(&remote_http),
            Err(ProviderConfigError::Invalid(_))
        ));

        let unknown_field = super::DEFAULT_CONFIG.replace(
            "store_responses = true",
            "store_responses = true\napi_key = \"must-not-be-accepted\"",
        );
        assert!(matches!(
            ProviderConfigLoader::parse(&unknown_field),
            Err(ProviderConfigError::Invalid(_))
        ));
    }

    #[test]
    fn upgrades_legacy_configuration_in_memory() {
        let legacy = super::DEFAULT_CONFIG
            .lines()
            .filter(|line| {
                !line.starts_with("version =")
                    && !line.starts_with("default_provider_id =")
                    && !line.starts_with("available_models =")
                    && !line.starts_with("enabled_models =")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let catalog = ProviderConfigLoader::parse(&legacy).unwrap();

        assert_eq!(catalog.default_provider_id, "openai");
        assert_eq!(catalog.profiles[0].enabled_models, vec!["gpt-5.6"]);
    }

    #[tokio::test]
    async fn creates_a_local_configuration_file_when_missing() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("nested/providers.toml");
        let loader = ProviderConfigLoader::new(path.clone());

        let profiles = loader
            .ensure_and_load()
            .await
            .expect("configuration should be created");

        assert_eq!(profiles.profiles.len(), 2);
        assert!(path.is_file());
    }

    #[tokio::test]
    async fn saves_default_provider_and_model_selection() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("providers.toml");
        let loader = ProviderConfigLoader::new(path);
        let mut catalog = loader.ensure_and_load().await.unwrap();
        catalog.default_provider_id = "local-compatible".to_owned();
        catalog.profiles[0]
            .available_models
            .push("gpt-secondary".to_owned());
        catalog.profiles[0]
            .enabled_models
            .push("gpt-secondary".to_owned());
        catalog.profiles[0].default_model = "gpt-secondary".to_owned();

        loader.save(&catalog).await.unwrap();
        let reloaded = loader.load().await.unwrap();

        assert_eq!(reloaded.default_provider_id, "local-compatible");
        assert_eq!(reloaded.profiles[0].default_model, "gpt-secondary");
        assert_eq!(reloaded.profiles[0].enabled_models.len(), 2);
    }
}
