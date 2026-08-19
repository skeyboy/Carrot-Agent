#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub default_provider_profile_id: String,
    pub default_model: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConversation {
    pub title: String,
    pub default_provider_profile_id: String,
    pub default_model: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversationChanges {
    pub title: Option<String>,
    pub default_provider_profile_id: Option<String>,
    pub default_model: Option<String>,
}
