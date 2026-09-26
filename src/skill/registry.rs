//! Skill Registry with Collision Resolution & Monotonic Safety Validation (SKL-01, SKL-03, D-09).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use thiserror::Error;

use crate::skill::discovery::{SkillDiscovery, SkillPackage};
use crate::skill::manifest::SkillManifest;

/// Errors arising from skill registration and validation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SkillError {
    #[error("Monotonicity violation: skill '{id}' attempted to weaken safety constraint: {reason}")]
    MonotonicityViolation { id: String, reason: String },

    #[error(
        "Skill '{skill_id}' requests unauthorized capability '{capability}' not in agent envelope"
    )]
    UnauthorizedCapability {
        skill_id: String,
        capability: String,
    },

    #[error("Skill '{0}' not found in registry")]
    NotFound(String),
}

/// Registry managing active skills loaded across discovery tiers.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, SkillPackage>,
    overridden_skills: Vec<(String, SkillPackage)>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load and populate registry from multi-tier discovery.
    pub fn load_discovered(workspace_root: Option<&Path>) -> Result<Self, SkillError> {
        let mut registry = Self::new();
        let packages = SkillDiscovery::discover_all(workspace_root);
        for package in packages {
            registry.register(package)?;
        }
        Ok(registry)
    }

    /// Register a skill package, applying precedence rules and monotonic safety checks.
    pub fn register(&mut self, package: SkillPackage) -> Result<(), SkillError> {
        let id = package.manifest.id.clone();

        if let Some(existing) = self.skills.get(&id) {
            // Higher precedence tier replaces lower tier
            if package.origin > existing.origin {
                // Monotonic verification check: child cannot lower verification tier
                if package.manifest.verification.tier < existing.manifest.verification.tier {
                    return Err(SkillError::MonotonicityViolation {
                        id: id.clone(),
                        reason: format!(
                            "Cannot lower verification tier from {} to {}",
                            existing.manifest.verification.tier, package.manifest.verification.tier
                        ),
                    });
                }

                // Monotonic approval check: child cannot waive required approval
                if existing.manifest.risk_profile.requires_approval
                    && !package.manifest.risk_profile.requires_approval
                {
                    return Err(SkillError::MonotonicityViolation {
                        id: id.clone(),
                        reason: "Cannot remove mandatory operator approval requirement".to_string(),
                    });
                }

                // Monotonic risk level check: child cannot downgrade risk level
                if package.manifest.risk_profile.level < existing.manifest.risk_profile.level {
                    return Err(SkillError::MonotonicityViolation {
                        id: id.clone(),
                        reason: format!(
                            "Cannot downgrade risk level from {} to {}",
                            existing.manifest.risk_profile.level,
                            package.manifest.risk_profile.level
                        ),
                    });
                }

                let previous = self.skills.insert(id.clone(), package).unwrap();
                self.overridden_skills.push((id, previous));
            }
        } else {
            self.skills.insert(id, package);
        }

        Ok(())
    }

    /// Retrieve a skill package by identifier.
    pub fn get(&self, id: &str) -> Option<&SkillPackage> {
        self.skills.get(id)
    }

    /// List all registered skills.
    pub fn list(&self) -> Vec<&SkillPackage> {
        let mut list: Vec<&SkillPackage> = self.skills.values().collect();
        list.sort_by_key(|p| &p.manifest.id);
        list
    }

    /// Validate that an invoking agent possesses all capabilities required by a skill (SKL-03).
    pub fn validate_agent_authority(
        &self,
        skill: &SkillManifest,
        agent_envelope: &HashSet<String>,
    ) -> Result<(), SkillError> {
        for required_cap in &skill.required_capabilities {
            if !agent_envelope.contains(required_cap) {
                return Err(SkillError::UnauthorizedCapability {
                    skill_id: skill.id.clone(),
                    capability: required_cap.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::discovery::SkillOriginTier;
    use crate::skill::manifest::{
        SkillExecutionConfig, SkillExecutionMode, SkillProcedure, SkillRiskLevel, SkillRiskProfile,
        SkillVerificationSpec,
    };
    use semver::Version;

    fn create_dummy_package(
        id: &str,
        tier: SkillOriginTier,
        v_tier: u8,
        risk: SkillRiskLevel,
        approval: bool,
    ) -> SkillPackage {
        SkillPackage {
            manifest: SkillManifest {
                schema_version: 1,
                id: id.to_string(),
                name: id.to_string(),
                version: Version::new(1, 0, 0),
                description: "test".to_string(),
                required_capabilities: vec!["file_read".to_string()],
                input_schema: serde_json::json!({}),
                execution: SkillExecutionConfig {
                    mode: SkillExecutionMode::InTask,
                },
                procedure: SkillProcedure {
                    instructions: "test".to_string(),
                    steps: vec![],
                },
                verification: SkillVerificationSpec {
                    tier: v_tier,
                    commands: vec![],
                    evidence_required: vec![],
                },
                risk_profile: SkillRiskProfile {
                    level: risk,
                    requires_approval: approval,
                },
            },
            origin: tier,
            source_path: None,
            content_hash: "abcd".to_string(),
        }
    }

    #[test]
    fn test_registry_monotonic_rejection_on_lowered_tier() {
        let mut registry = SkillRegistry::new();
        let base = create_dummy_package(
            "test-skill",
            SkillOriginTier::Builtin,
            4,
            SkillRiskLevel::Medium,
            false,
        );
        registry.register(base).unwrap();

        // Workspace tier tries to lower verification tier to 2 -> rejected!
        let invalid = create_dummy_package(
            "test-skill",
            SkillOriginTier::Workspace,
            2,
            SkillRiskLevel::Medium,
            false,
        );
        let err = registry.register(invalid).unwrap_err();
        assert!(matches!(err, SkillError::MonotonicityViolation { .. }));
    }

    #[test]
    fn test_registry_monotonic_rejection_on_removed_approval() {
        let mut registry = SkillRegistry::new();
        let base = create_dummy_package(
            "test-skill",
            SkillOriginTier::Builtin,
            3,
            SkillRiskLevel::High,
            true,
        );
        registry.register(base).unwrap();

        // Workspace tier tries to remove required approval -> rejected!
        let invalid = create_dummy_package(
            "test-skill",
            SkillOriginTier::Workspace,
            3,
            SkillRiskLevel::High,
            false,
        );
        let err = registry.register(invalid).unwrap_err();
        assert!(matches!(err, SkillError::MonotonicityViolation { .. }));
    }

    #[test]
    fn test_agent_authority_validation() {
        let registry = SkillRegistry::new();
        let skill = create_dummy_package(
            "test-skill",
            SkillOriginTier::Builtin,
            3,
            SkillRiskLevel::Low,
            false,
        )
        .manifest;

        let mut allowed_caps = HashSet::new();
        allowed_caps.insert("file_read".to_string());
        assert!(
            registry
                .validate_agent_authority(&skill, &allowed_caps)
                .is_ok()
        );

        let empty_caps = HashSet::new();
        let err = registry
            .validate_agent_authority(&skill, &empty_caps)
            .unwrap_err();
        assert!(matches!(err, SkillError::UnauthorizedCapability { .. }));
    }
}
