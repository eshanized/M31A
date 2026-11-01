//! Autonomous multi-agent swarm coordinator and collaborative blackboard.
//!
//! Orchestrates specialized autonomous agents working in unison:
//! - Lead Architect: designs technical spec, module boundaries, and interfaces.
//! - Core Coder: executes surgical code edits and implementation.
//! - Quality Engineer (Tester): runs test verification and designs edge case checks.
//! - Adversary (Red Team): challenges implementations with boundary fuzzing and invariant attacks.
//! - Security Guardian: inspects shell executions, sandbox boundaries, and credential safety.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SwarmRoleKind {
    Architect,
    Coder,
    Tester,
    Adversary,
    Guardian,
}

impl SwarmRoleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Architect => "architect",
            Self::Coder => "coder",
            Self::Tester => "tester",
            Self::Adversary => "adversary",
            Self::Guardian => "guardian",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Architect => "Lead Architect (Turing)",
            Self::Coder => "Core Coder (Carmack)",
            Self::Tester => "Test Engineer (Hopper)",
            Self::Adversary => "Adversary Red Team (Lovelace)",
            Self::Guardian => "Security Guardian (Cerberus)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SwarmPhase {
    Architecting,
    Implementing,
    Testing,
    AdversarialGauntlet,
    SecurityReview,
    Completed,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwarmTaskStep {
    pub id: usize,
    pub title: String,
    pub assigned_role: SwarmRoleKind,
    pub is_completed: bool,
    pub output_summary: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwarmMessage {
    pub sender: SwarmRoleKind,
    pub timestamp: u64,
    pub content: String,
}

/// An adversarial challenge or boundary fuzz test posed by the Red Team.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdversarialChallenge {
    pub id: String,
    pub title: String,
    pub attack_vector: String,
    pub input_scenario: String,
    pub expected_invariant: String,
    pub survived: bool,
    pub outcome_notes: Option<String>,
}

/// Consolidated audit report from the adversarial gauntlet.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdversarialGauntletReport {
    pub mission_id: String,
    pub total_challenges: usize,
    pub survived_challenges: usize,
    pub challenges: Vec<AdversarialChallenge>,
    pub passed_gauntlet: bool,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwarmBlackboard {
    pub mission_id: String,
    pub objective: String,
    pub phase: SwarmPhase,
    pub plan_steps: Vec<SwarmTaskStep>,
    pub artifacts: HashMap<String, String>,
    pub messages: Vec<SwarmMessage>,
    pub challenges: Vec<AdversarialChallenge>,
}

impl SwarmBlackboard {
    pub fn new(mission_id: impl Into<String>, objective: impl Into<String>) -> Self {
        Self {
            mission_id: mission_id.into(),
            objective: objective.into(),
            phase: SwarmPhase::Architecting,
            plan_steps: Vec::new(),
            artifacts: HashMap::new(),
            messages: Vec::new(),
            challenges: Vec::new(),
        }
    }

    pub fn post_message(&mut self, sender: SwarmRoleKind, content: impl Into<String>) {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.messages.push(SwarmMessage {
            sender,
            timestamp: ts,
            content: content.into(),
        });
    }

    pub fn record_artifact(&mut self, name: impl Into<String>, content: impl Into<String>) {
        self.artifacts.insert(name.into(), content.into());
    }

    pub fn set_plan(&mut self, steps: Vec<SwarmTaskStep>) {
        self.plan_steps = steps;
    }

    pub fn complete_step(&mut self, step_id: usize, summary: impl Into<String>) -> bool {
        if let Some(step) = self.plan_steps.iter_mut().find(|s| s.id == step_id) {
            step.is_completed = true;
            step.output_summary = Some(summary.into());
            true
        } else {
            false
        }
    }

    pub fn record_challenge(&mut self, challenge: AdversarialChallenge) {
        self.challenges.push(challenge);
    }

