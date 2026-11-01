//! `m31a-fuzzy`: High-performance fuzzy matching, scoring, and candidate search engine.

pub mod candidates;
pub mod file_search;
pub mod matcher;

pub use candidates::{Candidate, CandidateType};
pub use file_search::WorkspaceScanner;
pub use matcher::{fuzzy_match, fuzzy_match_indices};
