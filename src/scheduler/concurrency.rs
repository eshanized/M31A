use crate::state_machine::agent::AgentRole;
use std::collections::BTreeMap;
use thiserror::Error;

/// Explicit concurrency limits bounding host resources and role parallelism (D-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcurrencyLimits {
    pub max_global_workers: usize,
    pub max_per_role: BTreeMap<AgentRole, usize>,
}

impl Default for ConcurrencyLimits {
    fn default() -> Self {
        // Per-role limits come from the role registry (single authority);
        // roles without an explicit limit share the registry default.
        let mut max_per_role = BTreeMap::new();
        if let Ok(guard) = crate::agent::registry::RoleRegistry::global().read() {
            for id in guard.builtin_ids() {
                let role = AgentRole::new(&id);
                if let Some(def) = guard.resolve(&role)
                    && let Some(limit) = def.concurrency_limit
                {
                    max_per_role.insert(role, limit);
                }
            }
        }

        Self {
            max_global_workers: crate::config::canonical::DEFAULT_SCHEDULER_WORKERS,
            max_per_role,
        }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ConcurrencyExhaustedError {
    #[error("global worker limit exhausted: active {active} >= max {max}")]
    GlobalLimitReached { active: usize, max: usize },
    #[error("role worker limit exhausted for {role:?}: active {active} >= max {max}")]
    RoleLimitReached {
        role: AgentRole,
        active: usize,
        max: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcurrencyReservation {
    pub role: AgentRole,
}

/// Multi-dimensional concurrency limiter enforcing global worker and per-role bounds.
#[derive(Debug, Default, Clone)]
pub struct ConcurrencyLimiter {
    active_global_workers: usize,
    active_per_role: BTreeMap<AgentRole, usize>,
}

impl ConcurrencyLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_global_workers(&self) -> usize {
        self.active_global_workers
    }

    pub fn active_for_role(&self, role: AgentRole) -> usize {
        self.active_per_role.get(&role).copied().unwrap_or(0)
    }

    /// Check whether worker capacity is available without mutating state.
    ///
    /// Limit resolution preserves legacy semantics exactly: roles with a
    /// configured limit are bounded by it; built-in roles without one stay
    /// unbounded. Newly registered extension roles without an explicit
    /// limit are bounded by the registry default
    /// ([`DEFAULT_PER_ROLE_CONCURRENCY`](crate::agent::registry::DEFAULT_PER_ROLE_CONCURRENCY)),
    /// so extensions are concurrency-bounded without scheduler changes.
    pub fn can_reserve(
        &self,
        role: AgentRole,
        limits: &ConcurrencyLimits,
    ) -> Result<(), ConcurrencyExhaustedError> {
        if self.active_global_workers >= limits.max_global_workers {
            return Err(ConcurrencyExhaustedError::GlobalLimitReached {
                active: self.active_global_workers,
                max: limits.max_global_workers,
            });
        }

        let role_max = limits.max_per_role.get(&role).copied().or_else(|| {
            let is_builtin = crate::agent::registry::RoleRegistry::global()
                .read()
                .map(|guard| guard.builtin_ids().contains(&role.as_str().to_string()))
                .unwrap_or(true);
            if is_builtin {
                None
            } else {
                Some(crate::agent::registry::DEFAULT_PER_ROLE_CONCURRENCY)
            }
        });
        if let Some(role_max) = role_max {
            let current = self.active_for_role(role.clone());
            if current >= role_max {
                return Err(ConcurrencyExhaustedError::RoleLimitReached {
                    role,
                    active: current,
                    max: role_max,
                });
            }
        }

        Ok(())
    }

    /// Atomically check and reserve a global worker slot and role worker slot (D-10).
    pub fn try_reserve(
        &mut self,
        role: AgentRole,
        limits: &ConcurrencyLimits,
    ) -> Result<ConcurrencyReservation, ConcurrencyExhaustedError> {
        self.can_reserve(role.clone(), limits)?;

        self.active_global_workers += 1;
        *self.active_per_role.entry(role.clone()).or_insert(0) += 1;

        Ok(ConcurrencyReservation { role })
    }

    /// Release worker capacity for a role.
    pub fn release(&mut self, role: AgentRole) {
        self.active_global_workers = self.active_global_workers.saturating_sub(1);
        if let Some(count) = self.active_per_role.get_mut(&role) {
            *count = count.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_limit_rejection() {
        let mut limiter = ConcurrencyLimiter::new();
        let limits = ConcurrencyLimits {
            max_global_workers: 2,
            max_per_role: BTreeMap::new(),
        };

        let res1 = limiter.try_reserve(AgentRole::implementer(), &limits);
        assert!(res1.is_ok());
        let res2 = limiter.try_reserve(AgentRole::implementer(), &limits);
        assert!(res2.is_ok());

        // 3rd should fail global limit
        let res3 = limiter.try_reserve(AgentRole::implementer(), &limits);
        assert_eq!(
            res3.unwrap_err(),
            ConcurrencyExhaustedError::GlobalLimitReached { active: 2, max: 2 }
        );

        // Release one slot and try again
        limiter.release(AgentRole::implementer());
        assert_eq!(limiter.active_global_workers(), 1);
        assert!(
            limiter
                .try_reserve(AgentRole::implementer(), &limits)
                .is_ok()
        );
    }

    #[test]
    fn test_role_limit_rejection() {
        let mut limiter = ConcurrencyLimiter::new();
        let mut max_per_role = BTreeMap::new();
        max_per_role.insert(AgentRole::architect(), 1);
        let limits = ConcurrencyLimits {
            max_global_workers: 10,
            max_per_role,
        };

        let res1 = limiter.try_reserve(AgentRole::architect(), &limits);
        assert!(res1.is_ok());

        // 2nd architect fails role limit
        let res2 = limiter.try_reserve(AgentRole::architect(), &limits);
        assert_eq!(
            res2.unwrap_err(),
            ConcurrencyExhaustedError::RoleLimitReached {
                role: AgentRole::architect(),
                active: 1,
                max: 1,
            }
        );

        // But implementer can still be reserved
        let res3 = limiter.try_reserve(AgentRole::implementer(), &limits);
        assert!(res3.is_ok());
    }
}
