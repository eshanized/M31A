//! Release engineering: version authority, manifest, integrity, provenance,
//! SBOM, RC state machine, and blocker evaluation.
//!
//! Authority laws (see crate root / AGENTS.md) apply in full. In particular:
//! release metadata must never claim verification that did not occur, and
//! release automation fails closed rather than producing a partial or
//! unverifiable release.
//!
//! The single version authority is the `Cargo.toml` package version,
//! propagated at compile time via `CARGO_PKG_VERSION`. There is no second
//! version source.
//!
//! Cryptographic signing: the repository defines no signing infrastructure,
//! so provenance is factual, verifiable metadata — never a fake signature.
//! `SIGNING_MECHANISM` states this explicitly and validation rejects any
//! manifest that claims otherwise.

pub mod hygiene;
pub mod integrity;
pub mod manifest;
pub mod provenance;
pub mod sbom;
pub mod status;
pub mod version;

pub use hygiene::{HygieneFinding, HygieneKind, check_release_dir};

pub use integrity::{sha256_bytes, sha256_file, verify_sha256sums, write_sha256sums};
pub use manifest::{ArtifactEntry, ReleaseManifest};
pub use provenance::{Provenance, SIGNING_MECHANISM, collect_provenance, verify_provenance};
pub use sbom::{Sbom, SbomComponent, generate_sbom, validate_sbom};
pub use status::{
    Blocker, BlockerEvaluator, Finding, ReleaseClassification, ReleaseEvidence, ReleaseState,
};
pub use version::{
    PKG_VERSION, RUNTIME_NAME, cli_version_string, parse_cargo_toml_version, runtime_version,
};
