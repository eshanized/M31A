//! Tracks cumulative resource consumption and enforces 10-dimensional layered bounds (BST-01, BST-02, D-05).

use crate::state::budget::ResourceBudget;
use serde::{Deserialize, Serialize};

// BudgetKind is defined in the budget domain and re-exported here for controller use (AD-007).
pub use crate::budget::kind::BudgetKind;

/// Tracks cumulative resource consumption and enforces layered bounds across 10 dimensions (D-12, BST-01, BST-02).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetTracker {
    pub wall_clock_seconds_consumed: u64,
    pub concurrent_agents_active: usize,
    pub agent_steps_consumed: usize,
    pub model_calls_consumed: usize,
    pub tokens_consumed: u64,
    pub cost_usd_consumed: f64,
    pub cpu_seconds_consumed: u64,
    pub memory_bytes_consumed: u64,
    pub artifact_bytes_consumed: u64,
    pub retries_consumed: usize,
}

impl BudgetTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_elapsed_seconds(&mut self, seconds: u64) {
        self.wall_clock_seconds_consumed = self.wall_clock_seconds_consumed.saturating_add(seconds);
    }

    pub fn record_cpu_seconds(&mut self, seconds: u64) {
        self.cpu_seconds_consumed = self.cpu_seconds_consumed.saturating_add(seconds);
    }

    pub fn record_memory_bytes(&mut self, bytes: u64) {
        self.memory_bytes_consumed = self.memory_bytes_consumed.max(bytes);
    }

    pub fn record_artifact_bytes(&mut self, bytes: u64) {
        self.artifact_bytes_consumed = self.artifact_bytes_consumed.saturating_add(bytes);
    }

    /// Pre-flight admission check for wall-clock deadline.
    pub fn check_preflight_wall_clock(&self, limits: &ResourceBudget) -> Result<(), BudgetKind> {
        if limits
            .max_wall_clock_seconds
            .is_some_and(|max| self.wall_clock_seconds_consumed >= max)
        {
            return Err(BudgetKind::WallClock);
        }
        Ok(())
    }

    /// Pre-admission check for task token estimation.
    pub fn check_admission_tokens(
        &self,
        estimated_tokens: u64,
        limits: &ResourceBudget,
    ) -> Result<(), BudgetKind> {
        if limits
            .max_tokens
            .is_some_and(|max| self.tokens_consumed.saturating_add(estimated_tokens) > max)
        {
            return Err(BudgetKind::Tokens);
        }
        Ok(())
    }

    /// Atomic reservation of a concurrent worker.
    pub fn try_reserve_worker(&mut self, limits: &ResourceBudget) -> Result<(), BudgetKind> {
        if limits
            .max_concurrent_agents
            .is_some_and(|max| self.concurrent_agents_active >= max)
        {
            return Err(BudgetKind::ConcurrentAgents);
        }
        self.concurrent_agents_active += 1;
        Ok(())
    }

    /// Releases a previously reserved concurrent worker.
    pub fn release_worker(&mut self) {
        self.concurrent_agents_active = self.concurrent_agents_active.saturating_sub(1);
    }

    /// Post-execution reconciliation adding delta metrics and verifying all 10 bounds (BST-02).
    pub fn reconcile_consumption(
        &mut self,
        steps: usize,
        calls: usize,
        tokens: u64,
        cost: f64,
        retries: usize,
        limits: &ResourceBudget,
    ) -> Result<(), BudgetKind> {
        self.reconcile_full_consumption(steps, calls, tokens, cost, 0, 0, 0, retries, limits)
    }

    /// Comprehensive post-execution reconciliation across all 10 dimensions.
    #[allow(clippy::too_many_arguments)]
    pub fn reconcile_full_consumption(
        &mut self,
        steps: usize,
        calls: usize,
        tokens: u64,
        cost: f64,
        cpu_seconds: u64,
        memory_bytes: u64,
        artifact_bytes: u64,
        retries: usize,
        limits: &ResourceBudget,
    ) -> Result<(), BudgetKind> {
        self.agent_steps_consumed = self.agent_steps_consumed.saturating_add(steps);
        self.model_calls_consumed = self.model_calls_consumed.saturating_add(calls);
        self.tokens_consumed = self.tokens_consumed.saturating_add(tokens);
        self.cost_usd_consumed += cost;
        self.cpu_seconds_consumed = self.cpu_seconds_consumed.saturating_add(cpu_seconds);
        self.memory_bytes_consumed = self.memory_bytes_consumed.max(memory_bytes);
        self.artifact_bytes_consumed = self.artifact_bytes_consumed.saturating_add(artifact_bytes);
        self.retries_consumed = self.retries_consumed.saturating_add(retries);

        if limits
            .max_wall_clock_seconds
            .is_some_and(|max| self.wall_clock_seconds_consumed >= max)
        {
            return Err(BudgetKind::WallClock);
        }
        if limits
            .max_agent_steps
            .is_some_and(|max| self.agent_steps_consumed > max)
        {
            return Err(BudgetKind::AgentSteps);
        }
        if limits
            .max_model_calls
            .is_some_and(|max| self.model_calls_consumed > max)
        {
            return Err(BudgetKind::ModelCalls);
        }
        if limits
            .max_tokens
            .is_some_and(|max| self.tokens_consumed > max)
        {
            return Err(BudgetKind::Tokens);
        }
        if limits
            .max_cost_usd
            .is_some_and(|max| self.cost_usd_consumed > max)
        {
            return Err(BudgetKind::CostUsd);
        }
        if limits
            .max_cpu_seconds
            .is_some_and(|max| self.cpu_seconds_consumed > max)
        {
            return Err(BudgetKind::CpuSeconds);
        }
        if limits
            .max_memory_bytes
            .is_some_and(|max| self.memory_bytes_consumed > max)
        {
            return Err(BudgetKind::MemoryBytes);
        }
        if limits
            .max_artifact_bytes
            .is_some_and(|max| self.artifact_bytes_consumed > max)
        {
            return Err(BudgetKind::ArtifactBytes);
        }
        if limits
            .max_retries
            .is_some_and(|max| self.retries_consumed > max)
        {
            return Err(BudgetKind::Retries);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preflight_wall_clock_check() {
        let mut tracker = BudgetTracker::default();
        let limits = ResourceBudget {
            max_wall_clock_seconds: Some(100),
            ..Default::default()
        };

        assert!(tracker.check_preflight_wall_clock(&limits).is_ok());

        tracker.record_elapsed_seconds(100);
        assert_eq!(
            tracker.check_preflight_wall_clock(&limits),
            Err(BudgetKind::WallClock)
        );
    }

    #[test]
    fn test_worker_reservation_and_release() {
        let mut tracker = BudgetTracker::default();
        let limits = ResourceBudget {
            max_concurrent_agents: Some(2),
            ..Default::default()
        };

        assert!(tracker.try_reserve_worker(&limits).is_ok());
        assert_eq!(tracker.concurrent_agents_active, 1);
        assert!(tracker.try_reserve_worker(&limits).is_ok());
        assert_eq!(tracker.concurrent_agents_active, 2);

        // At capacity, next attempt fails
        assert_eq!(
            tracker.try_reserve_worker(&limits),
            Err(BudgetKind::ConcurrentAgents)
        );

        tracker.release_worker();
        assert_eq!(tracker.concurrent_agents_active, 1);
        assert!(tracker.try_reserve_worker(&limits).is_ok());
    }

    #[test]
    fn test_reconcile_consumption_exhaustion() {
        let mut tracker = BudgetTracker::default();
        let limits = ResourceBudget {
            max_tokens: Some(1000),
            max_cost_usd: Some(0.05),
            ..Default::default()
        };

        assert!(
            tracker
                .reconcile_consumption(1, 1, 500, 0.02, 0, &limits)
                .is_ok()
        );
        assert_eq!(tracker.tokens_consumed, 500);

        // Exceed tokens limit
        let res = tracker.reconcile_consumption(1, 1, 600, 0.01, 0, &limits);
        assert_eq!(res, Err(BudgetKind::Tokens));
        assert_eq!(tracker.tokens_consumed, 1100);
    }

    #[test]
    fn test_budget_tracker_all_kinds_exhaustion() {
        // Test CpuSeconds exhaustion
        let mut tracker_cpu = BudgetTracker::default();
        let limits_cpu = ResourceBudget {
            max_cpu_seconds: Some(10),
            ..Default::default()
        };
        assert_eq!(
            tracker_cpu.reconcile_full_consumption(0, 0, 0, 0.0, 15, 0, 0, 0, &limits_cpu),
            Err(BudgetKind::CpuSeconds)
        );

        // Test MemoryBytes exhaustion
        let mut tracker_mem = BudgetTracker::default();
        let limits_mem = ResourceBudget {
            max_memory_bytes: Some(1024),
            ..Default::default()
        };
        assert_eq!(
            tracker_mem.reconcile_full_consumption(0, 0, 0, 0.0, 0, 2048, 0, 0, &limits_mem),
            Err(BudgetKind::MemoryBytes)
        );

        // Test ArtifactBytes exhaustion
        let mut tracker_art = BudgetTracker::default();
        let limits_art = ResourceBudget {
            max_artifact_bytes: Some(5000),
            ..Default::default()
        };
        assert_eq!(
            tracker_art.reconcile_full_consumption(0, 0, 0, 0.0, 0, 0, 6000, 0, &limits_art),
            Err(BudgetKind::ArtifactBytes)
        );
    }
}
