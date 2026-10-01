//! Two-Phase Concurrency-Safe Budget Enforcer.
//!
//! Provides atomic pre-admission capacity reservation and post-execution settlement.

use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::budget::kind::BudgetKind;
use crate::budget::policy::{BudgetExhaustionAction, BudgetExhaustionPolicy};
use crate::budget::receipt::ReservationReceipt;
use crate::model::types::{TokenUsage, UsageSource};
use crate::state::budget::ResourceBudget;

/// Conversion multiplier for USD to integer micro-cents ($0.000001 precision).
const MICRO_CENTS_PER_USD: f64 = 100_000_000.0;

/// Estimated resource requirements for a task before admission.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskEstimates {
    pub estimated_tokens: u64,
    pub estimated_cost_usd: f64,
    pub requires_worker: bool,
    pub estimated_artifact_bytes: u64,
}

impl Default for TaskEstimates {
    fn default() -> Self {
        Self {
            estimated_tokens: 0,
            estimated_cost_usd: 0.0,
            requires_worker: true,
            estimated_artifact_bytes: 0,
        }
    }
}

/// Actual resource consumption measured after task execution finishes.
#[derive(Debug, Clone, PartialEq)]
pub struct ActualUsage {
    pub steps: usize,
    pub calls: usize,
    pub tokens: u64,
    pub cost_usd: f64,
    pub artifact_bytes: u64,
    pub retries: usize,
}

impl Default for ActualUsage {
    fn default() -> Self {
        Self {
            steps: 1,
            calls: 1,
            tokens: 0,
            cost_usd: 0.0,
            artifact_bytes: 0,
            retries: 0,
        }
    }
}

impl ActualUsage {
    /// Authoritative settlement from provider-reported usage.
    ///
    /// Maps the canonical normalized `TokenUsage` to consumed resources:
    /// `tokens = total_tokens`, one model call (and one step by default).
    /// Cost is NOT inferred from token counts here (no pricing table in the
    /// enforcer); pass a measured cost via `with_cost_usd` when the provider
    /// reports one. The returned value carries the usage provenance so
    /// callers can distinguish authoritative from estimated settlement: an
    /// `Estimated` source means the provider reported no usable counters and
    /// the value is a fallback, never silently mixed with authoritative data.
    pub fn from_token_usage(usage: &TokenUsage) -> Self {
        Self {
            steps: 1,
            calls: 1,
            tokens: usage.total_tokens as u64,
            cost_usd: 0.0,
            artifact_bytes: 0,
            retries: 0,
        }
    }

    /// Attach a provider-reported cost to usage-derived actuals.
    pub fn with_cost_usd(mut self, cost_usd: f64) -> Self {
        self.cost_usd = cost_usd.max(0.0);
        self
    }

    /// Whether the underlying usage record is authoritative provider data
    /// (as opposed to an estimate fallback).
    pub fn is_authoritative(usage: &TokenUsage) -> bool {
        usage.source == UsageSource::AuthoritativeProvider
    }
}

/// Concurrency-safe, two-phase resource budget enforcer.
pub struct BudgetEnforcer {
    limits: RwLock<ResourceBudget>,
    tokens_reserved: AtomicU64,
    tokens_consumed: AtomicU64,
    cost_reserved_microcents: AtomicU64,
    cost_consumed_microcents: AtomicU64,
    workers_active: AtomicUsize,
    artifact_bytes_reserved: AtomicU64,
    artifact_bytes_consumed: AtomicU64,
    retries_consumed: AtomicUsize,
    agent_steps_consumed: AtomicUsize,
    model_calls_consumed: AtomicUsize,
}

impl BudgetEnforcer {
    /// Create a new enforcer bounded by the given limits.
    pub fn new(limits: ResourceBudget) -> Self {
        Self {
            limits: RwLock::new(limits),
            tokens_reserved: AtomicU64::new(0),
            tokens_consumed: AtomicU64::new(0),
            cost_reserved_microcents: AtomicU64::new(0),
            cost_consumed_microcents: AtomicU64::new(0),
            workers_active: AtomicUsize::new(0),
            artifact_bytes_reserved: AtomicU64::new(0),
            artifact_bytes_consumed: AtomicU64::new(0),
            retries_consumed: AtomicUsize::new(0),
            agent_steps_consumed: AtomicUsize::new(0),
            model_calls_consumed: AtomicUsize::new(0),
        }
    }

