//! Autonomous Code Modification & Change Authority Subsystem.
//!
//! Core principle: The model proposes. The runtime decides.
//!
//! Owns:
//! - Pre-mutation reconciliation & conflict detection (`PreMutationReconciler`)
//! - Atomic multi-file mutation application with rollback (`AtomicChangeApplier`)
//! - Post-mutation re-observation & context refresh (`PostMutationObserver`)
//! - Independent diff self-review rejecting fake success (`DiffReviewer`)
//! - Change provenance and requirement traceability (`ChangeProvenanceStore`)
//! - Change authority coordinator (`ChangeAuthority`)

pub mod applier;
pub mod authority;
pub mod observer;
pub mod provenance;
pub mod reconciler;
pub mod review;

pub use applier::{AppliedChangeSet, AtomicChangeApplier, ChangeApplyError};
pub use authority::{ChangeAuthority, ChangeAuthorityError, ChangeExecutionOutcome};
pub use observer::{FreshMutationEvidence, PostMutationObserver};
pub use provenance::ChangeProvenanceStore;
pub use reconciler::{PreMutationReconciler, detect_generated_file};
pub use review::DiffReviewer;
