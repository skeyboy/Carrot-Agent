use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AttachmentDescriptor {
    pub id: String,
    pub conversation_id: String,
    pub media_type: String,
    pub file_name: String,
    pub byte_length: u64,
    pub content_hash: String,
    pub relative_path: String,
    pub created_at_ms: i64,
}
