//! Local Model Health Monitoring and auto-recovery.
//!
//! Tracks responsiveness and memory health of local inference runtimes
//! (e.g., Ollama, LM Studio, LocalAI) to prevent agent deadlocks and hanging turns.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Status of the local LLM runtime health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy { latency_ms: u64 },
    Degraded { latency_ms: u64, message: String },
    Unreachable { error: String },
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy { .. })
    }

    pub fn is_operational(&self) -> bool {
        !matches!(self, HealthStatus::Unreachable { .. })
    }
}

/// Active health monitor for local LLM engines.
#[derive(Debug, Clone)]
pub struct LocalModelHealthMonitor {
    endpoint: String,
    timeout: Duration,
    client: reqwest::Client,
}

impl LocalModelHealthMonitor {
    pub fn new(endpoint: impl Into<String>) -> Self {
        let ep = endpoint.into();
        let timeout = Duration::from_secs(5);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            endpoint: ep,
            timeout,
            client,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Performs an active probe against the model server.
    pub async fn check_health(&self) -> HealthStatus {
        let start = Instant::now();

        // Target either OpenAI-compatible /v1/models or Ollama /api/tags
        let probe_url = if self.endpoint.contains("11434") {
            format!(
                "{}/api/tags",
                self.endpoint.trim_end_matches("/v1").trim_end_matches('/')
            )
        } else if self.endpoint.ends_with("/v1") {
            format!("{}/models", self.endpoint)
        } else {
            format!("{}/v1/models", self.endpoint.trim_end_matches('/'))
        };

        match self.client.get(&probe_url).send().await {
            Ok(resp) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                if resp.status().is_success() {
                    if latency_ms > 3000 {
                        HealthStatus::Degraded {
                            latency_ms,
                            message: format!(
                                "High latency response ({}ms) from {}",
                                latency_ms, probe_url
                            ),
                        }
                    } else {
                        HealthStatus::Healthy { latency_ms }
                    }
                } else {
                    HealthStatus::Degraded {
                        latency_ms,
                        message: format!("HTTP {} from {}", resp.status(), probe_url),
                    }
                }
            }
            Err(e) => HealthStatus::Unreachable {
                error: format!("Probe to {} failed: {}", probe_url, e),
            },
        }
    }

    /// Fetches model tags/names available on the local runtime.
    pub async fn list_local_models(&self) -> Result<Vec<String>> {
        let is_ollama = self.endpoint.contains("11434");
        let target_url = if is_ollama {
            format!(
                "{}/api/tags",
                self.endpoint.trim_end_matches("/v1").trim_end_matches('/')
            )
        } else if self.endpoint.ends_with("/v1") {
            format!("{}/models", self.endpoint)
        } else {
            format!("{}/v1/models", self.endpoint.trim_end_matches('/'))
        };

        let resp = self.client.get(&target_url).send().await?;
        let json: serde_json::Value = resp.json().await?;

        let mut models = Vec::new();
        if is_ollama {
            if let Some(arr) = json.get("models").and_then(|v| v.as_array()) {
                for item in arr {
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        models.push(name.to_string());
                    }
                }
            }
        } else if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    models.push(id.to_string());
                }
            }
        }

        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_predicates() {
        let healthy = HealthStatus::Healthy { latency_ms: 120 };
        assert!(healthy.is_healthy());
        assert!(healthy.is_operational());

        let degraded = HealthStatus::Degraded {
            latency_ms: 3500,
            message: "slow".into(),
        };
        assert!(!degraded.is_healthy());
        assert!(degraded.is_operational());

        let unreachable = HealthStatus::Unreachable {
            error: "connection refused".into(),
        };
        assert!(!unreachable.is_healthy());
        assert!(!unreachable.is_operational());
    }

    #[tokio::test]
    async fn test_unreachable_endpoint() {
        let monitor = LocalModelHealthMonitor::new("http://127.0.0.1:59999/v1")
            .with_timeout(Duration::from_millis(50));
        let status = monitor.check_health().await;
        assert!(!status.is_operational());
    }
}
