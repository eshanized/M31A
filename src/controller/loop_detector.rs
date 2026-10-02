use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::controller::progress::LoopStage;
use crate::ids::TaskId;

/// Semantic progress fingerprint preventing infinite oscillating retries (D-11).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LoopSignature {
    pub task_id: Option<TaskId>,
    pub failure_class: String,
    pub recovery_strategy: String,
    pub stage: LoopStage,
    pub progress_fingerprint: u64,
}

/// Bounded sliding-window detector for semantic execution loops (D-11, Edge 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopDetector {
    window: VecDeque<LoopSignature>,
    capacity: usize,
    repetition_threshold: usize,
}

impl LoopDetector {
    pub fn new(capacity: usize, repetition_threshold: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
            repetition_threshold,
        }
    }

    /// Records a loop signature in the sliding window and returns true if repetition threshold is reached.
    pub fn record(&mut self, signature: LoopSignature) -> bool {
        if self.window.len() >= self.capacity {
            self.window.pop_front();
        }

        self.window.push_back(signature.clone());

        let count = self
            .window
            .iter()
            .filter(|s| {
                (s.task_id == signature.task_id
                    || s.task_id.is_none()
                    || signature.task_id.is_none())
                    && s.progress_fingerprint == signature.progress_fingerprint
                    && s.failure_class == signature.failure_class
                    && s.recovery_strategy == signature.recovery_strategy
                    && s.stage == signature.stage
            })
            .count();

        count >= self.repetition_threshold
    }

    pub fn clear(&mut self) {
        self.window.clear();
    }

    pub fn signatures(&self) -> &VecDeque<LoopSignature> {
        &self.window
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new(20, 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_detected_at_threshold() {
        let mut detector = LoopDetector::default();
        let task_id = TaskId::new();

        let sig = LoopSignature {
            task_id: Some(task_id),
            failure_class: "SyntaxError".into(),
            recovery_strategy: "Retry".into(),
            stage: LoopStage::ClassifyFailure,
            progress_fingerprint: 100,
        };

        assert!(!detector.record(sig.clone()));
        assert!(!detector.record(sig.clone()));
        // Third repetition of identical failure & progress trips loop detection
        assert!(detector.record(sig));
    }

    #[test]
    fn test_progress_advancement_resets_loop_count() {
        let mut detector = LoopDetector::default();
        let task_id = TaskId::new();

        let sig1 = LoopSignature {
            task_id: Some(task_id),
            failure_class: "SyntaxError".into(),
            recovery_strategy: "Retry".into(),
            stage: LoopStage::ClassifyFailure,
            progress_fingerprint: 100,
        };

        let sig2 = LoopSignature {
            task_id: Some(task_id),
            failure_class: "SyntaxError".into(),
            recovery_strategy: "Retry".into(),
            stage: LoopStage::ClassifyFailure,
            progress_fingerprint: 101, // State advanced!
        };

        assert!(!detector.record(sig1.clone()));
        assert!(!detector.record(sig1));
        // Progress changed, so this does not trigger loop
        assert!(!detector.record(sig2));
    }

    #[test]
    fn test_sliding_window_capacity_bound() {
        let mut detector = LoopDetector::new(5, 3);
        for i in 0..10 {
            detector.record(LoopSignature {
                task_id: None,
                failure_class: format!("Error-{}", i),
                recovery_strategy: "None".into(),
                stage: LoopStage::Observe,
                progress_fingerprint: i as u64,
            });
        }
        assert_eq!(detector.signatures().len(), 5);
    }

    #[test]
    fn test_loop_detector_clear_and_eviction_semantics() {
        // Arrange
        let mut detector = LoopDetector::new(3, 2);
        let sig = LoopSignature {
            task_id: None,
            failure_class: "NetworkError".into(),
            recovery_strategy: "Backoff".into(),
            stage: LoopStage::ExecuteBoundedWork,
            progress_fingerprint: 42,
        };

        // Act & Assert: 1st record -> false
        assert!(!detector.record(sig.clone()));
        assert_eq!(detector.signatures().len(), 1);

        // Clear resets the window
        detector.clear();
        assert_eq!(detector.signatures().len(), 0);

        // Recording again starts fresh (does not trip at 1st record)
        assert!(!detector.record(sig.clone()));
        // 2nd record reaches threshold 2 -> trips!
        assert!(detector.record(sig));
    }
}
