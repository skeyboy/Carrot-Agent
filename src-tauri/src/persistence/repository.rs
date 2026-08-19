use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::conversation::{Conversation, ConversationChanges, NewConversation};
use crate::domain::storage::{ConversationStore, StoreError};

use super::database::{Database, DatabaseError};
use super::models::{ConversationChangeset, ConversationRow, NewConversationRow};
use super::schema::conversations;

#[derive(Clone)]
pub struct SqliteConversationStore {
    database: Database,
}

impl SqliteConversationStore {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    async fn find_row(&self, conversation_id: &str) -> Result<Option<ConversationRow>, StoreError> {
        let mut connection = self
            .database
            .connection()
            .await
            .map_err(store_unavailable)?;
        conversations::table
            .filter(conversations::id.eq(conversation_id))
            .select(ConversationRow::as_select())
            .first(&mut connection)
            .await
            .optional()
            .map_err(query_unavailable)
    }
}

#[async_trait]
impl ConversationStore for SqliteConversationStore {
    async fn list(&self) -> Result<Vec<Conversation>, StoreError> {
        let mut connection = self
            .database
            .connection()
            .await
            .map_err(store_unavailable)?;
        let rows = conversations::table
            .filter(conversations::archived.eq(false))
            .order((conversations::updated_at_ms.desc(), conversations::id.asc()))
            .select(ConversationRow::as_select())
            .load::<ConversationRow>(&mut connection)
            .await
            .map_err(query_unavailable)?;

        rows.into_iter().map(Conversation::try_from).collect()
    }

    async fn get(&self, id: &str) -> Result<Option<Conversation>, StoreError> {
        self.find_row(id)
            .await?
            .map(Conversation::try_from)
            .transpose()
    }

    async fn create(&self, input: NewConversation) -> Result<Conversation, StoreError> {
        let id = Uuid::now_v7().to_string();
        let now = now_ms()?;
        let row = NewConversationRow {
            id: &id,
            title: &input.title,
            default_provider_profile_id: &input.default_provider_profile_id,
            default_model: &input.default_model,
            archived: false,
            version: 1,
            created_at_ms: now,
            updated_at_ms: now,
        };
        let mut connection = self
            .database
            .connection()
            .await
            .map_err(store_unavailable)?;
        diesel::insert_into(conversations::table)
            .values(row)
            .execute(&mut connection)
            .await
            .map_err(query_unavailable)?;
        drop(connection);

        self.get(&id).await?.ok_or_else(|| StoreError::Unavailable {
            message: "created conversation could not be loaded".to_owned(),
        })
    }

    async fn update(
        &self,
        id: &str,
        changes: ConversationChanges,
    ) -> Result<Option<Conversation>, StoreError> {
        let next_version =
            changes
                .expected_version
                .checked_add(1)
                .ok_or_else(|| StoreError::InvalidData {
                    message: "conversation version overflow".to_owned(),
                })?;
        let changeset = ConversationChangeset {
            title: changes.title.as_deref(),
            default_provider_profile_id: changes.default_provider_profile_id.as_deref(),
            default_model: changes.default_model.as_deref(),
            version: next_version,
            updated_at_ms: now_ms()?,
        };
        let mut connection = self
            .database
            .connection()
            .await
            .map_err(store_unavailable)?;
        let updated = diesel::update(
            conversations::table.filter(
                conversations::id
                    .eq(id)
                    .and(conversations::version.eq(changes.expected_version)),
            ),
        )
        .set(changeset)
        .execute(&mut connection)
        .await
        .map_err(query_unavailable)?;
        drop(connection);

        if updated == 0 {
            return match self.get(id).await? {
                Some(_) => Err(StoreError::Conflict),
                None => Ok(None),
            };
        }

        self.get(id).await
    }

    async fn delete(&self, id: &str, expected_version: i64) -> Result<bool, StoreError> {
        let mut connection = self
            .database
            .connection()
            .await
            .map_err(store_unavailable)?;
        let deleted = diesel::delete(
            conversations::table.filter(
                conversations::id
                    .eq(id)
                    .and(conversations::version.eq(expected_version)),
            ),
        )
        .execute(&mut connection)
        .await
        .map_err(query_unavailable)?;
        drop(connection);

        if deleted == 0 && self.get(id).await?.is_some() {
            return Err(StoreError::Conflict);
        }

        Ok(deleted == 1)
    }
}

fn now_ms() -> Result<i64, StoreError> {
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

fn store_unavailable(error: DatabaseError) -> StoreError {
    StoreError::Unavailable {
        message: error.to_string(),
    }
}

fn query_unavailable(error: diesel::result::Error) -> StoreError {
    StoreError::Unavailable {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::conversation::{ConversationChanges, NewConversation};
    use crate::domain::storage::{ConversationStore, StoreError};

    use super::{Database, SqliteConversationStore};

    async fn store() -> (tempfile::TempDir, SqliteConversationStore) {
        let temp = tempfile::tempdir().expect("temporary directory");
        let database = Database::connect(&temp.path().join("carrot.sqlite3"))
            .await
            .expect("database should initialize");
        (temp, SqliteConversationStore::new(database))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn creates_updates_lists_and_deletes_conversations() {
        let (_temp, store) = store().await;
        let created = store
            .create(NewConversation {
                title: "First conversation".to_owned(),
                default_provider_profile_id: "openai".to_owned(),
                default_model: "gpt-test".to_owned(),
            })
            .await
            .expect("conversation should be created");

        assert_eq!(created.version, 1);
        assert_eq!(store.list().await.expect("list").len(), 1);

        let updated = store
            .update(
                &created.id,
                ConversationChanges {
                    expected_version: created.version,
                    title: Some("Renamed".to_owned()),
                    default_provider_profile_id: None,
                    default_model: None,
                },
            )
            .await
            .expect("update")
            .expect("conversation exists");

        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.version, 2);
        assert!(
            store
                .delete(&updated.id, updated.version)
                .await
                .expect("delete")
        );
        assert!(store.list().await.expect("list after delete").is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejects_stale_updates() {
        let (_temp, store) = store().await;
        let created = store
            .create(NewConversation {
                title: "Versioned".to_owned(),
                default_provider_profile_id: "openai".to_owned(),
                default_model: "gpt-test".to_owned(),
            })
            .await
            .expect("conversation should be created");
        let result = store
            .update(
                &created.id,
                ConversationChanges {
                    expected_version: 99,
                    title: Some("Stale".to_owned()),
                    default_provider_profile_id: None,
                    default_model: None,
                },
            )
            .await;

        assert!(matches!(result, Err(StoreError::Conflict)));
    }
}
