use std::time::{SystemTime, UNIX_EPOCH};

use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::domain::attachment::AttachmentDescriptor;
use crate::domain::storage::StoreError;

use super::database::Database;
use super::models::{AttachmentRow, NewAttachmentRow};
use super::schema::attachments;

#[derive(Clone)]
pub struct SqliteAttachmentStore {
    database: Database,
}

impl SqliteAttachmentStore {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AttachmentDescriptor>, StoreError> {
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let rows = attachments::table
            .filter(attachments::conversation_id.eq(conversation_id))
            .filter(attachments::status.eq("ready"))
            .order((attachments::created_at_ms.asc(), attachments::id.asc()))
            .select(AttachmentRow::as_select())
            .load::<AttachmentRow>(&mut connection)
            .await
            .map_err(query_error)?;
        rows.into_iter()
            .map(AttachmentDescriptor::try_from)
            .collect()
    }

    pub async fn insert(
        &self,
        descriptor: &AttachmentDescriptor,
    ) -> Result<AttachmentDescriptor, StoreError> {
        let byte_length =
            i64::try_from(descriptor.byte_length).map_err(|error| StoreError::InvalidData {
                message: error.to_string(),
            })?;
        let row = NewAttachmentRow {
            id: &descriptor.id,
            conversation_id: &descriptor.conversation_id,
            item_id: None,
            file_name: &descriptor.file_name,
            media_type: &descriptor.media_type,
            byte_length,
            content_hash: &descriptor.content_hash,
            relative_path: &descriptor.relative_path,
            status: "ready",
            created_at_ms: descriptor.created_at_ms,
        };
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        diesel::insert_into(attachments::table)
            .values(row)
            .execute(&mut connection)
            .await
            .map_err(query_error)?;
        Ok(descriptor.clone())
    }

    pub async fn delete(&self, id: &str) -> Result<Option<String>, StoreError> {
        let mut connection = self.database.connection().await.map_err(unavailable)?;
        let relative_path = attachments::table
            .filter(attachments::id.eq(id))
            .select(attachments::relative_path)
            .first::<String>(&mut connection)
            .await
            .optional()
            .map_err(query_error)?;
        if relative_path.is_some() {
            diesel::update(attachments::table.filter(attachments::id.eq(id)))
                .set(attachments::status.eq("deleted"))
                .execute(&mut connection)
                .await
                .map_err(query_error)?;
        }
        Ok(relative_path)
    }
}

pub fn now_ms() -> Result<i64, StoreError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| StoreError::Unavailable {
            message: error.to_string(),
        })?
        .as_millis();
    i64::try_from(millis).map_err(|error| StoreError::Unavailable {
        message: error.to_string(),
    })
}

fn unavailable(error: super::DatabaseError) -> StoreError {
    StoreError::Unavailable {
        message: error.to_string(),
    }
}

fn query_error(error: diesel::result::Error) -> StoreError {
    StoreError::Unavailable {
        message: error.to_string(),
    }
}
