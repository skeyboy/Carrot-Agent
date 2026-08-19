use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AttachmentDescriptor {
    pub id: String,
    pub media_type: String,
    pub file_name: String,
    pub byte_length: u64,
}
