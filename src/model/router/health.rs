//! Runtime-local circuit breaker with semantic failure classification (D-06, MDL-04).
//!
//! Tracks model/endpoint health across Healthy, Degraded, Open, and HalfOpen states.
//! Strictly enforces that client-side bugs (400 Bad Request, 401 Unauthorized, etc.)
//! never degrade provider health or trip circuit breakers.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::model::types::ModelError;

/// Operational health state of a model endpoint circuit breaker (D-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    /// Fully functional with zero or below-threshold failures.
    Healthy,
    /// Has experienced transient failures below the tripping threshold.
    Degraded,
    /// Circuit tripped due to rate limit cooldown or consecutive transient failures.
    Open,
    /// Cooldown has elapsed; a single probe request is permitted to test recovery.
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Open => write!(f, "open"),
            Self::HalfOpen => write!(f, "half_open"),
        }
    }
}

/// Semantic failure category determining impact on circuit health (D-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    /// Connection drop, timeout, DNS resolution failure.
    TransientNetwork,
    /// Provider-side 5xx server errors (500, 502, 503, 504).
    ServerOverload,
    /// HTTP 429 rate limit with required cooldown duration.
    RateLimit { cooldown_secs: u64 },
    /// Client-side / configuration errors (400, 401, 403, 404, invalid params, context length).
    /// These NEVER degrade circuit health.
    NonDegrading,
}

impl FailureKind {
    /// Classify a ModelError into a semantic failure category.
    pub fn from_model_error(error: &ModelError) -> Self {
        match error {
            ModelError::Network(_) | ModelError::StreamInterrupted(_) => Self::TransientNetwork,
            ModelError::RateLimited { cooldown_secs } => Self::RateLimit {
                cooldown_secs: *cooldown_secs,
            },
            ModelError::Http { status, .. } => {
                if *status == 429 {
                    Self::RateLimit { cooldown_secs: 30 }
                } else if (500..=599).contains(status) {
                    Self::ServerOverload
                } else {
                    // 400, 401, 403, 404, etc. are client/auth/config bugs, not provider outages.
                    Self::NonDegrading
                }
            }
            ModelError::AuthenticationFailed
            | ModelError::AuthenticationFailure(_)
            | ModelError::MissingConfiguration(_)
            | ModelError::MissingCredentials(_)
            | ModelError::InvalidRequest(_)
            | ModelError::UnsupportedCapability(_)
            | ModelError::ContextWindowExhausted { .. }
            | ModelError::Cancelled
            | ModelError::InvalidResponse(_)
            | ModelError::ProtocolViolation(_) => Self::NonDegrading,
            ModelError::EndpointUnavailable(_)
            | ModelError::ModelUnavailable(_)
            | ModelError::ProviderInternalFailure(_) => Self::ServerOverload,
            ModelError::Timeout(_) => Self::TransientNetwork,
        }
    }
}

/// State machine tracking availability, failure threshold, and cooldown for a model endpoint (D-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCircuitBreaker {
    pub endpoint: String,
    pub model_name: String,
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub failure_threshold: u32,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub half_open_probe_allowed: bool,
    pub last_failure_reason: Option<String>,
}

