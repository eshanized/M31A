//! Dynamic Model Catalog (PRD §01, MDL-01, D-05).
//!
//! Provides the canonical in-memory and cached model catalog for the M31A runtime.
//! Represents dynamically discovered provider model capabilities, context limits,
//! availability states, and deterministic fallback selection without hardcoded model lists.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::router::resolver::{ModelCandidate, ModelTier};
use crate::model::types::{ModelError, ProviderCapabilityStatus};

/// Provenance source of model catalog entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
    /// Models dynamically discovered from live provider endpoint.
    Discovered,
    /// Models restored from persistent local cache.
    Cache,
    /// Conservative fallback default catalog when discovery is unavailable.
    FallbackDefault,
    /// Offline test fixtures for deterministic validation.
    TestFixture,
}

impl std::fmt::Display for CatalogSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovered => write!(f, "discovered"),
            Self::Cache => write!(f, "cache"),
            Self::FallbackDefault => write!(f, "fallback_default"),
            Self::TestFixture => write!(f, "test_fixture"),
        }
    }
}

/// Refresh lifecycle state of the model catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogRefreshState {
    /// Discovery succeeded; catalog is current.
    DiscoverySuccess,
    /// Discovery failed; catalog loaded from cache and is explicitly stale.
    DiscoveryFailedWithCache,
    /// Discovery failed and no cache is available; unknown/unavailable catalog.
    DiscoveryFailedNoCache,
    /// Initialized in-memory, discovery not yet performed.
    Uninitialized,
}

impl std::fmt::Display for CatalogRefreshState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiscoverySuccess => write!(f, "current"),
            Self::DiscoveryFailedWithCache => write!(f, "stale_cache"),
            Self::DiscoveryFailedNoCache => write!(f, "unavailable_no_cache"),
            Self::Uninitialized => write!(f, "uninitialized"),
        }
    }
}

/// Canonical real-model integration configuration.
///
/// ALL real-model behavioral tests MUST resolve to this provider/model pair.
/// The value is centralized here so test files and runtime constructors never
/// scatter competing model identifiers. An explicit operator override via
/// `M31A_MODEL` / `NVIDIA_MODEL` is honored by [`resolve_real_model_id`];
/// otherwise the canonical identifier below is authoritative.
pub const CANONICAL_REAL_MODEL_PROVIDER: &str = "nvidia";
pub const CANONICAL_REAL_MODEL_ID: &str = "nvidia/nemotron-3-ultra-550b-a55b";
pub const CANONICAL_REAL_MODEL_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

/// Resolve the model identifier for real-model integration.
///
/// Returns the trimmed `M31A_MODEL` (fallback `NVIDIA_MODEL`) override when
/// set and non-empty, otherwise [`CANONICAL_REAL_MODEL_ID`]. This keeps a
/// single resolution point for every real-model test harness.
pub fn resolve_real_model_id() -> String {
    std::env::var("M31A_MODEL")
        .or_else(|_| std::env::var("NVIDIA_MODEL"))
        .map(|v| v.trim().to_string())
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| CANONICAL_REAL_MODEL_ID.to_string())
}

/// Canonical dynamic model catalog for the M31A runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub provider: String,
    pub models: Vec<ModelCandidate>,
    pub discovered_at: Option<u64>,
    pub source: CatalogSource,
    pub refresh_state: CatalogRefreshState,
}

impl ModelCatalog {
    /// Relative path inside workspace storage for persisting the cached model catalog.
    pub const CACHE_RELATIVE_PATH: &'static str = ".m31a/cache/model_catalog.json";

    /// Default freshness threshold (e.g. 1 hour).
    pub const DEFAULT_MAX_AGE_SECS: u64 = 3600;