    pub fn evaluate_gauntlet(&self) -> AdversarialGauntletReport {
        let total = self.challenges.len();
        let survived = self.challenges.iter().filter(|c| c.survived).count();
        let passed = total == 0 || survived == total;
        let summary = format!(
            "Adversarial Gauntlet: {}/{} challenges survived ({})",
            survived,
            total,
            if passed { "PASSED" } else { "FAILED" }
        );

        AdversarialGauntletReport {
            mission_id: self.mission_id.clone(),
            total_challenges: total,
            survived_challenges: survived,
            challenges: self.challenges.clone(),
            passed_gauntlet: passed,
            summary,
        }
    }
}

pub struct SwarmCoordinator {
    blackboard: Arc<RwLock<SwarmBlackboard>>,
}

impl SwarmCoordinator {
    pub fn new(mission_id: &str, objective: &str) -> Self {
        let bb = SwarmBlackboard::new(mission_id, objective);
        Self {
            blackboard: Arc::new(RwLock::new(bb)),
        }
    }

    pub fn blackboard(&self) -> Arc<RwLock<SwarmBlackboard>> {
        Arc::clone(&self.blackboard)
    }

    /// Advance swarm mission to next logical phase in standard workflow.
    pub fn transition_phase(&self) -> anyhow::Result<SwarmPhase> {
        let mut bb = self.blackboard.write().unwrap();
        let next_phase = match bb.phase {
            SwarmPhase::Architecting => {
                if !bb.artifacts.contains_key("architecture_spec") && bb.plan_steps.is_empty() {
                    anyhow::bail!(
                        "Cannot transition to Implementing: Architect has not published an architecture spec or plan steps."
                    );
                }
                SwarmPhase::Implementing
            }
            SwarmPhase::Implementing => {
                let pending_coder_tasks = bb
                    .plan_steps
                    .iter()
                    .any(|s| s.assigned_role == SwarmRoleKind::Coder && !s.is_completed);
                if pending_coder_tasks {
                    anyhow::bail!(
                        "Cannot transition to Testing: Coder has unfinished implementation steps."
                    );
                }
                SwarmPhase::Testing
            }
            SwarmPhase::Testing => {
                let pending_tests = bb
                    .plan_steps
                    .iter()
                    .any(|s| s.assigned_role == SwarmRoleKind::Tester && !s.is_completed);
                if pending_tests {
                    anyhow::bail!(
                        "Cannot transition to SecurityReview: Tester has unfinished test verifications."
                    );
                }
                SwarmPhase::SecurityReview
            }
            SwarmPhase::AdversarialGauntlet => SwarmPhase::SecurityReview,
            SwarmPhase::SecurityReview => SwarmPhase::Completed,
            SwarmPhase::Completed | SwarmPhase::Failed => bb.phase,
        };

        bb.phase = next_phase;
        Ok(next_phase)
    }

    /// Advance swarm mission through the enhanced Adversarial Gauntlet workflow.
    pub fn transition_phase_adversarial(&self) -> anyhow::Result<SwarmPhase> {
        let mut bb = self.blackboard.write().unwrap();
        let next_phase = match bb.phase {
            SwarmPhase::Architecting => {
                if !bb.artifacts.contains_key("architecture_spec") && bb.plan_steps.is_empty() {
                    anyhow::bail!("Cannot transition to Implementing: No architecture spec.");
                }
                SwarmPhase::Implementing
            }
            SwarmPhase::Implementing => {
                let pending_coder_tasks = bb
                    .plan_steps
                    .iter()
                    .any(|s| s.assigned_role == SwarmRoleKind::Coder && !s.is_completed);
                if pending_coder_tasks {
                    anyhow::bail!("Cannot transition to Testing: Coder has unfinished steps.");
                }
                SwarmPhase::Testing
            }
            SwarmPhase::Testing => {
                let pending_tests = bb
                    .plan_steps
                    .iter()
                    .any(|s| s.assigned_role == SwarmRoleKind::Tester && !s.is_completed);
                if pending_tests {
                    anyhow::bail!("Cannot transition to AdversarialGauntlet: Tests unfinished.");
                }
                SwarmPhase::AdversarialGauntlet
            }
            SwarmPhase::AdversarialGauntlet => {
                let report = bb.evaluate_gauntlet();
                if !report.passed_gauntlet {
                    anyhow::bail!(
                        "Cannot transition to SecurityReview: Adversarial gauntlet detected broken invariants: {}",
                        report.summary
                    );
                }
                SwarmPhase::SecurityReview
            }
            SwarmPhase::SecurityReview => SwarmPhase::Completed,
            SwarmPhase::Completed | SwarmPhase::Failed => bb.phase,
        };

        bb.phase = next_phase;
        Ok(next_phase)
    }