impl ModelCircuitBreaker {
    /// Construct a new circuit breaker in the Healthy state.
    pub fn new(endpoint: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model_name: model_name.into(),
            state: CircuitState::Healthy,
            consecutive_failures: 0,
            failure_threshold: 3,
            cooldown_until: None,
            half_open_probe_allowed: true,
            last_failure_reason: None,
        }
    }

    /// Builder method to customize failure threshold (minimum 1).
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold.max(1);
        self
    }

    /// Check if a request may be admitted through the circuit breaker.
    pub fn check_admission(&mut self) -> Result<(), ModelError> {
        self.check_admission_at(Utc::now())
    }

    /// Check admission evaluated at a specific timestamp (facilitates deterministic testing).
    pub fn check_admission_at(&mut self, now: DateTime<Utc>) -> Result<(), ModelError> {
        match self.state {
            CircuitState::Healthy | CircuitState::Degraded => Ok(()),
            CircuitState::Open => {
                if let Some(cooldown) = self.cooldown_until {
                    if now >= cooldown {
                        self.state = CircuitState::HalfOpen;
                        self.half_open_probe_allowed = false;
                        Ok(())
                    } else {
                        let diff = (cooldown - now).num_seconds();
                        let cooldown_secs = if diff <= 0 { 1 } else { diff as u64 };
                        Err(ModelError::RateLimited { cooldown_secs })
                    }
                } else {
                    // Open with no cooldown timestamp defaults to allowing probe
                    self.state = CircuitState::HalfOpen;
                    self.half_open_probe_allowed = false;
                    Ok(())
                }
            }
            CircuitState::HalfOpen => {
                if self.half_open_probe_allowed {
                    self.half_open_probe_allowed = false;
                    Ok(())
                } else {
                    // Probe request is already running; block concurrent admissions
                    Err(ModelError::RateLimited { cooldown_secs: 1 })
                }
            }
        }
    }

    /// Record a successful model invocation, resetting circuit to Healthy.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.cooldown_until = None;
        self.state = CircuitState::Healthy;
        self.half_open_probe_allowed = true;
        self.last_failure_reason = None;
    }

    /// Record a failure, updating state based on semantic failure classification.
    pub fn record_failure(&mut self, error: &ModelError) {
        self.record_failure_at(error, Utc::now());
    }

    /// Record a failure at a specific timestamp (facilitates deterministic testing).
    pub fn record_failure_at(&mut self, error: &ModelError, now: DateTime<Utc>) {
        let kind = FailureKind::from_model_error(error);
        self.last_failure_reason = Some(error.to_string());

        match kind {
            FailureKind::NonDegrading => {
                // Client errors (400, 401, 403, 404, etc.) do NOT degrade health (D-06).
            }
            FailureKind::RateLimit { cooldown_secs } => {
                let secs = if cooldown_secs == 0 {
                    30
                } else {
                    cooldown_secs
                };
                self.cooldown_until = Some(now + Duration::seconds(secs as i64));
                self.state = CircuitState::Open;
                self.half_open_probe_allowed = false;
            }
            FailureKind::TransientNetwork | FailureKind::ServerOverload => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                // If in HalfOpen or failures reach/exceed threshold, trip to Open
                if self.state == CircuitState::HalfOpen
                    || self.consecutive_failures >= self.failure_threshold
                {
                    self.cooldown_until = Some(now + Duration::seconds(60));
                    self.state = CircuitState::Open;
                    self.half_open_probe_allowed = false;
                } else {
                    self.state = CircuitState::Degraded;
                }
            }
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    pub fn cooldown_until(&self) -> Option<DateTime<Utc>> {
        self.cooldown_until
    }

    pub fn last_failure_reason(&self) -> Option<&str> {
        self.last_failure_reason.as_deref()
    }
}

/// Inspection report for a tracked model endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelHealthMetrics {
    pub endpoint: String,
    pub model_name: String,
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub failure_threshold: u32,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub last_failure_reason: Option<String>,
}

