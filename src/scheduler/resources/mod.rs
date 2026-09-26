//! Hierarchical resource locking and conflict resolution.

pub mod key;
pub mod lease;
pub mod manager;

pub use key::{LockMode, PathScope, ResourceKey, ResourceNamespace};
pub use lease::{ActiveLease, LeaseId, LeaseReleaser, ResourceLeaseGuard};
pub use manager::{ResourceConflictError, ResourceManager};
