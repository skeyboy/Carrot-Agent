//! Local Diesel and SQLite persistence adapters.

mod attachment_repository;
mod database;
mod models;
mod repository;
mod schema;

pub use attachment_repository::{SqliteAttachmentStore, now_ms};
pub use database::{Database, DatabaseError};
pub use repository::SqliteConversationStore;
