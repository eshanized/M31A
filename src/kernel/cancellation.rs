//! Cancellation token hierarchy (KRN-04)
//!
//! Per KRN-04 and RESEARCH.md Pattern 3, parent-child cancellation tokens
//! for ownership chain propagation using tokio-util's CancellationToken.

use tokio_util::sync::CancellationToken;

/// MissionRuntime wraps a root CancellationToken and provides
/// child token spawning for the ownership chain.
#[derive(Debug, Clone)]
pub struct MissionRuntime {
    cancellation_token: CancellationToken,
}

impl MissionRuntime {
    /// Create a new MissionRuntime with a root cancellation token.
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Get a reference to the root cancellation token.
    pub fn token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Spawn a child cancellation token.
    /// The child token is cancelled when the parent is cancelled.
    /// Cancelling the child does NOT cancel the parent.
    pub fn spawn_child(&self) -> CancellationToken {
        self.cancellation_token.child_token()
    }

    /// Check if the root token is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Shutdown the runtime by cancelling the root token.
    /// This cancels all child tokens recursively.
    pub fn shutdown(&self) {
        self.cancellation_token.cancel();
    }
}

impl Default for MissionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_mission_runtime_new() {
        let runtime = MissionRuntime::new();
        assert!(!runtime.is_cancelled());
    }

    #[test]
    fn test_spawn_child() {
        let runtime = MissionRuntime::new();
        let child = runtime.spawn_child();
        assert!(!child.is_cancelled());
        assert!(!runtime.is_cancelled());
    }

    #[test]
    fn test_parent_cancels_children() {
        let runtime = MissionRuntime::new();
        let child1 = runtime.spawn_child();
        let child2 = runtime.spawn_child();
        let grandchild = child1.child_token();

        assert!(!runtime.is_cancelled());
        assert!(!child1.is_cancelled());
        assert!(!child2.is_cancelled());
        assert!(!grandchild.is_cancelled());

        runtime.shutdown();

        assert!(runtime.is_cancelled());
        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
        assert!(grandchild.is_cancelled());
    }

    #[test]
    fn test_child_cancellation_isolated() {
        let runtime = MissionRuntime::new();
        let child = runtime.spawn_child();

        child.cancel();

        assert!(child.is_cancelled());
        assert!(!runtime.is_cancelled()); // Parent should NOT be cancelled
    }

    #[test]
    fn test_is_cancelled_observable() {
        let runtime = MissionRuntime::new();
        assert!(!runtime.is_cancelled());

        runtime.shutdown();
        assert!(runtime.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_propagation_in_select() {
        let runtime = MissionRuntime::new();
        let child = runtime.spawn_child();
        let child_clone = child.clone();

        // Spawn a task that waits for cancellation
        let handle = tokio::spawn(async move {
            child_clone.cancelled().await;
            "cancelled"
        });

        // Give it a moment to start waiting
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Cancel parent - should propagate to child
        runtime.shutdown();

        // The task should complete due to cancellation
        let result = timeout(Duration::from_secs(1), handle).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap(), "cancelled");
    }

    #[tokio::test]
    async fn test_multiple_levels_of_children() {
        let runtime = MissionRuntime::new();
        let child1 = runtime.spawn_child();
        let child2 = child1.child_token();
        let child3 = child2.child_token();

        assert!(!runtime.is_cancelled());
        assert!(!child1.is_cancelled());
        assert!(!child2.is_cancelled());
        assert!(!child3.is_cancelled());

        runtime.shutdown();

        assert!(runtime.is_cancelled());
        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
        assert!(child3.is_cancelled());
    }

    #[tokio::test]
    async fn test_shutdown_cancels_all_descendants() {
        let runtime = MissionRuntime::new();
        let children: Vec<_> = (0..10).map(|_| runtime.spawn_child()).collect();
        let grandchildren: Vec<_> = children.iter().map(|c| c.child_token()).collect();

        runtime.shutdown();

        for child in &children {
            assert!(child.is_cancelled());
        }
        for grandchild in &grandchildren {
            assert!(grandchild.is_cancelled());
        }
    }
}