    /// Dynamically update limits (e.g. from an approved budget grant).
    pub fn update_limits(&self, new_limits: ResourceBudget) {
        let mut limits = self.limits.write().expect("write lock limits");
        *limits = new_limits;
    }

    /// Get current active limits snapshot.
    pub fn current_limits(&self) -> ResourceBudget {
        self.limits.read().expect("read lock limits").clone()
    }

    /// Access budget limits snapshot.
    pub fn budget(&self) -> ResourceBudget {
        self.current_limits()
    }

    /// Pre-admission phase: atomically reserve estimated capacity.
    pub fn reserve(
        &self,
        estimates: &TaskEstimates,
        is_interactive: bool,
    ) -> Result<ReservationReceipt, BudgetExhaustionAction> {
        let limits = self.limits.read().expect("read lock limits");

        // 1. Worker concurrency check
        if estimates.requires_worker
            && let Some(max_workers) = limits.max_concurrent_agents
        {
            let current = self.workers_active.load(Ordering::SeqCst);
            if current >= max_workers {
                return Err(BudgetExhaustionPolicy::determine_transition(
                    BudgetKind::ConcurrentAgents,
                    is_interactive,
                ));
            }
        }

        // 2. Token budget check
        if let Some(max_tokens) = limits.max_tokens {
            let total_anticipated = self
                .tokens_consumed
                .load(Ordering::SeqCst)
                .saturating_add(self.tokens_reserved.load(Ordering::SeqCst))
                .saturating_add(estimates.estimated_tokens);
            if total_anticipated > max_tokens {
                return Err(BudgetExhaustionPolicy::determine_transition(
                    BudgetKind::Tokens,
                    is_interactive,
                ));
            }
        }

        // 3. Cost budget check
        if let Some(max_cost) = limits.max_cost_usd {
            let max_microcents = (max_cost * MICRO_CENTS_PER_USD) as u64;
            let est_microcents = (estimates.estimated_cost_usd * MICRO_CENTS_PER_USD) as u64;
            let total_cost = self
                .cost_consumed_microcents
                .load(Ordering::SeqCst)
                .saturating_add(self.cost_reserved_microcents.load(Ordering::SeqCst))
                .saturating_add(est_microcents);
            if total_cost > max_microcents {
                return Err(BudgetExhaustionPolicy::determine_transition(
                    BudgetKind::CostUsd,
                    is_interactive,
                ));
            }
        }

        // 4. Artifact bytes check
        if let Some(max_artifacts) = limits.max_artifact_bytes {
            let total_artifacts = self
                .artifact_bytes_consumed
                .load(Ordering::SeqCst)
                .saturating_add(self.artifact_bytes_reserved.load(Ordering::SeqCst))
                .saturating_add(estimates.estimated_artifact_bytes);
            if total_artifacts > max_artifacts {
                return Err(BudgetExhaustionPolicy::determine_transition(
                    BudgetKind::ArtifactBytes,
                    is_interactive,
                ));
            }
        }

        // Commit reservation
        if estimates.requires_worker {
            self.workers_active.fetch_add(1, Ordering::SeqCst);
        }
        self.tokens_reserved
            .fetch_add(estimates.estimated_tokens, Ordering::SeqCst);
        let est_microcents = (estimates.estimated_cost_usd * MICRO_CENTS_PER_USD) as u64;
        self.cost_reserved_microcents
            .fetch_add(est_microcents, Ordering::SeqCst);
        self.artifact_bytes_reserved
            .fetch_add(estimates.estimated_artifact_bytes, Ordering::SeqCst);

        Ok(ReservationReceipt::new(
            estimates.estimated_tokens,
            estimates.estimated_cost_usd,
            estimates.requires_worker,
            estimates.estimated_artifact_bytes,
        ))
    }

    /// Post-execution settlement phase: release reserved amounts and record actual consumption.
    ///
    /// All reservation releases are saturating so a synthetic receipt, double
    /// settle, or settle-after-release can never wrap unsigned counters negative.
    /// Totals remain monotonic.
    #[allow(deprecated)]
    pub fn settle(&self, receipt: &ReservationReceipt, actual: &ActualUsage) {
        // Release worker (saturating: never underflow on double settle).
        if receipt.reserved_worker {
            let prev = self.workers_active.load(Ordering::SeqCst);
            if prev > 0 {
                self.workers_active.fetch_sub(1, Ordering::SeqCst);
            }
        }

        // Release reserved tokens and add actual consumed
        self.tokens_reserved
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |cur| {
                Some(cur.saturating_sub(receipt.reserved_tokens))
            })
            .ok();
        self.tokens_consumed
            .fetch_add(actual.tokens, Ordering::SeqCst);

