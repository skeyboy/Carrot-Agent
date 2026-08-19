use diesel::{AsChangeset, Insertable, Queryable, Selectable};

use crate::domain::conversation::Conversation;
use crate::domain::storage::StoreError;

use super::schema::conversations;

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
