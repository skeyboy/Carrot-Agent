use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ProviderProfile {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub default_model: String,
    pub store_responses: bool,
}