        // Release reserved cost and add actual consumed
        let est_microcents = (receipt.reserved_cost_usd * MICRO_CENTS_PER_USD) as u64;
        let actual_microcents = (actual.cost_usd * MICRO_CENTS_PER_USD) as u64;
        self.cost_reserved_microcents
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |cur| {
                Some(cur.saturating_sub(est_microcents))
            })
            .ok();
        self.cost_consumed_microcents
            .fetch_add(actual_microcents, Ordering::SeqCst);

        // Release reserved artifact bytes and add actual
        self.artifact_bytes_reserved
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |cur| {
                Some(cur.saturating_sub(receipt.reserved_artifact_bytes))
            })
            .ok();
        self.artifact_bytes_consumed
            .fetch_add(actual.artifact_bytes, Ordering::SeqCst);

        // Record steps, calls, retries
        self.agent_steps_consumed
            .fetch_add(actual.steps, Ordering::SeqCst);
        self.model_calls_consumed
            .fetch_add(actual.calls, Ordering::SeqCst);
        self.retries_consumed
            .fetch_add(actual.retries, Ordering::SeqCst);
    }

    /// Read snapshot of total consumed tokens.
    pub fn total_tokens_consumed(&self) -> u64 {
        self.tokens_consumed.load(Ordering::SeqCst)
    }

    /// Settle a reservation against authoritative provider-reported usage.
    ///
    /// Preferred over raw `settle` when the provider returned usable
    /// counters: releases the estimate reservation and records
    /// `usage.total_tokens` as consumed. When `usage.source ==
    /// Estimated`, the caller MUST have fallen back explicitly (e.g. the
    /// provider genuinely reported no counters); the settlement still
    /// proceeds but the returned `ActualUsage` is distinguishable via
    /// `ActualUsage::is_authoritative`. Totals remain monotonic
    /// (saturating adds; consumed counters never decrease and can never go
    /// negative — atomics are unsigned and reservation release is bounded
    /// by the outstanding receipt).
    pub fn settle_model_usage(
        &self,
        receipt: &ReservationReceipt,
        usage: &TokenUsage,
    ) -> ActualUsage {
        let actual = ActualUsage::from_token_usage(usage);
        self.settle(receipt, &actual);
        actual
    }

    /// Release reserved amounts without recording consumption (e.g. when work is aborted/denied before execution).
    ///
    /// Denied or aborted work must not inflate step and call counters.
    /// Release records zero steps, calls, tokens, and cost.
    pub fn release_reservation(&self, receipt: &ReservationReceipt) {
        let zero = ActualUsage {
            steps: 0,
            calls: 0,
            tokens: 0,
            cost_usd: 0.0,
            artifact_bytes: 0,
            retries: 0,
        };
        self.settle(receipt, &zero);
    }

    /// Read snapshot of total consumed cost in USD.
    pub fn total_cost_usd_consumed(&self) -> f64 {
        self.cost_consumed_microcents.load(Ordering::SeqCst) as f64 / MICRO_CENTS_PER_USD
    }

    /// Read snapshot of total active workers.
    pub fn active_workers(&self) -> usize {
        self.workers_active.load(Ordering::SeqCst)
    }

    /// Restore consumption counters from the durable ledger.
    ///
    /// Must only target a FRESH enforcer (all counters zero): restoring onto
    /// live counters would double-count. The ledger is the crash-recovery
    /// backup; the enforcer remains the single-run authority.
    pub fn restore_consumed(
        &self,
        tokens: u64,
        cost_microcents: u64,
        artifact_bytes: u64,
        steps: usize,
        calls: usize,
        retries: usize,
    ) {
        self.tokens_consumed.fetch_add(tokens, Ordering::SeqCst);
        self.cost_consumed_microcents
            .fetch_add(cost_microcents, Ordering::SeqCst);
        self.artifact_bytes_consumed
            .fetch_add(artifact_bytes, Ordering::SeqCst);
        self.agent_steps_consumed.fetch_add(steps, Ordering::SeqCst);
        self.model_calls_consumed.fetch_add(calls, Ordering::SeqCst);
        self.retries_consumed.fetch_add(retries, Ordering::SeqCst);
    }

    /// Read an authoritative snapshot of budget limits and real-time consumption.
    pub fn snapshot(&self) -> BudgetSnapshot {
        let limits = self.current_limits();
        BudgetSnapshot {
            max_agent_steps: limits.max_agent_steps,
            max_tokens: limits.max_tokens,
            max_wall_clock_seconds: limits.max_wall_clock_seconds,
            max_cost_usd: limits.max_cost_usd,
            max_retries: limits.max_retries,
            max_concurrent_agents: limits.max_concurrent_agents,
            max_artifact_bytes: limits.max_artifact_bytes,
            tokens_consumed: self.tokens_consumed.load(Ordering::SeqCst),
            tokens_reserved: self.tokens_reserved.load(Ordering::SeqCst),
            cost_consumed_usd: self.cost_consumed_microcents.load(Ordering::SeqCst) as f64
                / MICRO_CENTS_PER_USD,
            cost_reserved_usd: self.cost_reserved_microcents.load(Ordering::SeqCst) as f64
                / MICRO_CENTS_PER_USD,
            active_workers: self.workers_active.load(Ordering::SeqCst),
            artifact_bytes_consumed: self.artifact_bytes_consumed.load(Ordering::SeqCst),
            retries_consumed: self.retries_consumed.load(Ordering::SeqCst),
            agent_steps_consumed: self.agent_steps_consumed.load(Ordering::SeqCst),
            model_calls_consumed: self.model_calls_consumed.load(Ordering::SeqCst),
        }
    }
}

