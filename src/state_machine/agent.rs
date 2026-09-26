//! Agent lifecycle state machine

use crate::state_machine::error::TransitionError;
use serde::{Deserialize, Serialize};

/// Agent states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    Starting,
    Initializing,
    Running,
    Paused,
    Completing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Initializing => write!(f, "Initializing"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Completing => write!(f, "Completing"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl std::str::FromStr for AgentState {
    type Err = TransitionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "starting" => Ok(Self::Starting),
            "initializing" => Ok(Self::Initializing),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "completing" => Ok(Self::Completing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(TransitionError::invalid_transition(s, "UnknownState")),
        }
    }
}

/// Events that can trigger agent state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentEvent {
    Initialize,
    Start,
    Pause,
    Resume,
    Complete,
    Fail,
    Cancel,
}

/// Open agent-role identity (R-01).
///
/// A role is identified by a stable snake_case string (`"implementer"`).
/// The 18 built-in ids are associated constructors below; any other
/// well-formed id is a legitimate identity whose *existence and behavior*
/// resolve through [`crate::agent::registry::RoleRegistry`].
///
/// This type carries NO behavioral metadata: prompt bindings, capability
/// envelopes, verification defaults, lifecycle stages, and concurrency
/// limits live in the registry. Never `match` on role identity in Rust to
/// decide behavior — resolve the registered definition instead.
///
/// Wire format: serializes as a plain string (`"implementer"`),
/// preserving a snake_case string encoding so persisted tasks,
/// handoffs, trailers, and events round-trip deterministically.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentRole(String);

/// Built-in compatibility aliases: normalized input → canonical role id.
///
/// Pure static data for parsing inputs written against historic aliases
/// (`"discovery"`, `"stack"`, …). This table is NOT existence authority —
/// existence is enforced by the role registry at validation boundaries
/// (planning, dispatch, workflow compilation). New roles use their exact id.
pub const ROLE_ALIASES: &[(&str, &str)] = &[
    ("planner", "planner"),
    ("researcher", "researcher"),
    ("architect", "architect"),
    ("requirementsarchitect", "architect"),
    ("systemarchitect", "architect"),
    ("roadmaparchitect", "architect"),
    ("implementer", "implementer"),
    ("reviewer", "reviewer"),
    ("verifier", "verifier"),
    ("diagnostician", "diagnostician"),
    ("integrator", "integrator"),
    ("discoveryanalyst", "discovery_analyst"),
    ("discovery", "discovery_analyst"),
    ("stackresearcher", "stack_researcher"),
    ("researchstack", "stack_researcher"),
    ("stack", "stack_researcher"),
    ("featuresresearcher", "features_researcher"),
    ("researchfeatures", "features_researcher"),
    ("features", "features_researcher"),
    ("architectureresearcher", "architecture_researcher"),
    ("researcharchitecture", "architecture_researcher"),
    ("architecture", "architecture_researcher"),
    ("pitfallsresearcher", "pitfalls_researcher"),
    ("researchpitfalls", "pitfalls_researcher"),
    ("pitfalls", "pitfalls_researcher"),
    ("securityresearcher", "security_researcher"),
    ("researchsecurity", "security_researcher"),
    ("security", "security_researcher"),
    ("deploymentresearcher", "deployment_researcher"),
    ("researchdeployment", "deployment_researcher"),
    ("deployment", "deployment_researcher"),
    ("synthesizer", "synthesizer"),
    ("synthesis", "synthesizer"),
    ("auditor", "auditor"),
    ("systemauditor", "auditor"),
    ("audit", "auditor"),
    ("releasecertifier", "release_certifier"),
    ("releasecertification", "release_certifier"),
    ("certifier", "release_certifier"),
];

