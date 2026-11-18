//! Persistent storage service initialization and SQLite database access.

use crate::error::RuntimeError;
use m31a_storage::Storage;
use std::path::Path;

/// Initialize the storage backend, either at a custom database path or the default path.
pub fn init_storage(custom_db_path: Option<&Path>) -> Result<Storage, RuntimeError> {
    if let Some(path) = custom_db_path {
        Storage::open(path).map_err(|e| RuntimeError::Storage(e.to_string()))
    } else {
        Storage::open_default().map_err(|e| RuntimeError::Storage(e.to_string()))
    }
}
