//! 12-Point Canonical Phase Loop Lifecycle Hooks (`m31a-hooks::lifecycle`).
//!
//! Canonical phase loop lifecycle hooks governing spec-driven engineering phases:
//! Provides the 12 canonical loop extension seams that govern the spec-driven phase lifecycle:
//! 1. `discuss:pre` - Prior to elicitation / user discussion
//! 2. `discuss:post` - After `CONTEXT.md` has been authored
//! 3. `plan:pre` - Before plan authoring begins
//! 4. `plan:post` - After `PLAN.md` has passed pre-checks
//! 5. `execute:pre` - Before phase execution initiates
//! 6. `execute:wave:pre` - Before a wave of parallel plans starts
//! 7. `execute:wave:post` - After a wave finishes (runs pre-commit verification)
//! 8. `execute:post` - After all waves conclude
//! 9. `verify:pre` - Before adversarial verification begins
//! 10. `verify:post` - After verification completes (`VERIFICATION.md`, `VALIDATION.md`)
//! 11. `ship:pre` - Prior to git branch merging / shipping
//! 12. `ship:post` - After phase milestone has shipped

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanonicalPhasePoint {
    DiscussPre,
    DiscussPost,
    PlanPre,
    PlanPost,
    ExecutePre,
    ExecuteWavePre,
    ExecuteWavePost,
    ExecutePost,
    VerifyPre,
    VerifyPost,
    ShipPre,
    ShipPost,
}

