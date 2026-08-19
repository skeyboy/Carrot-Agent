use diesel::{AsChangeset, Insertable, Queryable, Selectable};

use crate::domain::attachment::AttachmentDescriptor;
use crate::domain::conversation::Conversation;
use crate::domain::storage::StoreError;

use super::schema::{attachments, conversations};

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = attachments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AttachmentRow {
    pub id: String,
    pub conversation_id: String,
    #[allow(dead_code)]
    pub item_id: Option<String>,
    pub file_name: String,
    pub media_type: String,
    pub byte_length: i64,
    pub content_hash: String,
    pub relative_path: String,
    pub status: String,
    pub created_at_ms: i64,
}

impl TryFrom<AttachmentRow> for AttachmentDescriptor {
    type Error = StoreError;

    fn try_from(row: AttachmentRow) -> Result<Self, Self::Error> {
        let byte_length = u64::try_from(row.byte_length).map_err(|_| StoreError::InvalidData {
            message: format!("attachment {} has an invalid byte length", row.id),
        })?;
        if row.id.trim().is_empty()
            || row.conversation_id.trim().is_empty()
            || row.file_name.trim().is_empty()
            || row.media_type != "image/png"
            || row.content_hash.trim().is_empty()
            || row.relative_path.trim().is_empty()
            || row.status != "ready"
            || row.created_at_ms < 0
        {
            return Err(StoreError::InvalidData {
                message: format!("attachment {} failed domain validation", row.id),
            });
        }
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            media_type: row.media_type,
            file_name: row.file_name,
            byte_length,
            content_hash: row.content_hash,
            relative_path: row.relative_path,
            created_at_ms: row.created_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = attachments)]
pub struct NewAttachmentRow<'a> {
    pub id: &'a str,
    pub conversation_id: &'a str,
    pub item_id: Option<&'a str>,
    pub file_name: &'a str,
    pub media_type: &'a str,
    pub byte_length: i64,
    pub content_hash: &'a str,
    pub relative_path: &'a str,
    pub status: &'a str,
    pub created_at_ms: i64,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub default_provider_profile_id: String,
    pub default_model: String,
    pub archived: bool,
    pub version: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl TryFrom<ConversationRow> for Conversation {
    type Error = StoreError;

    fn try_from(row: ConversationRow) -> Result<Self, Self::Error> {
        if row.id.trim().is_empty()
            || row.title.trim().is_empty()
            || row.default_provider_profile_id.trim().is_empty()
            || row.default_model.trim().is_empty()
            || row.version < 1
            || row.created_at_ms < 0
            || row.updated_at_ms < row.created_at_ms
        {
            return Err(StoreError::InvalidData {
                message: format!("conversation {} failed domain validation", row.id),
            });
        }

        Ok(Self {
            id: row.id,
            title: row.title,
            default_provider_profile_id: row.default_provider_profile_id,
            default_model: row.default_model,
            archived: row.archived,
            version: row.version,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = conversations)]
pub struct NewConversationRow<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub default_provider_profile_id: &'a str,
    pub default_model: &'a str,
    pub archived: bool,
    pub version: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(AsChangeset)]
#[diesel(table_name = conversations)]
pub struct ConversationChangeset<'a> {
    pub title: Option<&'a str>,
    pub default_provider_profile_id: Option<&'a str>,
    pub default_model: Option<&'a str>,
    pub version: i64,
    pub updated_at_ms: i64,
}
