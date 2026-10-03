//! Canonical 8 role profiles, 10-field AgentProfile contract, and SHA-256 fingerprinting (AGT-01, AGT-02, D-01).
//!
//! Every agent in M31A operates under an immutable strongly typed profile that
//! explicitly specifies its role, bounds, capability envelope, context policy, and instructions.

use crate::agent::envelope::CapabilityEnvelope;
use crate::agent::model_policy::ModelPolicy;
use crate::prompt::PromptReference;
use crate::state_machine::agent::AgentRole;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Context allocation and headroom policy for an agent (AGT-02, AGT-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub default_max_tokens: usize,
    pub reserved_headroom_tokens: usize,
    pub compaction_threshold_percent: u8,
}

/// Execution deadline and stall termination policy for an agent (AGT-02, AGT-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminationPolicy {
    pub step_stall_timeout_secs: u64,
    pub task_wall_clock_timeout_secs: u64,
    pub max_consecutive_action_failures: u32,
}

/// AGT-02 10-field Agent Profile definition contract.
///
/// Fully describes the behavioral envelope, capability permissions,
/// step ceilings, and PromptReference for an agent role instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub role: AgentRole,
    pub description: String,
    pub model_policy: ModelPolicy,
    pub capability_policy: CapabilityEnvelope,
    pub sandbox_policy: String,
    pub context_policy: ContextPolicy,
    pub max_steps: u32, // Hard immutable ceiling for the role
    pub termination_policy: TerminationPolicy,
    pub prompt_ref: PromptReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_instructions: Option<String>,
}

/// Safe configuration overrides applied to a built-in profile (D-01, T-06-01).
///
/// Overrides can only tighten step limits or tune model preferences.
/// Hard step ceilings and mandatory system instructions cannot be relaxed or removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOverride {
    pub max_steps: Option<u32>,
    pub preferred_model: Option<String>,
    pub fallback_models: Option<Vec<String>>,
    pub additional_instructions: Option<String>,
    pub temperature_millicelsius: Option<u32>,
}

/// Default preferred model when a profile does not receive an explicit model
/// assignment from runtime configuration.
///
/// Declarative authority boundary: this is a fallback default (Class B),
/// not a capability claim. The effective model resolves from user configuration
/// (`agents.default_model` / provider registry) whenever present;
/// `AgentProfile::apply_override` allows tightening. A single named constant
/// centralizes the default so changes occur in one place.
pub const DEFAULT_PREFERRED_MODEL: &str = "claude-3-7-sonnet";
/// Default fallback model. Same authority rules as [`DEFAULT_PREFERRED_MODEL`].
pub const DEFAULT_FALLBACK_MODEL: &str = "gpt-4o";

/// Errors raised when a configuration override violates profile safety invariants.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProfileOverrideError {
    #[error("cannot exceed hard role step ceiling of {ceiling}, requested {requested}")]
    StepCeilingExceeded { ceiling: u32, requested: u32 },
    #[error("mandatory system instructions cannot be cleared")]
    CannotRemoveMandatoryInstruction,
}

impl AgentProfile {
    /// Compute deterministic SHA-256 fingerprint of the effective profile (D-01).
    pub fn fingerprint(&self) -> String {
        let serialized = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        format!("{:x}", hasher.finalize())
    }

    /// Retrieve immutable built-in profile for a role id (AGT-01, D-01).
    ///
    /// Compatibility shim over [`crate::agent::registry::RoleRegistry`]:
    /// the registry is the single authority for role definitions, and this
    /// function materializes its profile template. Prefer
    /// `RoleRegistry::profile_for` in new code (explicit `Result` instead of
    /// panic). Panics only if the registry lost a built-in definition, which
    /// indicates a broken initialization, never a user error.
    pub fn built_in(role: AgentRole) -> Self {
        crate::agent::registry::RoleRegistry::profile_for_global(&role)
            .expect("built-in role definition missing from RoleRegistry")
    }

