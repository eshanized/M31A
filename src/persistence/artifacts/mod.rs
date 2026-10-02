//! Artifact storage module
//!
//! Per PST-03, filesystem/object storage stores large logs, reports, patches,
//! test output, screenshots, and generated artifacts.

pub mod fs_store;
pub mod quota;
pub mod service;

pub use fs_store::{ArtifactStore, EvidenceClassification, FsArtifactStore};
pub use quota::{ArtifactExemption, QuotaEnforcer, QuotaError, StreamingQuotaWriter};
pub use service::{
    ArtifactError, ArtifactMetadata, ArtifactProvenance, ArtifactRecord, ArtifactService,
    ArtifactStatus, IntegrityCheckResult, compute_artifact_hash,
};
