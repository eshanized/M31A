use crate::controller::progress::LoopStage;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ControllerError {
    #[error("invalid stage transition from {from:?} to {attempted}")]
    InvalidStageTransition { from: LoopStage, attempted: String },

    #[error("seam error in {seam}: {message}")]
    SeamError { seam: String, message: String },

    #[error("budget exceeded: {detail}")]
    BudgetExceeded { detail: String },

    #[error("invariant violation: {detail}")]
    InvariantViolation { detail: String },

    #[error("execution cancelled")]
    Cancelled,

    #[error("internal error: {message}")]
    Internal { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_formatting() {
        let err = ControllerError::InvalidStageTransition {
            from: LoopStage::Observe,
            attempted: "ExecuteBoundedWork".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid stage transition from Observe to ExecuteBoundedWork"
        );

        let err2 = ControllerError::Cancelled;
        assert_eq!(err2.to_string(), "execution cancelled");

        let err3 = ControllerError::SeamError {
            seam: "scheduler".to_string(),
            message: "timeout".to_string(),
        };
        assert_eq!(err3.to_string(), "seam error in scheduler: timeout");
    }
}
