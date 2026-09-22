//! Context compilation and prompt-injection defense subsystem.
//!
//! Submodules:
//! - `envelope`: XML trust envelopes with prompt-injection smuggling defense.
//! - `tokenizer`: TokenizerAdapter with exact BPE and conservative fallback.
//! - `externalize`: Dynamic artifact externalization and preview generation.
//! - `priority`: 5-class deterministic priority pipeline and reverse compaction engine.
//! - `manifest`: Auditable ContextCompilationManifest model.
//! - `compiler`: ProductionContextCompiler satisfying ContextCompiler seam.

pub mod compiler;
pub mod envelope;
pub mod evidence;
pub mod externalize;
pub mod manifest;
pub mod priority;
pub mod tokenizer;

pub use compiler::*;
pub use envelope::*;
pub use evidence::*;
pub use externalize::*;
pub use manifest::*;
pub use priority::*;
pub use tokenizer::*;