    /// Create a new, uninitialized catalog for a provider.
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            models: Vec::new(),
            discovered_at: None,
            source: CatalogSource::FallbackDefault,
            refresh_state: CatalogRefreshState::Uninitialized,
        }
    }

    /// Construct a catalog from successful provider model discovery.
    pub fn from_discovered(
        provider: impl Into<String>,
        models: Vec<ModelCandidate>,
        timestamp: u64,
    ) -> Self {
        Self {
            provider: provider.into(),
            models,
            discovered_at: Some(timestamp),
            source: CatalogSource::Discovered,
            refresh_state: CatalogRefreshState::DiscoverySuccess,
        }
    }

    /// Construct a catalog loaded from cache, marking it as either current or stale.
    pub fn from_cache(
        provider: impl Into<String>,
        models: Vec<ModelCandidate>,
        timestamp: u64,
        is_stale: bool,
    ) -> Self {
        let refresh_state = if is_stale {
            CatalogRefreshState::DiscoveryFailedWithCache
        } else {
            CatalogRefreshState::DiscoverySuccess
        };
        Self {
            provider: provider.into(),
            models,
            discovered_at: Some(timestamp),
            source: CatalogSource::Cache,
            refresh_state,
        }
    }

    /// Construct an explicitly failed, empty catalog when discovery fails and no cache exists.
    pub fn failed_no_cache(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            models: Vec::new(),
            discovered_at: None,
            source: CatalogSource::FallbackDefault,
            refresh_state: CatalogRefreshState::DiscoveryFailedNoCache,
        }
    }

    /// Update catalog entries from discovered provider models.
    pub fn update_from_provider(
        &mut self,
        provider: impl Into<String>,
        models: Vec<ModelCandidate>,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.provider = provider.into();
        self.models = models;
        self.discovered_at = Some(now);
        self.source = CatalogSource::Discovered;
        self.refresh_state = CatalogRefreshState::DiscoverySuccess;
    }

    /// Check if the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Total number of models in catalog.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Return all models matching a given tier.
    pub fn models_for_tier(&self, tier: ModelTier) -> Vec<&ModelCandidate> {
        self.models.iter().filter(|m| m.tier == tier).collect()
    }

    /// Find a model candidate by ID (case-insensitive search).
    pub fn find_model(&self, model_id: &str) -> Option<&ModelCandidate> {
        let query = model_id.trim().to_lowercase();
        self.models
            .iter()
            .find(|m| m.model_id.to_lowercase() == query)
    }

    /// Check if a model is available in the current catalog.
    pub fn contains_model(&self, model_id: &str) -> bool {
        self.find_model(model_id)
            .map(|m| m.availability == ProviderCapabilityStatus::Available)
            .unwrap_or(false)
    }

    /// Check whether catalog freshness has expired relative to current system time.
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match self.discovered_at {
            Some(ts) => now.saturating_sub(ts) > max_age_secs,
            None => true,
        }
    }

    /// Select a default model deterministically according to constitutional policy (§11):
    /// 1. Configured preferred model if present and available in catalog.
    /// 2. Deterministic Standard tier model (sorted lexicographically by model_id).
    /// 3. Deterministic Fast tier model (sorted lexicographically by model_id).
    /// 4. Deterministic Reasoning tier model (sorted lexicographically by model_id).
    /// 5. None if catalog is empty or no valid models exist.
    pub fn select_default(&self, preferred: Option<&str>) -> Option<ModelCandidate> {
        if let Some(candidate) = preferred
            .and_then(|p| self.find_model(p))
            .filter(|c| c.availability == ProviderCapabilityStatus::Available)
        {
            return Some(candidate.clone());
        }

        let mut available: Vec<&ModelCandidate> = self
            .models
            .iter()
            .filter(|m| {
                m.availability == ProviderCapabilityStatus::Available
                    && m.context_capacity >= 4096
                    && m.supports_tools
            })
            .collect();

        if available.is_empty() {
            return None;
        }

        // 1. Try Standard tier models
        let mut standard: Vec<&ModelCandidate> = available
            .iter()
            .copied()
            .filter(|m| m.tier == ModelTier::Standard)
            .collect();
        if !standard.is_empty() {
            standard.sort_by(|a, b| a.model_id.cmp(&b.model_id));
            return Some(standard[0].clone());
        }

        // 2. Try Fast tier models
        let mut fast: Vec<&ModelCandidate> = available
            .iter()
            .copied()
            .filter(|m| m.tier == ModelTier::Fast)
            .collect();
        if !fast.is_empty() {
            fast.sort_by(|a, b| a.model_id.cmp(&b.model_id));
            return Some(fast[0].clone());
        }

        // 3. Try Reasoning tier models
        let mut reasoning: Vec<&ModelCandidate> = available
            .iter()
            .copied()
            .filter(|m| m.tier == ModelTier::Reasoning)
            .collect();
        if !reasoning.is_empty() {
            reasoning.sort_by(|a, b| a.model_id.cmp(&b.model_id));
            return Some(reasoning[0].clone());
        }

        // 4. Stable tie-break over remaining available models
        available.sort_by(|a, b| a.model_id.cmp(&b.model_id));
        Some(available[0].clone())
    }

    /// Select a fast auxiliary model deterministically from the catalog.
    /// Prefers available Fast tier models with context >= 4096, excluding the primary model if possible.
    pub fn select_fast_default(&self, exclude: Option<&str>) -> Option<ModelCandidate> {
        let available: Vec<&ModelCandidate> = self
            .models
            .iter()
            .filter(|m| {
                m.availability == ProviderCapabilityStatus::Available
                    && m.context_capacity >= 4096
                    && Some(m.model_id.as_str()) != exclude
                    && !m.model_id.contains("embed")
                    && !m.model_id.contains("reward")
            })
            .collect();

        if available.is_empty() {
            return self
                .models
                .iter()
                .find(|m| {
                    m.availability == ProviderCapabilityStatus::Available
                        && m.context_capacity >= 4096
                })
                .cloned();
        }

        // 1. Try Fast tier models with tool support
        let mut fast_tools: Vec<&ModelCandidate> = available
            .iter()
            .copied()
            .filter(|m| m.tier == ModelTier::Fast && m.supports_tools)
            .collect();
        if !fast_tools.is_empty() {
            fast_tools.sort_by(|a, b| a.model_id.cmp(&b.model_id));
            return Some(fast_tools[0].clone());
        }

        // 2. Try any Fast tier models
        let mut fast: Vec<&ModelCandidate> = available
            .iter()
            .copied()
            .filter(|m| m.tier == ModelTier::Fast)
            .collect();
        if !fast.is_empty() {
            fast.sort_by(|a, b| a.model_id.cmp(&b.model_id));
            return Some(fast[0].clone());
        }

        // 2. Try Standard tier models
        let mut standard: Vec<&ModelCandidate> = available
            .iter()
            .copied()
            .filter(|m| m.tier == ModelTier::Standard)
            .collect();
        if !standard.is_empty() {
            standard.sort_by(|a, b| a.model_id.cmp(&b.model_id));
            return Some(standard[0].clone());
        }

        // 3. Stable tie-break over remaining available models
        let mut all = available;
        all.sort_by(|a, b| a.model_id.cmp(&b.model_id));
        Some(all[0].clone())
    }

    /// Load catalog from JSON cache file.
    pub fn load_from_cache_file(path: &Path) -> Result<Self, ModelError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ModelError::ProviderInternalFailure(e.to_string()))?;
        let catalog: Self = serde_json::from_str(&content)
            .map_err(|e| ModelError::InvalidResponse(e.to_string()))?;
        Ok(catalog)
    }

    /// Save catalog to JSON cache file with directory creation.
    pub fn save_to_cache_file(&self, path: &Path) -> Result<(), ModelError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ModelError::ProviderInternalFailure(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ModelError::ProviderInternalFailure(e.to_string()))?;
        fs::write(path, json).map_err(|e| ModelError::ProviderInternalFailure(e.to_string()))?;
        Ok(())
    }

    /// Get standard cache file path for a workspace.
    pub fn cache_path(workspace_root: &Path) -> PathBuf {
        workspace_root.join(Self::CACHE_RELATIVE_PATH)
    }
}
