use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use url::{Host, Url};

use crate::domain::provider::{
    ProviderCapabilities, ProviderKind, ProviderProfile, ProviderProtocol,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderConfigFile {
    providers: Vec<ProviderProfileConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderProfileConfig {
    id: String,
    label: String,
    kind: ProviderKind,
    #[serde(default = "default_protocol")]
    protocol: ProviderProtocol,
    base_url: String,
    default_model: String,
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

        Ok(Self {
            id: config.id,
            label: config.label,
            kind: config.kind,
            protocol: config.protocol,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            default_model: config.default_model,
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

    pub async fn ensure_and_load(&self) -> Result<Vec<ProviderProfile>, ProviderConfigError> {
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

    pub async fn load(&self) -> Result<Vec<ProviderProfile>, ProviderConfigError> {
        let source = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|error| ProviderConfigError::Read(error.to_string()))?;
        Self::parse(&source)
    }

    fn parse(source: &str) -> Result<Vec<ProviderProfile>, ProviderConfigError> {
        let file: ProviderConfigFile = toml::from_str(source)
            .map_err(|error| ProviderConfigError::Invalid(error.to_string()))?;
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
        Ok(profiles)
    }
}

fn default_protocol() -> ProviderProtocol {
    ProviderProtocol::Responses
}

fn default_true() -> bool {
    true
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
        assert_eq!(profiles.len(), 2);
        assert!(profiles[0].store_responses);
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

    #[tokio::test]
    async fn creates_a_local_configuration_file_when_missing() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("nested/providers.toml");
        let loader = ProviderConfigLoader::new(path.clone());

        let profiles = loader
            .ensure_and_load()
            .await
            .expect("configuration should be created");

        assert_eq!(profiles.len(), 2);
        assert!(path.is_file());
    }
}
