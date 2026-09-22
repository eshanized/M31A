use crate::controller::error::ControllerError;
use crate::controller::halting::ControllerHaltReason;
use crate::controller::progress::LoopStage;
use std::time::Duration;

/// Result of executing a single stage within the Autonomy Controller loop (D-03).
#[derive(Debug, Clone, PartialEq)]
pub enum StageOutcome {
    Advance(LoopStage),
    SkipTo(LoopStage),
    Yield(Option<Duration>),
    Halt(ControllerHaltReason),
}

impl StageOutcome {
    pub fn advance(stage: LoopStage) -> Self {
        Self::Advance(stage)
    }

    pub fn skip_to(stage: LoopStage) -> Self {
        Self::SkipTo(stage)
    }

    pub fn yield_now() -> Self {
        Self::Yield(None)
    }

    pub fn yield_for(d: Duration) -> Self {
        Self::Yield(Some(d))
    }

    pub fn halt(reason: ControllerHaltReason) -> Self {
        Self::Halt(reason)
    }

    /// Validates whether transitioning from `from` with `outcome` is permitted by the 12-stage legal transition graph (D-03).
    pub fn validate_transition(
        from: LoopStage,
        outcome: &StageOutcome,
    ) -> Result<(), ControllerError> {
        let is_valid = match (from, outcome) {
            // Observe -> Advance(IdentifyReadyWork), Halt(_)
            (LoopStage::Observe, StageOutcome::Advance(LoopStage::IdentifyReadyWork)) => true,
            (LoopStage::Observe, StageOutcome::Halt(_)) => true,

            // IdentifyReadyWork -> Advance(ValidatePolicyAndResources), SkipTo(Verify), SkipTo(Checkpoint), Yield(_), Halt(_)
            (
                LoopStage::IdentifyReadyWork,
                StageOutcome::Advance(LoopStage::ValidatePolicyAndResources),
            ) => true,
            (LoopStage::IdentifyReadyWork, StageOutcome::SkipTo(LoopStage::Verify)) => true,
            (LoopStage::IdentifyReadyWork, StageOutcome::SkipTo(LoopStage::Checkpoint)) => true,
            (LoopStage::IdentifyReadyWork, StageOutcome::Yield(_)) => true,
            (LoopStage::IdentifyReadyWork, StageOutcome::Halt(_)) => true,

            // ValidatePolicyAndResources -> Advance(AllocateWorkers), SkipTo(ClassifyFailure), Halt(_)
            (
                LoopStage::ValidatePolicyAndResources,
                StageOutcome::Advance(LoopStage::AllocateWorkers),
            ) => true,
            (
                LoopStage::ValidatePolicyAndResources,
                StageOutcome::SkipTo(LoopStage::ClassifyFailure),
            ) => true,
            (LoopStage::ValidatePolicyAndResources, StageOutcome::Halt(_)) => true,

            // AllocateWorkers -> Advance(CompileContext), Yield(_)
            (LoopStage::AllocateWorkers, StageOutcome::Advance(LoopStage::CompileContext)) => true,
            (LoopStage::AllocateWorkers, StageOutcome::Yield(_)) => true,

            // CompileContext -> Advance(ExecuteBoundedWork), SkipTo(ClassifyFailure)
            (LoopStage::CompileContext, StageOutcome::Advance(LoopStage::ExecuteBoundedWork)) => {
                true
            }
            (LoopStage::CompileContext, StageOutcome::SkipTo(LoopStage::ClassifyFailure)) => true,

            // ExecuteBoundedWork -> Advance(CollectResult)
            (LoopStage::ExecuteBoundedWork, StageOutcome::Advance(LoopStage::CollectResult)) => {
                true
            }

            // CollectResult -> Advance(UpdateState)
            (LoopStage::CollectResult, StageOutcome::Advance(LoopStage::UpdateState)) => true,

            // UpdateState -> Advance(Verify), SkipTo(ClassifyFailure)
            (LoopStage::UpdateState, StageOutcome::Advance(LoopStage::Verify)) => true,
            (LoopStage::UpdateState, StageOutcome::SkipTo(LoopStage::ClassifyFailure)) => true,

            // Verify -> SkipTo(Checkpoint), Advance(ClassifyFailure), Halt(_)
            (LoopStage::Verify, StageOutcome::SkipTo(LoopStage::Checkpoint)) => true,
            (LoopStage::Verify, StageOutcome::Advance(LoopStage::ClassifyFailure)) => true,
            (LoopStage::Verify, StageOutcome::Halt(_)) => true,

            // ClassifyFailure -> Advance(RecoverOrReplan), Halt(_)
            (LoopStage::ClassifyFailure, StageOutcome::Advance(LoopStage::RecoverOrReplan)) => true,
            (LoopStage::ClassifyFailure, StageOutcome::Halt(_)) => true,

            // RecoverOrReplan -> Advance(Checkpoint), SkipTo(Observe), SkipTo(Verify), SkipTo(ClassifyFailure), Halt(_)
            (LoopStage::RecoverOrReplan, StageOutcome::Advance(LoopStage::Checkpoint)) => true,
            (LoopStage::RecoverOrReplan, StageOutcome::SkipTo(LoopStage::Observe)) => true,
            (LoopStage::RecoverOrReplan, StageOutcome::SkipTo(LoopStage::Verify)) => true,
            (LoopStage::RecoverOrReplan, StageOutcome::SkipTo(LoopStage::ClassifyFailure)) => true,
            (LoopStage::RecoverOrReplan, StageOutcome::Halt(_)) => true,

            // Checkpoint -> Advance(Observe), Halt(_)
            (LoopStage::Checkpoint, StageOutcome::Advance(LoopStage::Observe)) => true,
            (LoopStage::Checkpoint, StageOutcome::Halt(_)) => true,

            // Any undeclared edge is illegal
            _ => false,
        };

        if is_valid {
            Ok(())
        } else {
            Err(ControllerError::InvalidStageTransition {
                from,
                attempted: format!("{outcome:?}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let valid_cases: Vec<(LoopStage, StageOutcome)> = vec![
            (
                LoopStage::Observe,
                StageOutcome::advance(LoopStage::IdentifyReadyWork),
            ),
            (
                LoopStage::Observe,
                StageOutcome::halt(ControllerHaltReason::Cancelled),
            ),
            (
                LoopStage::IdentifyReadyWork,
                StageOutcome::advance(LoopStage::ValidatePolicyAndResources),
            ),
            (
                LoopStage::IdentifyReadyWork,
                StageOutcome::skip_to(LoopStage::Verify),
            ),
            (
                LoopStage::IdentifyReadyWork,
                StageOutcome::skip_to(LoopStage::Checkpoint),
            ),
            (LoopStage::IdentifyReadyWork, StageOutcome::yield_now()),
            (
                LoopStage::IdentifyReadyWork,
                StageOutcome::halt(ControllerHaltReason::Blocked),
            ),
            (
                LoopStage::ValidatePolicyAndResources,
                StageOutcome::advance(LoopStage::AllocateWorkers),
            ),
            (
                LoopStage::ValidatePolicyAndResources,
                StageOutcome::skip_to(LoopStage::ClassifyFailure),
            ),
            (
                LoopStage::ValidatePolicyAndResources,
                StageOutcome::halt(ControllerHaltReason::BudgetExhausted {
                    kind: crate::controller::budget_tracker::BudgetKind::Tokens,
                }),
            ),
            (
                LoopStage::AllocateWorkers,
                StageOutcome::advance(LoopStage::CompileContext),
            ),
            (
                LoopStage::AllocateWorkers,
                StageOutcome::yield_for(Duration::from_millis(100)),
            ),
            (
                LoopStage::CompileContext,
                StageOutcome::advance(LoopStage::ExecuteBoundedWork),
            ),
            (
                LoopStage::CompileContext,
                StageOutcome::skip_to(LoopStage::ClassifyFailure),
            ),
            (
                LoopStage::ExecuteBoundedWork,
                StageOutcome::advance(LoopStage::CollectResult),
            ),
            (
                LoopStage::CollectResult,
                StageOutcome::advance(LoopStage::UpdateState),
            ),
            (
                LoopStage::UpdateState,
                StageOutcome::advance(LoopStage::Verify),
            ),
            (
                LoopStage::UpdateState,
                StageOutcome::skip_to(LoopStage::ClassifyFailure),
            ),
            (
                LoopStage::Verify,
                StageOutcome::skip_to(LoopStage::Checkpoint),
            ),
            (
                LoopStage::Verify,
                StageOutcome::advance(LoopStage::ClassifyFailure),
            ),
            (
                LoopStage::Verify,
                StageOutcome::halt(ControllerHaltReason::MissionCompleted),
            ),
            (
                LoopStage::Verify,
                StageOutcome::halt(ControllerHaltReason::Cancelled),
            ),
            (
                LoopStage::ClassifyFailure,
                StageOutcome::advance(LoopStage::RecoverOrReplan),
            ),
            (
                LoopStage::ClassifyFailure,
                StageOutcome::halt(ControllerHaltReason::FatalError {
                    failure_class: "panic".into(),
                }),
            ),
            (
                LoopStage::RecoverOrReplan,
                StageOutcome::advance(LoopStage::Checkpoint),
            ),
            (
                LoopStage::RecoverOrReplan,
                StageOutcome::skip_to(LoopStage::Observe),
            ),
            (
                LoopStage::RecoverOrReplan,
                StageOutcome::halt(ControllerHaltReason::MissionCompleted),
            ),
            (
                LoopStage::Checkpoint,
                StageOutcome::advance(LoopStage::Observe),
            ),
            (
                LoopStage::Checkpoint,
                StageOutcome::halt(ControllerHaltReason::MissionCompleted),
            ),
        ];

        for (from, outcome) in valid_cases {
            assert!(
                StageOutcome::validate_transition(from, &outcome).is_ok(),
                "Expected valid transition from {from:?} with outcome {outcome:?}"
            );
        }
    }

    #[test]
    fn test_invalid_transitions() {
        let invalid_cases: Vec<(LoopStage, StageOutcome)> = vec![
            // Observe cannot jump directly to execution or checkpoint
            (
                LoopStage::Observe,
                StageOutcome::advance(LoopStage::ExecuteBoundedWork),
            ),
            (
                LoopStage::Observe,
                StageOutcome::skip_to(LoopStage::Checkpoint),
            ),
            (LoopStage::Observe, StageOutcome::yield_now()),
            // ExecuteBoundedWork can ONLY advance to CollectResult
            (
                LoopStage::ExecuteBoundedWork,
                StageOutcome::advance(LoopStage::UpdateState),
            ),
            (
                LoopStage::ExecuteBoundedWork,
                StageOutcome::skip_to(LoopStage::Checkpoint),
            ),
            (
                LoopStage::ExecuteBoundedWork,
                StageOutcome::halt(ControllerHaltReason::Cancelled),
            ),
            // AllocateWorkers cannot halt directly (must yield or advance)
            (
                LoopStage::AllocateWorkers,
                StageOutcome::halt(ControllerHaltReason::Blocked),
            ),
            // Checkpoint cannot jump to ValidatePolicyAndResources
            (
                LoopStage::Checkpoint,
                StageOutcome::advance(LoopStage::ValidatePolicyAndResources),
            ),
            (
                LoopStage::Checkpoint,
                StageOutcome::skip_to(LoopStage::ExecuteBoundedWork),
            ),
        ];

        for (from, outcome) in invalid_cases {
            let res = StageOutcome::validate_transition(from, &outcome);
            assert!(
                res.is_err(),
                "Expected invalid transition from {from:?} with outcome {outcome:?}"
            );
            if let Err(ControllerError::InvalidStageTransition { from: f, attempted }) = res {
                assert_eq!(f, from);
                assert_eq!(attempted, format!("{outcome:?}"));
            } else {
                panic!("Unexpected error type: {res:?}");
            }
        }
    }

    #[test]
    fn test_stage_outcome_helper_constructors() {
        // Arrange & Act
        let adv = StageOutcome::advance(LoopStage::Observe);
        let skip = StageOutcome::skip_to(LoopStage::Checkpoint);
        let y_now = StageOutcome::yield_now();
        let y_for = StageOutcome::yield_for(Duration::from_secs(5));
        let h = StageOutcome::halt(ControllerHaltReason::Cancelled);

        // Assert
        assert_eq!(adv, StageOutcome::Advance(LoopStage::Observe));
        assert_eq!(skip, StageOutcome::SkipTo(LoopStage::Checkpoint));
        assert_eq!(y_now, StageOutcome::Yield(None));
        assert_eq!(y_for, StageOutcome::Yield(Some(Duration::from_secs(5))));
        assert_eq!(h, StageOutcome::Halt(ControllerHaltReason::Cancelled));
    }

    #[test]
    fn test_validate_transition_exhaustive_rejects_illegal_leaps() {
        // Test backwards leaps that are not explicitly declared
        let illegal_leaps = [
            (
                LoopStage::ValidatePolicyAndResources,
                StageOutcome::advance(LoopStage::Observe),
            ),
            (
                LoopStage::CompileContext,
                StageOutcome::skip_to(LoopStage::Observe),
            ),
            (
                LoopStage::CollectResult,
                StageOutcome::advance(LoopStage::IdentifyReadyWork),
            ),
            (
                LoopStage::Verify,
                StageOutcome::advance(LoopStage::ExecuteBoundedWork),
            ),
            (
                LoopStage::Checkpoint,
                StageOutcome::advance(LoopStage::CompileContext),
            ),
            (
                LoopStage::IdentifyReadyWork,
                StageOutcome::advance(LoopStage::ExecuteBoundedWork),
            ),
        ];

        for (from, outcome) in illegal_leaps {
            let res = StageOutcome::validate_transition(from, &outcome);
            assert!(
                res.is_err(),
                "Expected illegal leap from {from:?} with outcome {outcome:?} to fail"
            );
        }
    }
}
