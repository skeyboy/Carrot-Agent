use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type, thiserror::Error)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppError {
    #[error("internal application error: {message}")]
    Internal { message: String },
}

impl AppError {
    #[allow(dead_code)]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}
