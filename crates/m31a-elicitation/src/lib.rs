//! `m31a-elicitation`: Interactive terminal prompt, question choice, and approval engine.

pub mod approval;
pub mod questions;

pub use approval::{ApprovalDecision, ApprovalKind, ApprovalState};
pub use questions::QuestionItem;
