//! Explicit TUI error states (Principles 10–11).
//!
//! Every major runtime failure becomes visible TUI state. The renderer never
//! silently returns an empty frame: failures are typed, carry a human-readable
//! message, and render through the normal shell (header / startup panel /
//! composer / footer).
//!
//! M31A visual identity is unchanged; this module only classifies failures so
//! the startup panel and notifications can explain *what* failed.

use serde::{Deserialize, Serialize};

/// Classification of a visible TUI failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TuiErrorKind {
    /// Canonical `AppRuntime` assembly failed.
    Runtime,
    /// `TuiRuntimeBridge` spawn or supervision failed.
    Bridge,
    /// Durable hydration (session / workspace / execution state) failed.
    Hydration,
    /// Model authority / provider / catalog failure surfaced to the cockpit.
    Model,
    /// Workspace resolution / configuration failure surfaced to the cockpit.
    Workspace,
}

impl TuiErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Bridge => "bridge",
            Self::Hydration => "hydration",
            Self::Model => "model",
            Self::Workspace => "workspace",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Runtime => "Runtime error",
            Self::Bridge => "Bridge error",
            Self::Hydration => "Hydration error",
            Self::Model => "Model error",
            Self::Workspace => "Workspace error",
        }
    }
}

/// A visible TUI failure: typed kind + human-readable message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiError {
    pub kind: TuiErrorKind,
    pub message: String,
}

impl TuiError {
    pub fn new(kind: TuiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(TuiErrorKind::Runtime, message)
    }

    pub fn bridge(message: impl Into<String>) -> Self {
        Self::new(TuiErrorKind::Bridge, message)
    }

    pub fn hydration(message: impl Into<String>) -> Self {
        Self::new(TuiErrorKind::Hydration, message)
    }

    pub fn model(message: impl Into<String>) -> Self {
        Self::new(TuiErrorKind::Model, message)
    }

    pub fn workspace(message: impl Into<String>) -> Self {
        Self::new(TuiErrorKind::Workspace, message)
    }

    /// Single-line presentation: `Bridge error: <message>`.
    pub fn display(&self) -> String {
        format!("{}: {}", self.kind.title(), self.message)
    }
}

impl std::fmt::Display for TuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_prefixes_kind_title() {
        let e = TuiError::bridge("boom");
        assert_eq!(e.display(), "Bridge error: boom");
    }

    #[test]
    fn kinds_have_stable_ids() {
        assert_eq!(TuiErrorKind::Runtime.as_str(), "runtime");
        assert_eq!(TuiErrorKind::Bridge.as_str(), "bridge");
        assert_eq!(TuiErrorKind::Hydration.as_str(), "hydration");
    }
}