impl AgentRole {
    /// Construct a role identity, normalizing case and surrounding whitespace.
    ///
    /// Construction always succeeds for non-empty input: identity is cheap,
    /// existence is enforced by the registry, never by this constructor.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into().trim().to_lowercase())
    }

    /// Raw identity string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this identity is well-formed (non-empty, `a-z0-9_` charset).
    pub fn is_well_formed(&self) -> bool {
        !self.0.is_empty()
            && self
                .0
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    // Built-in role identities (stable; wire-compatible with pre-27.5).
    pub fn planner() -> Self {
        Self("planner".to_string())
    }
    pub fn researcher() -> Self {
        Self("researcher".to_string())
    }
    pub fn architect() -> Self {
        Self("architect".to_string())
    }
    pub fn implementer() -> Self {
        Self("implementer".to_string())
    }
    pub fn reviewer() -> Self {
        Self("reviewer".to_string())
    }
    pub fn verifier() -> Self {
        Self("verifier".to_string())
    }
    pub fn diagnostician() -> Self {
        Self("diagnostician".to_string())
    }
    pub fn integrator() -> Self {
        Self("integrator".to_string())
    }
    pub fn discovery_analyst() -> Self {
        Self("discovery_analyst".to_string())
    }
    pub fn stack_researcher() -> Self {
        Self("stack_researcher".to_string())
    }
    pub fn features_researcher() -> Self {
        Self("features_researcher".to_string())
    }
    pub fn architecture_researcher() -> Self {
        Self("architecture_researcher".to_string())
    }
    pub fn pitfalls_researcher() -> Self {
        Self("pitfalls_researcher".to_string())
    }
    pub fn security_researcher() -> Self {
        Self("security_researcher".to_string())
    }
    pub fn deployment_researcher() -> Self {
        Self("deployment_researcher".to_string())
    }
    pub fn synthesizer() -> Self {
        Self("synthesizer".to_string())
    }
    pub fn auditor() -> Self {
        Self("auditor".to_string())
    }
    pub fn release_certifier() -> Self {
        Self("release_certifier".to_string())
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AgentRole {
    type Err = String;

    /// Parse a role identity with historic-alias compatibility.
    ///
    /// Accepts any well-formed id (registered or not) plus the historic
    /// aliases in [`ROLE_ALIASES`]. Fails only on empty input. Unknown ids
    /// parse successfully here and are rejected explicitly by the registry
    /// at validation boundaries.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase().replace(['_', '-'], "");
        if normalized.is_empty() {
            return Err("agent role id cannot be empty".to_string());
        }
        if let Some((_, canonical)) = ROLE_ALIASES.iter().find(|(alias, _)| *alias == normalized) {
            return Ok(Self(canonical.to_string()));
        }
        Ok(Self(s.trim().to_lowercase()))
    }
}

impl AgentState {
    /// Check if this state is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentState::Completed | AgentState::Failed | AgentState::Cancelled
        )
    }
}

