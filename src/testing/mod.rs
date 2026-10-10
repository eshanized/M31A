//! Test-support utilities for integration testing.
//!
//! Houses deterministic harnesses shared by `tests/` integration targets but
//! living behind the library crate boundary. Contents here are generic test
//! infrastructure (async runtime guards, bounded awaits) — never production
//! runtime logic and never credential handling.
//!
//! Credential-dependent harnesses (real-model integration, `.env` loading,
//! LLM API-key environment reads) live exclusively under `tests/support/`
//! and must never be reintroduced here: production code, including test
//! support exported from the library, must not discover credentials through
//! the environment.
pub mod runtime_harness;
