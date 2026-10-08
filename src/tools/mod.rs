//! Core model-facing tools, typed schemas, and filtering subsystem.
//!
//! TL-01: Strongly typed tool definitions and derived JSON schemas.
//! TL-02: 28 core model-facing tools across 6 functional domains.
//! TL-05: Multi-stage deterministic model-visible filtering.

pub mod agent;
pub mod artifact;
pub mod definition;
pub mod error;
pub mod filter;
pub mod fs;
pub mod git;
pub mod process;
pub mod qa;
pub mod registry;
pub mod repo;
pub mod risk;
pub mod skill;
pub mod terminal;
pub mod web;

pub use agent::{DelegateTaskInput, DelegateTaskOutput, DelegateTaskTool};
pub use definition::{
    AdaptStrategyInput, AdaptStrategyOutput, AdaptStrategyTool, AnyTool, CompleteInput,
    CompleteOutput, CompleteTool, ResourceLimits, ToolAdapter, ToolExecutionContext, TypedTool,
    to_openai_tool,
};
pub use error::ToolError;
pub use filter::{FilterCriteria, ToolAuthorityScope, ToolFilter};
pub use registry::ToolRegistry;
pub use risk::{EffectiveRisk, RiskClass, is_sensitive_path};
pub use skill::{SkillsInspectTool, SkillsListTool};
pub use terminal::{
    TerminalReadStreamInput, TerminalReadStreamOutput, TerminalReadStreamTool, TerminalResizeInput,
    TerminalResizeOutput, TerminalResizeTool, TerminalSessionStatusInput,
    TerminalSessionStatusTool, TerminalStartSessionInput, TerminalStartSessionOutput,
    TerminalStartSessionTool, TerminalTerminateSessionInput, TerminalTerminateSessionOutput,
    TerminalTerminateSessionTool, TerminalWriteInputInput, TerminalWriteInputOutput,
    TerminalWriteInputTool,
};
pub use web::{
    WebFetchInput, WebFetchOutput, WebFetchTool, WebSearchInput, WebSearchOutput, WebSearchTool,
};