    /// Formulates prompt context for an assigned role based on current blackboard state.
    pub fn format_role_prompt(&self, role: SwarmRoleKind) -> String {
        let bb = self.blackboard.read().unwrap();
        let mut prompt = format!(
            "=== M31A Swarm Mission ===\n\
             Role: {}\n\
             Phase: {:?}\n\
             Objective: {}\n\n",
            role.display_name(),
            bb.phase,
            bb.objective
        );

        if role == SwarmRoleKind::Adversary {
            prompt.push_str(
                "ADVERSARIAL MANDATE:\n\
                 Your objective is to find edge cases, concurrency deadlocks, malicious inputs,\n\
                 and invariant violations in the proposed implementation before production release.\n\
                 Design rigorous adversarial checks and record findings on the blackboard.\n\n"
            );
        }

        if !bb.plan_steps.is_empty() {
            prompt.push_str("Task Plan:\n");
            for step in &bb.plan_steps {
                let status = if step.is_completed { "[X]" } else { "[ ]" };
                prompt.push_str(&format!(
                    "  {} Step {}: {} ({:?})\n",
                    status, step.id, step.title, step.assigned_role
                ));
            }
            prompt.push('\n');
        }

        if !bb.artifacts.is_empty() {
            prompt.push_str("Published Artifacts:\n");
            for name in bb.artifacts.keys() {
                prompt.push_str(&format!("  - {}\n", name));
            }
            prompt.push('\n');
        }

        if !bb.messages.is_empty() {
            prompt.push_str("Recent Swarm Discussion:\n");
            for msg in bb.messages.iter().rev().take(5) {
                prompt.push_str(&format!("  [{}]: {}\n", msg.sender.as_str(), msg.content));
            }
            prompt.push('\n');
        }

        prompt
    }

    /// Releases any path locks held by an assigned role in the swarm.
    pub fn cleanup_role_locks(&self, role: SwarmRoleKind) -> usize {
        crate::execution_lock::ExecutionLockManager::global().release_holder_locks(role.as_str())
    }