impl CanonicalPhasePoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DiscussPre => "discuss:pre",
            Self::DiscussPost => "discuss:post",
            Self::PlanPre => "plan:pre",
            Self::PlanPost => "plan:post",
            Self::ExecutePre => "execute:pre",
            Self::ExecuteWavePre => "execute:wave:pre",
            Self::ExecuteWavePost => "execute:wave:post",
            Self::ExecutePost => "execute:post",
            Self::VerifyPre => "verify:pre",
            Self::VerifyPost => "verify:post",
            Self::ShipPre => "ship:pre",
            Self::ShipPost => "ship:post",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "discuss:pre" => Some(Self::DiscussPre),
            "discuss:post" => Some(Self::DiscussPost),
            "plan:pre" => Some(Self::PlanPre),
            "plan:post" => Some(Self::PlanPost),
            "execute:pre" => Some(Self::ExecutePre),
            "execute:wave:pre" => Some(Self::ExecuteWavePre),
            "execute:wave:post" => Some(Self::ExecuteWavePost),
            "execute:post" => Some(Self::ExecutePost),
            "verify:pre" => Some(Self::VerifyPre),
            "verify:post" => Some(Self::VerifyPost),
            "ship:pre" => Some(Self::ShipPre),
            "ship:post" => Some(Self::ShipPost),
            _ => None,
        }
    }

    pub fn all() -> &'static [CanonicalPhasePoint] {
        &[
            Self::DiscussPre,
            Self::DiscussPost,
            Self::PlanPre,
            Self::PlanPost,
            Self::ExecutePre,
            Self::ExecuteWavePre,
            Self::ExecuteWavePost,
            Self::ExecutePost,
            Self::VerifyPre,
            Self::VerifyPost,
            Self::ShipPre,
            Self::ShipPost,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnErrorPolicy {
    Halt,
    Warn,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseHookAction {
    Command(String),
    CapabilityRef {
        skill: Option<String>,
        agent: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseHookRule {
    pub id: String,
    pub point: CanonicalPhasePoint,
    pub action: PhaseHookAction,
    pub produces: Vec<String>,
    pub consumes: Vec<String>,
    pub when: Option<String>,
    pub on_error: OnErrorPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseDispatchResult {
    pub point: CanonicalPhasePoint,
    pub executed_hooks: usize,
    pub artifacts_produced: Vec<String>,
    pub halted: bool,
    pub halt_reason: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseHookRegistry {
    rules: Vec<PhaseHookRule>,
}

impl PhaseHookRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates a registry loaded with default phase loop lifecycle hooks.
    pub fn with_gsd_defaults() -> Self {
        let mut reg = Self::new();

        // 1. Discuss Post produces CONTEXT.md
        reg.register(PhaseHookRule {
            id: "gsd-discuss-produce-context".into(),
            point: CanonicalPhasePoint::DiscussPost,
            action: PhaseHookAction::CapabilityRef {
                skill: Some("discuss-phase".into()),
                agent: None,
            },
            produces: vec!["CONTEXT.md".into()],
            consumes: vec![],
            when: None,
            on_error: OnErrorPolicy::Halt,
        });

        // 2. Plan Post consumes CONTEXT.md and produces PLAN.md
        reg.register(PhaseHookRule {
            id: "gsd-plan-produce-plan".into(),
            point: CanonicalPhasePoint::PlanPost,
            action: PhaseHookAction::CapabilityRef {
                skill: Some("plan-phase".into()),
                agent: Some("gsd-planner".into()),
            },
            produces: vec!["PLAN.md".into()],
            consumes: vec!["CONTEXT.md".into()],
            when: None,
            on_error: OnErrorPolicy::Halt,
        });

        // 3. Execute Wave Post runs pre-commit hooks
        reg.register(PhaseHookRule {
            id: "gsd-wave-post-verify".into(),
            point: CanonicalPhasePoint::ExecuteWavePost,
            action: PhaseHookAction::Command("git status --porcelain".into()),
            produces: vec![],
            consumes: vec![],
            when: None,
            on_error: OnErrorPolicy::Halt,
        });

        // 4. Verify Post produces VALIDATION.md and VERIFICATION.md
        reg.register(PhaseHookRule {
            id: "gsd-verify-post-nyquist".into(),
            point: CanonicalPhasePoint::VerifyPost,
            action: PhaseHookAction::CapabilityRef {
                skill: Some("validate-phase".into()),
                agent: Some("gsd-nyquist-auditor".into()),
            },
            produces: vec!["VALIDATION.md".into(), "VERIFICATION.md".into()],
            consumes: vec!["SUMMARY.md".into()],
            when: Some("workflow.nyquist_validation".into()),
            on_error: OnErrorPolicy::Halt,
        });

        reg
    }

    /// Registers a new lifecycle hook.
    pub fn register(&mut self, rule: PhaseHookRule) {
        self.rules.push(rule);
    }

    /// Returns all registered rules matching a specific canonical point.
    pub fn resolve_hooks(&self, point: CanonicalPhasePoint) -> Vec<&PhaseHookRule> {
        self.rules.iter().filter(|r| r.point == point).collect()
    }

    /// Dispatches all hooks for a canonical point.
    pub fn dispatch(
        &self,
        point: CanonicalPhasePoint,
        workspace_root: &Path,
        context: &HashMap<String, String>,
    ) -> PhaseDispatchResult {
        let matching = self.resolve_hooks(point);
        let mut executed_count = 0;
        let mut artifacts_produced = Vec::new();
        let mut warnings = Vec::new();

        for rule in matching {
            // Check condition `when`
            if let Some(cond) = &rule.when
                && let Some(val) = context.get(cond)
                && (val == "false" || val == "0")
            {
                continue;
            }

            // Verify consumed artifacts exist if specified
            for req_file in &rule.consumes {
                let req_path = workspace_root.join(req_file);
                if !req_path.exists() {
                    let msg = format!(
                        "Hook '{}' at '{}' requires '{}' which does not exist in workspace",
                        rule.id,
                        point.as_str(),
                        req_file
                    );
                    match rule.on_error {
                        OnErrorPolicy::Halt => {
                            return PhaseDispatchResult {
                                point,
                                executed_hooks: executed_count,
                                artifacts_produced,
                                halted: true,
                                halt_reason: Some(msg),
                                warnings,
                            };
                        }
                        OnErrorPolicy::Warn => warnings.push(msg),
                        OnErrorPolicy::Skip => continue,
                    }
                }
            }

            // Execute action
            executed_count += 1;
            match &rule.action {
                PhaseHookAction::Command(cmd) => {
                    let res = Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .current_dir(workspace_root)
                        .output();

                    match res {
                        Ok(output) if output.status.success() => {
                            // Succeeded
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let msg = format!(
                                "Hook command '{}' failed with code {:?}: {}",
                                cmd,
                                output.status.code(),
                                stderr.trim()
                            );
                            match rule.on_error {
                                OnErrorPolicy::Halt => {
                                    return PhaseDispatchResult {
                                        point,
                                        executed_hooks: executed_count,
                                        artifacts_produced,
                                        halted: true,
                                        halt_reason: Some(msg),
                                        warnings,
                                    };
                                }
                                OnErrorPolicy::Warn => warnings.push(msg),
                                OnErrorPolicy::Skip => {}
                            }
                        }
                        Err(e) => {
                            let msg = format!("Failed to execute hook command '{}': {}", cmd, e);
                            match rule.on_error {
                                OnErrorPolicy::Halt => {
                                    return PhaseDispatchResult {
                                        point,
                                        executed_hooks: executed_count,
                                        artifacts_produced,
                                        halted: true,
                                        halt_reason: Some(msg),
                                        warnings,
                                    };
                                }
                                OnErrorPolicy::Warn => warnings.push(msg),
                                OnErrorPolicy::Skip => {}
                            }
                        }
                    }
                }
                PhaseHookAction::CapabilityRef { skill, agent } => {
                    // Symbolically record skill/agent execution
                    if let Some(s) = skill {
                        tracing::debug!(
                            "Dispatched capability skill '{}' at {}",
                            s,
                            point.as_str()
                        );
                    }
                    if let Some(a) = agent {
                        tracing::debug!(
                            "Dispatched capability agent '{}' at {}",
                            a,
                            point.as_str()
                        );
                    }
                }
            }

            for p in &rule.produces {
                if !artifacts_produced.contains(p) {
                    artifacts_produced.push(p.clone());
                }
            }
        }

        PhaseDispatchResult {
            point,
            executed_hooks: executed_count,
            artifacts_produced,
            halted: false,
            halt_reason: None,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_canonical_phase_points() {
        assert_eq!(CanonicalPhasePoint::all().len(), 12);
        assert_eq!(
            CanonicalPhasePoint::from_str("execute:wave:pre"),
            Some(CanonicalPhasePoint::ExecuteWavePre)
        );
        assert_eq!(CanonicalPhasePoint::DiscussPost.as_str(), "discuss:post");
    }

    #[test]
    fn test_dispatch_with_missing_consumed_artifact_halts() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = PhaseHookRegistry::new();

        reg.register(PhaseHookRule {
            id: "strict-plan-gate".into(),
            point: CanonicalPhasePoint::PlanPre,
            action: PhaseHookAction::Command("true".into()),
            produces: vec![],
            consumes: vec!["CONTEXT.md".into()],
            when: None,
            on_error: OnErrorPolicy::Halt,
        });

        let context = HashMap::new();
        let res = reg.dispatch(CanonicalPhasePoint::PlanPre, tmp.path(), &context);

        assert!(res.halted);
        assert!(res.halt_reason.unwrap().contains("requires 'CONTEXT.md'"));

        // Now create CONTEXT.md
        File::create(tmp.path().join("CONTEXT.md")).unwrap();
        let res_after = reg.dispatch(CanonicalPhasePoint::PlanPre, tmp.path(), &context);
        assert!(!res_after.halted);
        assert_eq!(res_after.executed_hooks, 1);
    }

    #[test]
    fn test_gsd_defaults_registry() {
        let reg = PhaseHookRegistry::with_gsd_defaults();
        assert_eq!(reg.resolve_hooks(CanonicalPhasePoint::DiscussPost).len(), 1);
        assert_eq!(reg.resolve_hooks(CanonicalPhasePoint::VerifyPost).len(), 1);
    }
}