    /// Apply a safe configuration override to this profile (D-01, T-06-01).
    pub fn apply_override(&self, ovr: ProfileOverride) -> Result<Self, ProfileOverrideError> {
        let mut modified = self.clone();

        // 1. Step limits can only be tightened, never loosened past the role hard ceiling
        if let Some(steps) = ovr.max_steps {
            if steps > self.max_steps {
                return Err(ProfileOverrideError::StepCeilingExceeded {
                    ceiling: self.max_steps,
                    requested: steps,
                });
            }
            modified.max_steps = steps;
        }

        // 2. Model preferences can be tuned
        if let Some(pref) = ovr.preferred_model {
            modified.model_policy.preferred_model = pref;
        }
        if let Some(fallbacks) = ovr.fallback_models {
            modified.model_policy.fallback_models = fallbacks;
        }
        if let Some(temp) = ovr.temperature_millicelsius {
            modified.model_policy.temperature_millicelsius = temp;
        }

        // 3. System instructions can only be augmented, never cleared
        if let Some(extra) = ovr.additional_instructions {
            if extra.trim().is_empty() {
                return Err(ProfileOverrideError::CannotRemoveMandatoryInstruction);
            }
            modified.additional_instructions = match self.additional_instructions {
                Some(ref existing) => Some(format!("{}\n\n{}", existing, extra.trim())),
                None => Some(extra.trim().to_string()),
            };
        }

        Ok(modified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_roles_have_valid_profiles() {
        use crate::agent::registry::RoleRegistry;
        let guard = RoleRegistry::global().read().expect("registry readable");
        for id in guard.builtin_ids() {
            let role = AgentRole::new(&id);
            let profile = guard.profile_for(&role).expect("built-in registered");
            assert_eq!(profile.role, role);
            assert!(!profile.id.is_empty());
            assert!(!profile.description.is_empty());
            assert!(!profile.sandbox_policy.is_empty());
            assert!(!profile.prompt_ref.id.is_empty());
            let expected_version =
                if role == AgentRole::auditor() || role == AgentRole::release_certifier() {
                    2
                } else if role == AgentRole::implementer()
                    || role == AgentRole::reviewer()
                    || role == AgentRole::verifier()
                    || role == AgentRole::diagnostician()
                {
                    // Canonical v2 generation (wiring remediation v0.1.1):
                    // role defaults bind the canonical contract version.
                    2
                } else {
                    1
                };
            assert_eq!(profile.prompt_ref.version, expected_version);
            assert!(profile.max_steps > 0);
            assert!(profile.context_policy.default_max_tokens > 0);
            assert!(profile.termination_policy.step_stall_timeout_secs > 0);
            assert!(!profile.capability_policy.allowed_capabilities.is_empty());

            let fp = profile.fingerprint();
            assert_eq!(fp.len(), 64); // SHA-256 hex string length
            assert_eq!(fp, profile.fingerprint()); // Reproducible
        }
    }

    #[test]
    fn test_profile_override_step_ceiling_enforcement() {
        let profile = AgentProfile::built_in(AgentRole::planner());
        assert_eq!(profile.max_steps, 20);

        // Tightening steps is permitted
        let valid_ovr = ProfileOverride {
            max_steps: Some(15),
            ..Default::default()
        };
        let updated = profile.apply_override(valid_ovr).unwrap();
        assert_eq!(updated.max_steps, 15);

        // Exceeding hard role ceiling fails closed
        let invalid_ovr = ProfileOverride {
            max_steps: Some(25),
            ..Default::default()
        };
        let err = profile.apply_override(invalid_ovr).unwrap_err();
        assert_eq!(
            err,
            ProfileOverrideError::StepCeilingExceeded {
                ceiling: 20,
                requested: 25,
            }
        );
    }

    #[test]
    fn test_profile_override_instruction_augmentation() {
        let profile = AgentProfile::built_in(AgentRole::implementer());
        let ovr = ProfileOverride {
            additional_instructions: Some("Focus strictly on memory safety.".to_string()),
            ..Default::default()
        };
        let updated = profile.apply_override(ovr).unwrap();
        assert_eq!(
            updated.additional_instructions.as_deref(),
            Some("Focus strictly on memory safety.")
        );
    }

    #[test]
    fn test_profile_override_rejects_empty_instruction() {
        let profile = AgentProfile::built_in(AgentRole::implementer());
        let ovr = ProfileOverride {
            additional_instructions: Some("   ".to_string()),
            ..Default::default()
        };
        let err = profile.apply_override(ovr).unwrap_err();
        assert_eq!(err, ProfileOverrideError::CannotRemoveMandatoryInstruction);
    }
}
