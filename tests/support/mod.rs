//! Shared integration-test support (test-only).
//!
//! Centralizes helpers that are allowed to touch LLM API-key environment
//! variables and `.env` files. Production code under `src/` must never
//! depend on this module.
#![allow(dead_code)]

pub mod autonomous;
pub mod credentials;
pub mod real_model;
