//! Persistence module - SQLite, streams, and artifact storage
//!
//! Per D-17, persistence/ owns the sqlx/SQLite implementation and transactional storage.
//! Per D-19, no generic platform/ module — platform-specific abstractions introduced
//! at subsystem boundaries when required (paths.rs lives in persistence/).

pub mod artifacts;
pub mod paths;
pub mod sqlite;
pub mod streams;

pub use artifacts::{
    ArtifactError, ArtifactMetadata, ArtifactProvenance, ArtifactRecord, ArtifactService,
    ArtifactStatus, ArtifactStore, FsArtifactStore, IntegrityCheckResult, compute_artifact_hash,
};
pub use paths::*;
pub use sqlite::{SqlitePool, create_pool, initialize_database, run_migrations};
pub use streams::{FileStream, StreamStore};
