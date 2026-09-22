use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Exhaustive 12-stage execution loop for the Autonomy Controller (D-02, AUT-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStage {
    Observe,
    IdentifyReadyWork,
    ValidatePolicyAndResources,
    AllocateWorkers,
    CompileContext,
    ExecuteBoundedWork,
    CollectResult,
    UpdateState,
    Verify,
    ClassifyFailure,
    RecoverOrReplan,
    Checkpoint,
}

impl LoopStage {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::IdentifyReadyWork => "identify_ready_work",
            Self::ValidatePolicyAndResources => "validate_policy_and_resources",
            Self::AllocateWorkers => "allocate_workers",
            Self::CompileContext => "compile_context",
            Self::ExecuteBoundedWork => "execute_bounded_work",
            Self::CollectResult => "collect_result",
            Self::UpdateState => "update_state",
            Self::Verify => "verify",
            Self::ClassifyFailure => "classify_failure",
            Self::RecoverOrReplan => "recover_or_replan",
            Self::Checkpoint => "checkpoint",
        }
    }

    pub fn is_first(&self) -> bool {
        matches!(self, Self::Observe)
    }

    pub fn is_last(&self) -> bool {
        matches!(self, Self::Checkpoint)
    }
}

impl fmt::Display for LoopStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Tracks the live execution progress of the Autonomy Controller, decoupled from MissionState (D-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerProgress {
    pub cycle: u64,
    pub current_stage: LoopStage,
    pub consecutive_idle_cycles: u32,
    pub failure_count: u32,
    pub recovery_count: u32,
    pub last_stage_transition_at: DateTime<Utc>,
}

impl ControllerProgress {
    pub fn new() -> Self {
        Self {
            cycle: 0,
            current_stage: LoopStage::Observe,
            consecutive_idle_cycles: 0,
            failure_count: 0,
            recovery_count: 0,
            last_stage_transition_at: Utc::now(),
        }
    }

    pub fn advance_stage(&mut self, next: LoopStage) {
        self.current_stage = next;
        self.last_stage_transition_at = Utc::now();
    }

    pub fn increment_cycle(&mut self) {
        self.cycle += 1;
        self.current_stage = LoopStage::Observe;
        self.last_stage_transition_at = Utc::now();
    }

    pub fn record_idle(&mut self) {
        self.consecutive_idle_cycles += 1;
    }

    pub fn reset_idle(&mut self) {
        self.consecutive_idle_cycles = 0;
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
    }

    pub fn record_recovery(&mut self) {
        self.recovery_count += 1;
    }
}

impl Default for ControllerProgress {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_loop_stage_properties() {
        assert_eq!(LoopStage::Observe.name(), "observe");
        assert!(LoopStage::Observe.is_first());
        assert!(!LoopStage::Observe.is_last());

        assert_eq!(LoopStage::Checkpoint.name(), "checkpoint");
        assert!(LoopStage::Checkpoint.is_last());
        assert!(!LoopStage::Checkpoint.is_first());

        assert_eq!(
            format!("{}", LoopStage::IdentifyReadyWork),
            "identify_ready_work"
        );
    }

    #[test]
    fn test_controller_progress_lifecycle() {
        let mut progress = ControllerProgress::new();
        assert_eq!(progress.cycle, 0);
        assert_eq!(progress.current_stage, LoopStage::Observe);
        assert_eq!(progress.consecutive_idle_cycles, 0);
        assert_eq!(progress.failure_count, 0);
        assert_eq!(progress.recovery_count, 0);

        progress.advance_stage(LoopStage::IdentifyReadyWork);
        assert_eq!(progress.current_stage, LoopStage::IdentifyReadyWork);

        progress.record_idle();
        progress.record_idle();
        assert_eq!(progress.consecutive_idle_cycles, 2);
        progress.reset_idle();
        assert_eq!(progress.consecutive_idle_cycles, 0);

        progress.record_failure();
        assert_eq!(progress.failure_count, 1);

        progress.record_recovery();
        assert_eq!(progress.recovery_count, 1);

        progress.increment_cycle();
        assert_eq!(progress.cycle, 1);
        assert_eq!(progress.current_stage, LoopStage::Observe);
    }

    #[test]
    fn test_loop_stage_first_last_and_all_names() {
        // Arrange & Assert: Check all 12 stages
        let stages = [
            (LoopStage::Observe, "observe", true, false),
            (
                LoopStage::IdentifyReadyWork,
                "identify_ready_work",
                false,
                false,
            ),
            (
                LoopStage::ValidatePolicyAndResources,
                "validate_policy_and_resources",
                false,
                false,
            ),
            (LoopStage::AllocateWorkers, "allocate_workers", false, false),
            (LoopStage::CompileContext, "compile_context", false, false),
            (
                LoopStage::ExecuteBoundedWork,
                "execute_bounded_work",
                false,
                false,
            ),
            (LoopStage::CollectResult, "collect_result", false, false),
            (LoopStage::UpdateState, "update_state", false, false),
            (LoopStage::Verify, "verify", false, false),
            (LoopStage::ClassifyFailure, "classify_failure", false, false),
            (
                LoopStage::RecoverOrReplan,
                "recover_or_replan",
                false,
                false,
            ),
            (LoopStage::Checkpoint, "checkpoint", false, true),
        ];

        for (stage, expected_name, is_first, is_last) in stages {
            // Act & Assert
            assert_eq!(stage.name(), expected_name);
            assert_eq!(stage.to_string(), expected_name);
            assert_eq!(stage.is_first(), is_first);
            assert_eq!(stage.is_last(), is_last);
        }
    }

    #[test]
    fn test_controller_progress_serde_roundtrip_complete() {
        // Arrange
        let mut progress = ControllerProgress::new();
        progress.cycle = 5;
        progress.advance_stage(LoopStage::ExecuteBoundedWork);
        progress.record_failure();
        progress.record_failure();
        progress.record_recovery();
        progress.record_idle();

        // Act
        let serialized = serde_json::to_string(&progress).unwrap();
        let deserialized: ControllerProgress = serde_json::from_str(&serialized).unwrap();

        // Assert
        assert_eq!(progress, deserialized);
        assert_eq!(deserialized.cycle, 5);
        assert_eq!(deserialized.current_stage, LoopStage::ExecuteBoundedWork);
        assert_eq!(deserialized.failure_count, 2);
        assert_eq!(deserialized.recovery_count, 1);
        assert_eq!(deserialized.consecutive_idle_cycles, 1);
    }
}
