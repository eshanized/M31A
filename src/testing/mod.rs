//! Test-support utilities for integration testing.
//!
//! Houses harnesses that are shared by `tests/` integration targets but must
//! live behind the library crate boundary. Contents here are test support,
//! never production runtime logic.
pub mod real_model;
pub mod runtime_harness;
