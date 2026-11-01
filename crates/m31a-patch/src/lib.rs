//! `m31a-patch`: High-performance multi-file patch engine.
//!
//! Supports structured multi-file V4A patch grammar (`*** Begin Patch`), standard unified diffs,
//! search/replace chunks, fuzzy hunk matching, and two-phase atomic file transactions.

pub mod engine;
pub mod fuzzy_matcher;
pub mod heredoc;
pub mod streaming_parser;
pub mod syntax_validator;
pub mod transaction;
pub mod unified_diff;
pub mod v4a;

pub use engine::{PatchEngine, PatchEvent};
pub use fuzzy_matcher::{FuzzyMatcher, MatchLocation};
pub use heredoc::extract_apply_patch_heredoc;
pub use syntax_validator::SyntaxValidator;
pub use transaction::{PatchTransaction, TransactionStats};
pub use unified_diff::{FileAction, ParsedPatch, PatchHunk, PatchLine, UnifiedDiffParser};
pub use v4a::parse_v4a_patch;
