//! Local Diesel and SQLite persistence adapters.

mod database;
mod models;
mod repository;
mod schema;

pub use database::{Database, DatabaseError};
pub use repository::SqliteConversationStore;
