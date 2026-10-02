//! Deployment & release channels: DEVELOPMENT vs PRODUCTION.
//!
//! One core runtime, multiple deployment channels. The channel is compile-time
//! artifact identity (`development` cargo feature; production is the default)
//! and determines installation names (`m31a` vs `m31a-dev`), global state
//! isolation, and update feeds — never security posture, workspace identity,
//! or the canonical `Cargo.toml` version authority.
//!
//! Module ownership: `release/` owns the release-candidate/evidence state
//! machine; `deployment/` owns the installed-artifact/channel lifecycle.
//! `config/` consumes deployment-aware paths; `runtime/` consumes the
//! deployment context; `tui/` projects it. No reverse dependencies.

pub mod artifact;
pub mod channel;
pub mod context;
pub mod features;
pub mod installer;
pub mod manifest;
pub mod paths;
pub mod promotion;
pub mod rollback;
pub mod updater;

pub use artifact::{ReleaseArtifact, SUPPORTED_TARGETS, is_supported_target};
pub use channel::{ChannelError, DeploymentChannel, UpdateChannel};
pub use context::{DeploymentContext, derive_build_id};
pub use features::{DevelopmentFeature, FeatureAvailability, FeatureGate};
pub use installer::{InstallError, Installer};
pub use manifest::{DeploymentManifest, DeploymentManifestError, ManifestArtifact};
pub use paths::DeploymentPaths;
pub use promotion::{PromotionError, evaluate_promotion};
pub use rollback::{RollbackError, rollback, rollback_available};
pub use updater::{UpdateCandidate, UpdateError, discover_update, feed_name};

/// Short `m31a --version` / `m31a-dev --version` line for the running artifact.
pub fn cli_version_string() -> String {
    DeploymentContext::current().cli_version_string()
}

/// Binary name for the running artifact (`m31a` or `m31a-dev`).
pub fn binary_name() -> &'static str {
    DeploymentChannel::current().binary_name()
}
