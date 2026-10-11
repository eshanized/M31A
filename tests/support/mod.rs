//! Shared integration-test support (test-only).
//!
//! Centralizes helpers that are allowed to touch LLM API-key environment
//! variables and `.env` files, as well as test doubles and harnesses.
//! Production code under `src/` must never depend on this module.
#![allow(dead_code)]

pub mod autonomous;
pub mod credentials;
pub mod mock_provider;
pub mod model_caller;
pub mod policy;
pub mod real_model;
pub mod runtime_harness;
