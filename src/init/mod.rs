//! Initialization, onboarding lifecycle, and doctor diagnostics (FRX-01–FRX-03).
//!
//! Provides the durable initialization state machine, pre-SQLite bootstrap sentinel,
//! and host environment diagnostic probes.

pub mod doctor;
pub mod lifecycle;

pub use doctor::{DiagnosticProbe, DiagnosticStatus, DoctorEngine};
pub use lifecycle::{InitError, InitManager, InitSentinel, InitState, SetupStep};