/// Validate and execute an agent state transition.
pub fn transition_agent(
    current: AgentState,
    event: AgentEvent,
) -> Result<AgentState, TransitionError> {
    if current.is_terminal() {
        return Err(TransitionError::terminal_state(format!("{:?}", current)));
    }

    let new_state = match (current, event) {
        (AgentState::Starting, AgentEvent::Initialize) => AgentState::Initializing,
        (AgentState::Initializing, AgentEvent::Start) => AgentState::Running,
        (AgentState::Running, AgentEvent::Pause) => AgentState::Paused,
        (AgentState::Paused, AgentEvent::Resume) => AgentState::Running,
        (AgentState::Running, AgentEvent::Complete) => AgentState::Completing,
        (AgentState::Completing, AgentEvent::Complete) => AgentState::Completed,
        (state, AgentEvent::Fail) if !state.is_terminal() => AgentState::Failed,
        (state, AgentEvent::Cancel) if !state.is_terminal() => AgentState::Cancelled,
        (from, event) => {
            return Err(TransitionError::invalid_transition(
                format!("{:?}", from),
                format!("{:?}", event),
            ));
        }
    };

    Ok(new_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert_eq!(
            transition_agent(AgentState::Starting, AgentEvent::Initialize),
            Ok(AgentState::Initializing)
        );
        assert_eq!(
            transition_agent(AgentState::Initializing, AgentEvent::Start),
            Ok(AgentState::Running)
        );
        assert_eq!(
            transition_agent(AgentState::Running, AgentEvent::Pause),
            Ok(AgentState::Paused)
        );
        assert_eq!(
            transition_agent(AgentState::Paused, AgentEvent::Resume),
            Ok(AgentState::Running)
        );
        assert_eq!(
            transition_agent(AgentState::Running, AgentEvent::Complete),
            Ok(AgentState::Completing)
        );
        assert_eq!(
            transition_agent(AgentState::Completing, AgentEvent::Complete),
            Ok(AgentState::Completed)
        );
        assert_eq!(
            transition_agent(AgentState::Running, AgentEvent::Fail),
            Ok(AgentState::Failed)
        );
        assert_eq!(
            transition_agent(AgentState::Initializing, AgentEvent::Fail),
            Ok(AgentState::Failed)
        );
        assert_eq!(
            transition_agent(AgentState::Running, AgentEvent::Cancel),
            Ok(AgentState::Cancelled)
        );
    }

    #[test]
    fn test_terminal_states_reject_transitions() {
        let terminal_states = [
            AgentState::Completed,
            AgentState::Failed,
            AgentState::Cancelled,
        ];
        for state in terminal_states {
            assert!(transition_agent(state, AgentEvent::Start).is_err());
            assert!(transition_agent(state, AgentEvent::Fail).is_err());
        }
    }

    #[test]
    fn test_invalid_transitions_rejected() {
        assert!(transition_agent(AgentState::Starting, AgentEvent::Start).is_err());
        assert!(transition_agent(AgentState::Initializing, AgentEvent::Pause).is_err());
        assert!(transition_agent(AgentState::Paused, AgentEvent::Initialize).is_err());
        assert!(transition_agent(AgentState::Completing, AgentEvent::Pause).is_err());
    }

    #[test]
    fn test_is_terminal() {
        assert!(AgentState::Completed.is_terminal());
        assert!(AgentState::Failed.is_terminal());
        assert!(AgentState::Cancelled.is_terminal());
        assert!(!AgentState::Starting.is_terminal());
        assert!(!AgentState::Running.is_terminal());
        assert!(!AgentState::Paused.is_terminal());
        assert!(!AgentState::Completing.is_terminal());
    }

    #[test]
    fn test_agent_role_serialization_and_display() {
        let roles = [
            (AgentRole::planner(), "\"planner\"", "planner"),
            (AgentRole::researcher(), "\"researcher\"", "researcher"),
            (AgentRole::architect(), "\"architect\"", "architect"),
            (AgentRole::implementer(), "\"implementer\"", "implementer"),
            (AgentRole::reviewer(), "\"reviewer\"", "reviewer"),
            (AgentRole::verifier(), "\"verifier\"", "verifier"),
            (
                AgentRole::diagnostician(),
                "\"diagnostician\"",
                "diagnostician",
            ),
            (AgentRole::integrator(), "\"integrator\"", "integrator"),
            (
                AgentRole::discovery_analyst(),
                "\"discovery_analyst\"",
                "discovery_analyst",
            ),
            (
                AgentRole::stack_researcher(),
                "\"stack_researcher\"",
                "stack_researcher",
            ),
            (
                AgentRole::features_researcher(),
                "\"features_researcher\"",
                "features_researcher",
            ),
            (
                AgentRole::architecture_researcher(),
                "\"architecture_researcher\"",
                "architecture_researcher",
            ),
            (
                AgentRole::pitfalls_researcher(),
                "\"pitfalls_researcher\"",
                "pitfalls_researcher",
            ),
            (
                AgentRole::security_researcher(),
                "\"security_researcher\"",
                "security_researcher",
            ),
            (
                AgentRole::deployment_researcher(),
                "\"deployment_researcher\"",
                "deployment_researcher",
            ),
            (AgentRole::synthesizer(), "\"synthesizer\"", "synthesizer"),
            (AgentRole::auditor(), "\"auditor\"", "auditor"),
            (
                AgentRole::release_certifier(),
                "\"release_certifier\"",
                "release_certifier",
            ),
        ];

        for (role, json_str, display_str) in roles {
            assert_eq!(role.to_string(), display_str);
            assert_eq!(role.as_str(), display_str);
            assert!(role.is_well_formed());
            let parsed: AgentRole = display_str.parse().expect("parse failed");
            assert_eq!(parsed, role);

            let serialized = serde_json::to_string(&role).expect("serialize failed");
            assert_eq!(serialized, json_str);

            let deserialized: AgentRole =
                serde_json::from_str(json_str).expect("deserialize failed");
            assert_eq!(deserialized, role);
        }

        // Historic aliases still resolve to canonical ids.
        assert_eq!(
            "discovery".parse::<AgentRole>().expect("alias parse"),
            AgentRole::discovery_analyst()
        );
        assert_eq!(
            "stack".parse::<AgentRole>().expect("alias parse"),
            AgentRole::stack_researcher()
        );
        assert_eq!(
            "audit".parse::<AgentRole>().expect("alias parse"),
            AgentRole::auditor()
        );

        // Identity construction is open: unknown ids parse, but are not
        // well-formed-or-known by construction. Existence is enforced by the
        // role registry, never by this parser.
        let custom: AgentRole = "database_architect".parse().expect("open parse");
        assert_eq!(custom.as_str(), "database_architect");
        assert!(custom.is_well_formed());
        assert_ne!(custom, AgentRole::implementer());

        assert!("".parse::<AgentRole>().is_err());
        assert!("   ".parse::<AgentRole>().is_err());
    }
}
