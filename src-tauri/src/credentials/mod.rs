//! Credential ports and platform-specific secure storage adapters.

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("secure credential storage is unavailable: {0}")]
    Unavailable(String),
    #[error("secure credential operation failed: {0}")]
    Operation(String),
}

#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn contains(&self, reference: &str) -> Result<bool, CredentialError>;
    async fn get(&self, reference: &str) -> Result<Option<String>, CredentialError>;
    async fn set(&self, reference: &str, secret: String) -> Result<(), CredentialError>;
    async fn delete(&self, reference: &str) -> Result<(), CredentialError>;
}

pub struct SystemCredentialStore;

const SERVICE_NAME: &str = "com.carrot.desktop";

impl SystemCredentialStore {
    fn validate_reference(reference: &str) -> Result<String, CredentialError> {
        let reference = reference.trim();
        if reference.is_empty() || reference.len() > 200 {
            return Err(CredentialError::Operation(
                "credential reference must contain 1 to 200 characters".to_owned(),
            ));
        }
        Ok(reference.to_owned())
    }
}

#[async_trait]
impl CredentialStore for SystemCredentialStore {
    async fn contains(&self, reference: &str) -> Result<bool, CredentialError> {
        Ok(self.get(reference).await?.is_some())
    }

    async fn get(&self, reference: &str) -> Result<Option<String>, CredentialError> {
        let reference = Self::validate_reference(reference)?;
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(SERVICE_NAME, &reference)
                .map_err(|error| CredentialError::Unavailable(error.to_string()))?;
            match entry.get_password() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(CredentialError::Operation(error.to_string())),
            }
        })
        .await
        .map_err(|error| CredentialError::Operation(error.to_string()))?
    }

    async fn set(&self, reference: &str, secret: String) -> Result<(), CredentialError> {
        let reference = Self::validate_reference(reference)?;
        if secret.trim().is_empty() {
            return Err(CredentialError::Operation(
                "API key cannot be blank".to_owned(),
            ));
        }
        tokio::task::spawn_blocking(move || {
            keyring::Entry::new(SERVICE_NAME, &reference)
                .map_err(|error| CredentialError::Unavailable(error.to_string()))?
                .set_password(&secret)
                .map_err(|error| CredentialError::Operation(error.to_string()))
        })
        .await
        .map_err(|error| CredentialError::Operation(error.to_string()))?
    }

    async fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        let reference = Self::validate_reference(reference)?;
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(SERVICE_NAME, &reference)
                .map_err(|error| CredentialError::Unavailable(error.to_string()))?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(CredentialError::Operation(error.to_string())),
            }
        })
        .await
        .map_err(|error| CredentialError::Operation(error.to_string()))?
    }
}