    /// Releases all path locks held under this swarm mission ID.
    pub fn cleanup_mission_locks(&self) -> usize {
        let bb = self.blackboard.read().unwrap();
        crate::execution_lock::ExecutionLockManager::global().release_holder_locks(&bb.mission_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_lifecycle() {
        let coord = SwarmCoordinator::new("m-01", "Build next-gen symbol navigation");
        let bb = coord.blackboard();

        // 1. Initial phase is Architecting
        assert_eq!(bb.read().unwrap().phase, SwarmPhase::Architecting);

        // 2. Transition without spec fails
        assert!(coord.transition_phase().is_err());

        // 3. Architect records spec and steps
        {
            let mut w = bb.write().unwrap();
            w.record_artifact(
                "architecture_spec",
                "Modules: symbol_index, outline, self_healing",
            );
            w.set_plan(vec![
                SwarmTaskStep {
                    id: 1,
                    title: "Implement symbol parser".to_string(),
                    assigned_role: SwarmRoleKind::Coder,
                    is_completed: false,
                    output_summary: None,
                },
                SwarmTaskStep {
                    id: 2,
                    title: "Add unit tests for symbol parser".to_string(),
                    assigned_role: SwarmRoleKind::Tester,
                    is_completed: false,
                    output_summary: None,
                },
            ]);
            w.post_message(SwarmRoleKind::Architect, "Architecture spec published.");
        }

        // 4. Can transition to Implementing
        let next = coord.transition_phase().unwrap();
        assert_eq!(next, SwarmPhase::Implementing);

        // 5. Coder completes step 1
        assert!(
            bb.write()
                .unwrap()
                .complete_step(1, "Implemented symbol_index.rs")
        );

        // 6. Transition to Testing
        let next = coord.transition_phase().unwrap();
        assert_eq!(next, SwarmPhase::Testing);

        // 7. Tester completes step 2
        assert!(bb.write().unwrap().complete_step(2, "All tests passing"));

        // 8. Transition to SecurityReview -> Completed
        let next = coord.transition_phase().unwrap();
        assert_eq!(next, SwarmPhase::SecurityReview);

        let final_phase = coord.transition_phase().unwrap();
        assert_eq!(final_phase, SwarmPhase::Completed);

        // 9. Path lock cleanup integration
        let lock_mgr = crate::execution_lock::ExecutionLockManager::global();
        let _g1 = lock_mgr
            .try_acquire_write("/tmp/swarm/spec.md", "architect", "drafting spec")
            .unwrap();
        let _g2 = lock_mgr
            .try_acquire_write("/tmp/swarm/code.rs", "m-01", "mission workspace")
            .unwrap();

        let role_cleaned = coord.cleanup_role_locks(SwarmRoleKind::Architect);
        assert_eq!(role_cleaned, 1);

        let mission_cleaned = coord.cleanup_mission_locks();
        assert_eq!(mission_cleaned, 1);

        std::mem::forget(_g1);
        std::mem::forget(_g2);
    }

    #[test]
    fn test_adversarial_gauntlet_workflow() {
        let coord = SwarmCoordinator::new("m-adv-01", "Implement concurrent cache");
        let bb = coord.blackboard();

        // Setup spec and completed tasks
        {
            let mut w = bb.write().unwrap();
            w.record_artifact("architecture_spec", "Concurrent cache");
            w.set_plan(vec![
                SwarmTaskStep {
                    id: 1,
                    title: "Implement cache".to_string(),
                    assigned_role: SwarmRoleKind::Coder,
                    is_completed: true,
                    output_summary: Some("Done".to_string()),
                },
                SwarmTaskStep {
                    id: 2,
                    title: "Test cache".to_string(),
                    assigned_role: SwarmRoleKind::Tester,
                    is_completed: true,
                    output_summary: Some("Done".to_string()),
                },
            ]);
        }

        // Advance to Implementing then Testing then AdversarialGauntlet
        assert_eq!(
            coord.transition_phase_adversarial().unwrap(),
            SwarmPhase::Implementing
        );
        assert_eq!(
            coord.transition_phase_adversarial().unwrap(),
            SwarmPhase::Testing
        );
        assert_eq!(
            coord.transition_phase_adversarial().unwrap(),
            SwarmPhase::AdversarialGauntlet
        );

        // Post a failed challenge -> cannot advance to SecurityReview
        bb.write().unwrap().record_challenge(AdversarialChallenge {
            id: "adv-1".to_string(),
            title: "Simultaneous read/write deadlock check".to_string(),
            attack_vector: "concurrency_race".to_string(),
            input_scenario: "1000 threads write while 5000 read".to_string(),
            expected_invariant: "No lock contention panic".to_string(),
            survived: false,
            outcome_notes: Some("Deadlock detected in RwLock upgrade".to_string()),
        });

        assert!(coord.transition_phase_adversarial().is_err());

        // Fix challenge to survived -> now can advance
        bb.write().unwrap().challenges[0].survived = true;
        assert_eq!(
            coord.transition_phase_adversarial().unwrap(),
            SwarmPhase::SecurityReview
        );
    }
}
