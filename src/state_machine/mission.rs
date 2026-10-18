//! Mission lifecycle transitions using the canonical execution lifecycle state model.
//!
//! Mission retains strict staged progression. Direct-execution shortcuts are
//! enabled only for ExecutionRun, while the shared state/event definitions live
//! in the canonical execution_run module.

pub use crate::state_machine::execution_run::{
    ExecutionRunEvent as MissionEvent, ExecutionRunState as MissionState,
};

/// Validate and execute a strict mission lifecycle transition.
///
/// This preserves the staged Mission workflow while reusing the canonical
/// state and event definitions. Direct-execution shortcuts remain exclusive
/// to `transition_execution_run`.
pub fn transition_mission(
    current: MissionState,
    event: MissionEvent,
) -> Result<MissionState, crate::state_machine::error::TransitionError> {
    crate::state_machine::execution_run::transition_execution_run_with_shortcuts(
        current, event, false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_forward_transitions() {
        assert_eq!(
            transition_mission(MissionState::Created, MissionEvent::Start),
            Ok(MissionState::Understanding)
        );
        assert_eq!(
            transition_mission(
                MissionState::Understanding,
                MissionEvent::UnderstandComplete
            ),
            Ok(MissionState::Researching)
        );
        assert_eq!(
            transition_mission(MissionState::Researching, MissionEvent::ResearchComplete),
            Ok(MissionState::Planning)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::PlanComplete),
            Ok(MissionState::Scheduled)
        );
        assert_eq!(
            transition_mission(MissionState::Scheduled, MissionEvent::Execute),
            Ok(MissionState::Executing)
        );
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Verify),
            Ok(MissionState::Verifying)
        );
        assert_eq!(
            transition_mission(MissionState::Verifying, MissionEvent::Review),
            Ok(MissionState::Reviewing)
        );
        assert_eq!(
            transition_mission(MissionState::Reviewing, MissionEvent::Integrate),
            Ok(MissionState::Integrating)
        );
        assert_eq!(
            transition_mission(MissionState::Integrating, MissionEvent::Ship),
            Ok(MissionState::Shipping)
        );
        assert_eq!(
            transition_mission(MissionState::Shipping, MissionEvent::Complete),
            Ok(MissionState::Completed)
        );
    }

    #[test]
    fn test_pause_resume() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Pause),
            Ok(MissionState::Paused)
        );
        assert_eq!(
            transition_mission(MissionState::Paused, MissionEvent::Resume),
            Ok(MissionState::Executing)
        );
    }

    #[test]
    fn test_block_unblock() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Block),
            Ok(MissionState::Blocked)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::Block),
            Ok(MissionState::Blocked)
        );
        assert_eq!(
            transition_mission(MissionState::Blocked, MissionEvent::Unblock),
            Ok(MissionState::Executing)
        );
    }

    #[test]
    fn test_fail_from_non_terminal() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Fail),
            Ok(MissionState::Failed)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::Fail),
            Ok(MissionState::Failed)
        );
        assert_eq!(
            transition_mission(MissionState::Created, MissionEvent::Fail),
            Ok(MissionState::Failed)
        );
    }

    #[test]
    fn test_cancel_from_non_terminal() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Cancel),
            Ok(MissionState::Cancelled)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::Cancel),
            Ok(MissionState::Cancelled)
        );
        assert_eq!(
            transition_mission(MissionState::Created, MissionEvent::Cancel),
            Ok(MissionState::Cancelled)
        );
    }

    #[test]
    fn test_terminal_states_reject_transitions() {
        let terminal_states = [
            MissionState::Completed,
            MissionState::Failed,
            MissionState::Cancelled,
        ];
        for state in terminal_states {
            assert!(transition_mission(state, MissionEvent::Start).is_err());
            assert!(transition_mission(state, MissionEvent::Fail).is_err());
            assert!(transition_mission(state, MissionEvent::Cancel).is_err());
        }
    }

    #[test]
    fn test_invalid_transitions_rejected() {
        // Can't skip steps
        assert!(
            transition_mission(MissionState::Created, MissionEvent::UnderstandComplete).is_err()
        );
        assert!(transition_mission(MissionState::Created, MissionEvent::Execute).is_err());
        assert!(transition_mission(MissionState::Understanding, MissionEvent::Execute).is_err());

        // Can't go backwards (except Unblock)
        assert!(
            transition_mission(MissionState::Executing, MissionEvent::UnderstandComplete).is_err()
        );
    }

    #[test]
    fn mission_rejects_execution_only_shortcuts() {
        assert!(transition_mission(MissionState::Created, MissionEvent::Schedule).is_err());
        assert!(transition_mission(MissionState::Created, MissionEvent::Execute).is_err());
        assert!(transition_mission(MissionState::Executing, MissionEvent::Complete).is_err());
        assert!(transition_mission(MissionState::Planning, MissionEvent::Schedule).is_err());
    }

    #[test]
    fn test_is_terminal() {
        assert!(MissionState::Completed.is_terminal());
        assert!(MissionState::Failed.is_terminal());
        assert!(MissionState::Cancelled.is_terminal());
        assert!(!MissionState::Executing.is_terminal());
        assert!(!MissionState::Paused.is_terminal());
        assert!(!MissionState::Blocked.is_terminal());
    }
}
