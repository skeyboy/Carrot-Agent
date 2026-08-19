use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    pub request_timeout_seconds: u32,
    pub max_model_steps: u16,
    pub attachment_max_megabytes: u16,
    pub default_strategy: RunStrategy,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            request_timeout_seconds: 120,
            max_model_steps: 8,
            attachment_max_megabytes: 20,
            default_strategy: RunStrategy::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunStrategy {
    Fast,
    Auto,
    Quality,
}

impl RunStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Auto => "auto",
            Self::Quality => "quality",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "fast" => Self::Fast,
            "auto" => Self::Auto,
            "quality" => Self::Quality,
            _ => return None,
        })
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(10..=900).contains(&self.request_timeout_seconds) {
            return Err("request timeout must be between 10 and 900 seconds".to_owned());
        }
        if !(1..=64).contains(&self.max_model_steps) {
            return Err("max model steps must be between 1 and 64".to_owned());
        }
        if !(1..=100).contains(&self.attachment_max_megabytes) {
            return Err("attachment limit must be between 1 and 100 MB".to_owned());
        }
        Ok(())
    }
}
