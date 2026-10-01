//! Initialization, onboarding lifecycle, and doctor diagnostics (FRX-01–FRX-03).
//!
//! Provides the durable initialization state machine, pre-SQLite bootstrap sentinel,
//! and host environment diagnostic probes.

pub mod doctor;
pub mod instance;
pub mod lifecycle;

pub use doctor::{DiagnosticProbe, DiagnosticStatus, DoctorEngine};
pub use instance::{
    OnboardingReason, StartupDecision, WorkspaceInstance, WorkspaceInstanceStore,
    begin_explicit_reonboarding, cached_instance, canonicalize_workspace_root, invalidate_instance,
    persist_successful_onboarding, resolve_shared_workspace_instance, resolve_startup,
    resolve_workspace_instance, workspace_instance_id,
};
pub use lifecycle::{
    INIT_STATE_VERSION, InitError, InitManager, InitSentinel, InitState, SetupStep,
};