/// Thread-safe registry managing runtime circuit breakers across all endpoints and models (D-06).
#[derive(Debug, Clone)]
pub struct CircuitBreakerRegistry {
    breakers: Arc<RwLock<HashMap<String, ModelCircuitBreaker>>>,
    default_failure_threshold: u32,
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreakerRegistry {
    /// Construct a new empty CircuitBreakerRegistry with default failure threshold = 3.
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            default_failure_threshold: 3,
        }
    }

    /// Customize the default failure threshold for newly registered breakers.
    pub fn with_default_failure_threshold(mut self, threshold: u32) -> Self {
        self.default_failure_threshold = threshold.max(1);
        self
    }

    fn breaker_key(endpoint: &str, model_name: &str) -> String {
        format!("{}::{}", endpoint, model_name)
    }

    /// Check admission for a given endpoint and model.
    pub fn check_admission(&self, endpoint: &str, model_name: &str) -> Result<(), ModelError> {
        let key = Self::breaker_key(endpoint, model_name);
        let mut map = self.breakers.write().unwrap_or_else(|e| e.into_inner());
        let breaker = map.entry(key).or_insert_with(|| {
            ModelCircuitBreaker::new(endpoint, model_name)
                .with_failure_threshold(self.default_failure_threshold)
        });
        breaker.check_admission()
    }

    /// Record a success for a given endpoint and model.
    pub fn record_success(&self, endpoint: &str, model_name: &str) {
        let key = Self::breaker_key(endpoint, model_name);
        let mut map = self.breakers.write().unwrap_or_else(|e| e.into_inner());
        let breaker = map.entry(key).or_insert_with(|| {
            ModelCircuitBreaker::new(endpoint, model_name)
                .with_failure_threshold(self.default_failure_threshold)
        });
        breaker.record_success();
    }

    /// Record a failure for a given endpoint and model.
    pub fn record_failure(&self, endpoint: &str, model_name: &str, error: &ModelError) {
        let key = Self::breaker_key(endpoint, model_name);
        let mut map = self.breakers.write().unwrap_or_else(|e| e.into_inner());
        let breaker = map.entry(key).or_insert_with(|| {
            ModelCircuitBreaker::new(endpoint, model_name)
                .with_failure_threshold(self.default_failure_threshold)
        });
        breaker.record_failure(error);
    }

    /// Record a failure at a specific timestamp (facilitates deterministic testing).
    pub fn record_failure_at(
        &self,
        endpoint: &str,
        model_name: &str,
        error: &ModelError,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        let key = Self::breaker_key(endpoint, model_name);
        let mut map = self.breakers.write().unwrap_or_else(|e| e.into_inner());
        let breaker = map.entry(key).or_insert_with(|| {
            ModelCircuitBreaker::new(endpoint, model_name)
                .with_failure_threshold(self.default_failure_threshold)
        });
        breaker.record_failure_at(error, now);
    }

    /// Query the current state of a specific endpoint and model.
    pub fn get_state(&self, endpoint: &str, model_name: &str) -> CircuitState {
        let key = Self::breaker_key(endpoint, model_name);
        let map = self.breakers.read().unwrap_or_else(|e| e.into_inner());
        map.get(&key)
            .map(|b| b.state)
            .unwrap_or(CircuitState::Healthy)
    }

    /// Query current state for a model across any endpoint or by model name alone.
    pub fn get_state_for_model(&self, model_name: &str) -> CircuitState {
        let map = self.breakers.read().unwrap_or_else(|e| e.into_inner());
        for breaker in map.values() {
            if breaker.model_name == model_name {
                return breaker.state;
            }
        }
        CircuitState::Healthy
    }

    /// Admission-aware state query: evaluates the time-based
    /// Open→HalfOpen transition before reporting, so `resolve_model` observes
    /// cooldown expiry instead of a stale Open. Unknown endpoints report
    /// Healthy (same as `get_state`).
    pub fn admission_state(&self, endpoint: &str, model_name: &str) -> CircuitState {
        let key = Self::breaker_key(endpoint, model_name);
        let mut map = self.breakers.write().unwrap_or_else(|e| e.into_inner());
        let breaker = map.entry(key).or_insert_with(|| {
            ModelCircuitBreaker::new(endpoint, model_name)
                .with_failure_threshold(self.default_failure_threshold)
        });
        let _ = breaker.check_admission();
        breaker.state
    }

    /// Admission-aware variant of `get_state_for_model`: transitions the
    /// first matching breaker before reporting its state.
    pub fn admission_state_for_model(&self, model_name: &str) -> CircuitState {
        let mut map = self.breakers.write().unwrap_or_else(|e| e.into_inner());
        for breaker in map.values_mut() {
            if breaker.model_name == model_name {
                let _ = breaker.check_admission();
                return breaker.state;
            }
        }
        CircuitState::Healthy
    }

    /// Minimum remaining cooldown across Open breakers, if any.
    /// Lets callers wait out a live cooldown once instead of failing fast
    /// when a concurrent invocation tripped the breaker. Returns None when
    /// no breaker is cooling down.
    pub fn min_cooldown_remaining_secs(&self) -> Option<u64> {
        let now = chrono::Utc::now();
        let map = self.breakers.read().unwrap_or_else(|e| e.into_inner());
        map.values()
            .filter(|b| b.state == CircuitState::Open)
            .filter_map(|b| b.cooldown_until)
            .map(|until| (until - now).num_seconds().max(0) as u64)
            .min()
    }

    /// Inspect all tracked circuit breakers, returning snapshot metrics.
    pub fn inspect_all(&self) -> Vec<ModelHealthMetrics> {
        let map = self.breakers.read().unwrap_or_else(|e| e.into_inner());
        map.values()
            .map(|b| ModelHealthMetrics {
                endpoint: b.endpoint.clone(),
                model_name: b.model_name.clone(),
                state: b.state,
                consecutive_failures: b.consecutive_failures,
                failure_threshold: b.failure_threshold,
                cooldown_until: b.cooldown_until,
                last_failure_reason: b.last_failure_reason.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_request_does_not_trip_circuit() {
        let mut cb = ModelCircuitBreaker::new("https://api.nvidia.com", "meta/llama-3.1-8b");
        assert_eq!(cb.state(), CircuitState::Healthy);

        // 400 Bad Request error
        let err_400 = ModelError::Http {
            status: 400,
            message: "invalid parameter foo".to_string(),
        };
        cb.record_failure(&err_400);
        assert_eq!(cb.state(), CircuitState::Healthy);
        assert_eq!(cb.consecutive_failures(), 0);

        // 401 Unauthorized
        let err_401 = ModelError::AuthenticationFailed;
        cb.record_failure(&err_401);
        assert_eq!(cb.state(), CircuitState::Healthy);
        assert_eq!(cb.consecutive_failures(), 0);

        // 403 Forbidden
        let err_403 = ModelError::Http {
            status: 403,
            message: "forbidden".to_string(),
        };
        cb.record_failure(&err_403);
        assert_eq!(cb.state(), CircuitState::Healthy);
        assert_eq!(cb.consecutive_failures(), 0);

        // Admission still passes
        assert!(cb.check_admission().is_ok());
    }

    #[test]
    fn test_consecutive_503_transitions_healthy_to_degraded_to_open() {
        let mut cb = ModelCircuitBreaker::new("https://api.nvidia.com", "meta/llama-3.3-70b")
            .with_failure_threshold(3);
        assert_eq!(cb.state(), CircuitState::Healthy);

        let err_503 = ModelError::Http {
            status: 503,
            message: "service unavailable".to_string(),
        };

        // Failure 1: Healthy -> Degraded
        cb.record_failure(&err_503);
        assert_eq!(cb.state(), CircuitState::Degraded);
        assert_eq!(cb.consecutive_failures(), 1);
        assert!(cb.check_admission().is_ok());

        // Failure 2: stays Degraded
        cb.record_failure(&err_503);
        assert_eq!(cb.state(), CircuitState::Degraded);
        assert_eq!(cb.consecutive_failures(), 2);
        assert!(cb.check_admission().is_ok());

        // Failure 3: Degraded -> Open
        cb.record_failure(&err_503);
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.consecutive_failures(), 3);
        assert!(cb.cooldown_until().is_some());

        // Admission fails when Open
        let adm_err = cb.check_admission().unwrap_err();
        assert!(matches!(adm_err, ModelError::RateLimited { .. }));
    }

    #[test]
    fn test_429_immediately_opens_circuit_with_cooldown() {
        let mut cb = ModelCircuitBreaker::new("https://api.nvidia.com", "deepseek-ai/deepseek-r1");
        assert_eq!(cb.state(), CircuitState::Healthy);

        let err_429 = ModelError::RateLimited { cooldown_secs: 45 };
        cb.record_failure(&err_429);

        assert_eq!(cb.state(), CircuitState::Open);
        assert!(cb.cooldown_until().is_some());

        let adm_err = cb.check_admission().unwrap_err();
        match adm_err {
            ModelError::RateLimited { cooldown_secs } => {
                assert!((40..=45).contains(&cooldown_secs));
            }
            other => panic!("expected RateLimited error, got {:?}", other),
        }
    }

    #[test]
    fn test_cooldown_expiry_enables_half_open_probe_and_recovery() {
        let base_time = Utc::now();
        let mut cb = ModelCircuitBreaker::new("https://api.nvidia.com", "meta/llama-3.1-8b");

        // Force into Open with 10s cooldown
        let err_429 = ModelError::RateLimited { cooldown_secs: 10 };
        cb.record_failure_at(&err_429, base_time);
        assert_eq!(cb.state(), CircuitState::Open);

        // Before expiry: admission rejected
        let check_early = cb.check_admission_at(base_time + Duration::seconds(5));
        assert!(check_early.is_err());
        assert_eq!(cb.state(), CircuitState::Open);

        // After expiry: transitions to HalfOpen and allows single probe
        let check_expired = cb.check_admission_at(base_time + Duration::seconds(11));
        assert!(check_expired.is_ok());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Second admission before probe completes is rejected
        let check_concurrent = cb.check_admission_at(base_time + Duration::seconds(12));
        assert!(check_concurrent.is_err());

        // Probe succeeds -> transitions to Healthy
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Healthy);
        assert_eq!(cb.consecutive_failures(), 0);
        assert!(cb.check_admission().is_ok());
    }

    #[test]
    fn test_circuit_breaker_registry_inspection_and_updates() {
        let registry = CircuitBreakerRegistry::new();
        let endpoint = "https://api.nvidia.com";
        let model = "meta/llama-3.1-8b";

        assert_eq!(registry.get_state(endpoint, model), CircuitState::Healthy);
        assert!(registry.check_admission(endpoint, model).is_ok());

        // Record 500 error
        registry.record_failure(
            endpoint,
            model,
            &ModelError::Http {
                status: 500,
                message: "internal error".to_string(),
            },
        );
        assert_eq!(registry.get_state(endpoint, model), CircuitState::Degraded);
        assert_eq!(registry.get_state_for_model(model), CircuitState::Degraded);

        let reports = registry.inspect_all();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].model_name, model);
        assert_eq!(reports[0].state, CircuitState::Degraded);
        assert_eq!(reports[0].consecutive_failures, 1);

        // Record success
        registry.record_success(endpoint, model);
        assert_eq!(registry.get_state(endpoint, model), CircuitState::Healthy);
    }
}
