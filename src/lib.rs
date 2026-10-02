//! M31 Autonomous (M31A) - Rust-native autonomous software-engineering runtime.
//!
//! Core principle: The model proposes. The runtime decides.

pub mod agent;
pub mod budget;
pub mod capability;
pub mod change;
pub mod checkpoint;
pub mod cli;
pub mod config;
pub mod context;
pub mod controller;
pub mod dag;
pub mod deployment;
pub mod error;
pub mod eval;
pub mod events;
pub mod git;
pub mod ids;
pub mod init;
pub mod interaction;
pub mod kernel;
pub mod memory;
pub mod model;
pub mod persistence;
pub mod pipeline;
pub mod planning;
pub mod platform;
pub mod policy;
pub mod process;
pub mod prompt;
pub mod recovery;
pub mod release;
pub mod repo;
pub mod report;
pub mod runtime;
pub mod sandbox;
pub mod scheduler;
pub mod skill;
pub mod state;
pub mod state_machine;
pub mod telemetry;
pub mod testing;
pub mod tools;
pub mod tui;
pub mod verification;
pub mod workflow;
