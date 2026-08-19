use std::path::Path;

use diesel::SqliteConnection;
use diesel::result::ConnectionError;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::{AsyncDieselConnectionManager, ManagerConfig};
use diesel_async::sync_connection_wrapper::SyncConnectionWrapper;
use diesel_async::{AsyncConnection, AsyncMigrationHarness, SimpleAsyncConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub type DbConnection = SyncConnectionWrapper<SqliteConnection>;
pub type DbPool = Pool<DbConnection>;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
const CONNECTION_PRAGMAS: &str = "\
    PRAGMA foreign_keys = ON;\
    PRAGMA journal_mode = WAL;\
    PRAGMA busy_timeout = 5000;\
";

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database path is not valid UTF-8")]
    InvalidPath,
    #[error("failed to build database pool: {0}")]
    PoolBuild(String),
    #[error("failed to acquire database connection: {0}")]
    Pool(String),
    #[error("database migration failed: {0}")]
    Migration(String),
}

#[derive(Clone)]
pub struct Database {
    pool: DbPool,
}

impl Database {
    pub async fn connect(path: &Path) -> Result<Self, DatabaseError> {
        let database_url = path.to_str().ok_or(DatabaseError::InvalidPath)?.to_owned();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| DatabaseError::PoolBuild(error.to_string()))?;
        }

        let mut manager_config = ManagerConfig::<DbConnection>::default();
        manager_config.custom_setup = Box::new(|url| {
            let url = url.to_owned();
            Box::pin(async move {
                let mut connection = DbConnection::establish(&url).await?;
                connection
                    .batch_execute(CONNECTION_PRAGMAS)
                    .await
                    .map_err(|error| ConnectionError::BadConnection(error.to_string()))?;
                Ok(connection)
            })
        });

        let manager = AsyncDieselConnectionManager::new_with_config(database_url, manager_config);
        let pool = Pool::builder(manager)
            .max_size(4)
            .build()
            .map_err(|error| DatabaseError::PoolBuild(error.to_string()))?;
        let database = Self { pool };
        database.run_migrations().await?;
        Ok(database)
    }

    pub(crate) async fn connection(
        &self,
    ) -> Result<diesel_async::pooled_connection::deadpool::Object<DbConnection>, DatabaseError>
    {
        self.pool
            .get()
            .await
            .map_err(|error| DatabaseError::Pool(error.to_string()))
    }

    async fn run_migrations(&self) -> Result<(), DatabaseError> {
        let connection = self.connection().await?;
        let mut harness = AsyncMigrationHarness::new(connection);
        harness
            .run_pending_migrations(MIGRATIONS)
            .map_err(|error| DatabaseError::Migration(error.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use diesel::sql_types::{BigInt, Text};
    use diesel::{QueryableByName, sql_query};
    use diesel_async::RunQueryDsl;

    use super::Database;

    #[derive(QueryableByName)]
    struct JournalMode {
        #[diesel(sql_type = Text)]
        journal_mode: String,
    }

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    #[derive(QueryableByName)]
    struct RunId {
        #[diesel(sql_type = Text)]
        id: String,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn applies_migrations_and_connection_pragmas() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let database = Database::connect(&temp.path().join("carrot.sqlite3"))
            .await
            .expect("database should initialize");
        let mut connection = database.connection().await.expect("database connection");

        let tables = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master \
             WHERE type = 'table' AND name IN \
             ('conversations', 'runs', 'items', 'run_events', 'pending_inputs',
              'tool_executions', 'plans', 'plan_steps', 'run_snapshots')",
        )
        .get_result::<Count>(&mut connection)
        .await
        .expect("table count");
        let journal = sql_query("PRAGMA journal_mode")
            .get_result::<JournalMode>(&mut connection)
            .await
            .expect("journal mode");

        assert_eq!(tables.count, 9);
        assert_eq!(journal.journal_mode, "wal");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enforces_active_run_and_event_sequence_constraints() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let database = Database::connect(&temp.path().join("carrot.sqlite3"))
            .await
            .expect("database should initialize");
        let mut connection = database.connection().await.expect("database connection");

        sql_query(
            "INSERT INTO conversations \
             (id, title, default_provider_profile_id, default_model, created_at_ms, updated_at_ms) \
             VALUES ('conversation-1', 'Test', 'openai', 'gpt-test', 1, 1)",
        )
        .execute(&mut connection)
        .await
        .expect("conversation insert");
        sql_query(
            "INSERT INTO runs \
             (id, conversation_id, status, phase, strategy, provider_profile_id, \
              provider_snapshot_json, model, runtime_instance_id, lease_expires_at_ms, \
              created_at_ms, updated_at_ms) \
             VALUES ('run-1', 'conversation-1', 'running', 'model_stream', 'auto', \
                     'openai', '{}', 'gpt-test', 'old-runtime', 100, 1, 1)",
        )
        .execute(&mut connection)
        .await
        .expect("first active run insert");

        let second_active_run = sql_query(
            "INSERT INTO runs \
             (id, conversation_id, status, phase, strategy, provider_profile_id, \
              provider_snapshot_json, model, created_at_ms, updated_at_ms) \
             VALUES ('run-2', 'conversation-1', 'queued', 'routing', 'auto', \
                     'openai', '{}', 'gpt-test', 2, 2)",
        )
        .execute(&mut connection)
        .await;
        assert!(second_active_run.is_err());

        sql_query(
            "INSERT INTO run_events (id, run_id, seq, kind, payload_json, persisted_at_ms) \
             VALUES ('event-1', 'run-1', 1, 'started', '{}', 1)",
        )
        .execute(&mut connection)
        .await
        .expect("first event insert");
        let duplicate_sequence = sql_query(
            "INSERT INTO run_events (id, run_id, seq, kind, payload_json, persisted_at_ms) \
             VALUES ('event-2', 'run-1', 1, 'duplicate', '{}', 2)",
        )
        .execute(&mut connection)
        .await;
        assert!(duplicate_sequence.is_err());

        let recoverable = sql_query(
            "SELECT id FROM runs \
             WHERE status NOT IN ('completed', 'failed', 'cancelled') \
               AND runtime_instance_id <> 'new-runtime'",
        )
        .load::<RunId>(&mut connection)
        .await
        .expect("recovery scan");
        assert_eq!(recoverable.len(), 1);
        assert_eq!(recoverable[0].id, "run-1");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn migration_is_reentrant_and_sqlite_rolls_back_transactions() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("carrot.sqlite3");
        let database = Database::connect(&path)
            .await
            .expect("first migration should initialize");
        drop(database);
        let database = Database::connect(&path)
            .await
            .expect("second migration run should be a no-op");
        let mut connection = database.connection().await.expect("database connection");

        sql_query("BEGIN IMMEDIATE")
            .execute(&mut connection)
            .await
            .expect("begin transaction");
        sql_query(
            "INSERT INTO conversations \
             (id, title, default_provider_profile_id, default_model, created_at_ms, updated_at_ms) \
             VALUES ('rolled-back', 'Test', 'openai', 'gpt-test', 1, 1)",
        )
        .execute(&mut connection)
        .await
        .expect("insert in transaction");
        sql_query("ROLLBACK")
            .execute(&mut connection)
            .await
            .expect("rollback transaction");

        let rows =
            sql_query("SELECT COUNT(*) AS count FROM conversations WHERE id = 'rolled-back'")
                .get_result::<Count>(&mut connection)
                .await
                .expect("conversation count");
        assert_eq!(rows.count, 0);
    }
}