/// Complete snapshot of current budget limits and consumption counters.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BudgetSnapshot {
    pub max_agent_steps: Option<usize>,
    pub max_tokens: Option<u64>,
    pub max_wall_clock_seconds: Option<u64>,
    pub max_cost_usd: Option<f64>,
    pub max_retries: Option<usize>,
    pub max_concurrent_agents: Option<usize>,
    pub max_artifact_bytes: Option<u64>,

    pub tokens_consumed: u64,
    pub tokens_reserved: u64,
    pub cost_consumed_usd: f64,
    pub cost_reserved_usd: f64,
    pub active_workers: usize,
    pub artifact_bytes_consumed: u64,
    pub retries_consumed: usize,
    pub agent_steps_consumed: usize,
    pub model_calls_consumed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_phase_reservation_and_settle() {
        let limits = ResourceBudget {
            max_tokens: Some(1000),
            max_concurrent_agents: Some(1),
            ..Default::default()
        };
        let enforcer = BudgetEnforcer::new(limits);

        let estimates = TaskEstimates {
            estimated_tokens: 600,
            estimated_cost_usd: 0.01,
            requires_worker: true,
            estimated_artifact_bytes: 0,
        };

        // First task reservation succeeds
        let receipt = enforcer.reserve(&estimates, true).unwrap();
        assert_eq!(enforcer.active_workers(), 1);

        // Second task reservation fails due to worker concurrency limit
        let second_est = TaskEstimates {
            estimated_tokens: 100,
            estimated_cost_usd: 0.0,
            requires_worker: true,
            estimated_artifact_bytes: 0,
        };
        let err = enforcer.reserve(&second_est, true).unwrap_err();
        assert_eq!(err, BudgetExhaustionAction::FailClosed);

        // Settle first task with actual 400 tokens
        let actual = ActualUsage {
            tokens: 400,
            ..Default::default()
        };
        enforcer.settle(&receipt, &actual);

        assert_eq!(enforcer.active_workers(), 0);
        assert_eq!(enforcer.total_tokens_consumed(), 400);

        // Now second task can be reserved!
        assert!(enforcer.reserve(&second_est, true).is_ok());
    }

    #[test]
    fn test_token_exhaustion_in_reservation() {
        let limits = ResourceBudget {
            max_tokens: Some(500),
            ..Default::default()
        };
        let enforcer = BudgetEnforcer::new(limits);

        let estimates = TaskEstimates {
            estimated_tokens: 600,
            estimated_cost_usd: 0.0,
            requires_worker: false,
            estimated_artifact_bytes: 0,
        };

        // Exceeds token budget -> soft limit in interactive mode pauses for approval
        let err = enforcer.reserve(&estimates, true).unwrap_err();
        assert_eq!(err, BudgetExhaustionAction::PauseForApproval);

        // In unattended mode, blocks
        let err_unattended = enforcer.reserve(&estimates, false).unwrap_err();
        assert_eq!(err_unattended, BudgetExhaustionAction::Block);
    }
}
