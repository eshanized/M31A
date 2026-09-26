//! Skill Verification Plan Compiler (SKL-01, SKL-03, D-12).
//!
//! Transforms declarative SkillVerificationSpec into an executable VerificationPlan,
//! enforcing runtime monotonicity: max(skill_tier, profile_tier, mission_tier).

use serde::{Deserialize, Serialize};

use crate::skill::manifest::SkillVerificationSpec;
use crate::verification::types::CheckTier;

/// Executable verification plan compiled from a skill's declared verification requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillVerificationPlan {
    pub skill_id: String,
    pub declared_tier: CheckTier,
    pub effective_tier: CheckTier,
    pub commands: Vec<String>,
    pub evidence_required: Vec<String>,
}

/// Compiler transforming skill verification specs into executable verification plans.
pub struct SkillVerificationCompiler;

impl SkillVerificationCompiler {
    /// Compile a verification spec into an executable plan, applying monotonic tier strengthening.
    pub fn compile(
        skill_id: &str,
        spec: &SkillVerificationSpec,
        minimum_tier: Option<CheckTier>,
    ) -> Result<SkillVerificationPlan, String> {
        let declared_tier = CheckTier::from_u8(spec.tier)
            .ok_or_else(|| format!("Invalid verification tier: {}", spec.tier))?;

        // Monotonic strengthening: effective tier cannot be lower than minimum required tier (D-12)
        let effective_tier = if let Some(min) = minimum_tier {
            std::cmp::max(declared_tier, min)
        } else {
            declared_tier
        };

        Ok(SkillVerificationPlan {
            skill_id: skill_id.to_string(),
            declared_tier,
            effective_tier,
            commands: spec.commands.clone(),
            evidence_required: spec.evidence_required.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monotonic_verification_tier_strengthening() {
        let spec = SkillVerificationSpec {
            tier: 2, // Compiler tier
            commands: vec!["cargo check".to_string()],
            evidence_required: vec![],
        };

        // If minimum environment tier is Tier 4 (Static Analysis), effective tier strengthens to 4
        let plan = SkillVerificationCompiler::compile(
            "test-skill",
            &spec,
            Some(CheckTier::StaticAnalysis),
        )
        .unwrap();

        assert_eq!(plan.declared_tier, CheckTier::Compiler);
        assert_eq!(plan.effective_tier, CheckTier::StaticAnalysis);
    }

    #[test]
    fn test_verification_plan_preserves_higher_skill_tier() {
        let spec = SkillVerificationSpec {
            tier: 5, // Diff/Invariants tier
            commands: vec!["git diff".to_string()],
            evidence_required: vec![],
        };

        // If minimum is Tier 3, effective tier remains 5 (max wins)
        let plan = SkillVerificationCompiler::compile("test-skill", &spec, Some(CheckTier::Tests))
            .unwrap();

        assert_eq!(plan.declared_tier, CheckTier::DiffInvariants);
        assert_eq!(plan.effective_tier, CheckTier::DiffInvariants);
    }
}
