use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type, thiserror::Error)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppError {
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("resource not found: {message}")]
    NotFound { message: String },
    #[error("resource changed concurrently: {message}")]
    Conflict { message: String },
    #[error("storage error: {message}")]
    Storage { message: String },
    #[error("configuration error: {message}")]
    Configuration { message: String },
    #[error("internal application error: {message}")]
    Internal { message: String },
}

impl AppError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn not_found(resource: &str, id: &str) -> Self {
        Self::NotFound {
            message: format!("{resource} '{id}' was not found"),
        }
    }

    #[allow(dead_code)]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl From<crate::domain::storage::StoreError> for AppError {
    fn from(error: crate::domain::storage::StoreError) -> Self {
        match error {
            crate::domain::storage::StoreError::Conflict => Self::Conflict {
                message: "reload the latest record and try again".to_owned(),
            },
            crate::domain::storage::StoreError::InvalidData { message }
            | crate::domain::storage::StoreError::Unavailable { message } => {
                Self::Storage { message }
            }
        }
    }
}

impl From<crate::persistence::DatabaseError> for AppError {
    fn from(error: crate::persistence::DatabaseError) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<crate::providers::ProviderConfigError> for AppError {
    fn from(error: crate::providers::ProviderConfigError) -> Self {
        Self::Configuration {
            message: error.to_string(),
        }
    }
}
