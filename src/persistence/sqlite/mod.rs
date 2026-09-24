//! SQLite persistence module
//!
//! Per D-03, sqlx for SQLite (async, compile-time checked queries, migration support).
//! Per D-17, persistence/ owns the sqlx/SQLite implementation.

pub mod pool;
pub mod repositories;
pub mod schema;
pub mod transaction;

pub use pool::create_pool;
pub use repositories::*;
pub use schema::{initialize_database, run_migrations};
pub use sqlx::sqlite::SqlitePool;
pub use transaction::SqliteTransactionManager;
