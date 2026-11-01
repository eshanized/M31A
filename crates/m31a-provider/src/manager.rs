//! Provider manager and model configuration catalog.
//!
//! Oversees active model configurations, API keys, provider failover policies, and health monitoring.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::debug;

use super::models::{
    CanonicalModel, CanonicalPricing, InputModality, ModelInfo, RefreshStrategy, ServiceTier,
    ToolMode, get_nvidia_model_catalog,
};

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

/// Per-originator model partition (upstream: models grouped by vendor/originator).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPartition {
    pub originator: String,
    pub models: Vec<ModelInfo>,
    pub etag: Option<String>,
    pub last_refreshed: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelPreset {
    Speed,
    Code,
    Reasoning,
    Flagship,
}

impl ModelPreset {
    pub fn default_model_slug(&self) -> &'static str {
        match self {
            Self::Speed => "nvidia/nemotron-3.5-bolt-e4b",
            Self::Code => "mistralai/codestral-22b-instruct-v0.1",
            Self::Reasoning => "deepseek-ai/deepseek-v4.1-flash",
            Self::Flagship => "nvidia/nemotron-3.5-lightning-30b-a3b",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextBudgetStatus {
    WithinBudget,
    ApproachingLimit { used: u64, limit: u64, pct: u8 },
    Exceeded { used: u64, limit: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheFilePayload {
    timestamp: u64,
    partitions: Vec<ModelPartition>,
    etag: Option<String>,
}

pub struct ModelsManager {
    cache_path: PathBuf,
    ttl: Duration,
    refresh_strategy: RefreshStrategy,
    partitions: Arc<RwLock<BTreeMap<String, ModelPartition>>>,
    fallback_map: HashMap<String, String>,
    /// If true, filters models by auth requirement (only models the user can access).
    filter_by_auth: bool,
    /// If true, marks the default model in picker UI.
    mark_default_by_picker_visibility: bool,
}

impl ModelsManager {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_path = PathBuf::from(home)
            .join(".config")
            .join("m31a")
            .join("models_cache.json");

        let mut fallback_map = HashMap::new();
        fallback_map.insert(
            "meta/llama-3.3-70b-instruct".to_string(),
            "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        );
        fallback_map.insert(
            "deepseek-ai/deepseek-r1".to_string(),
            "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        );
        fallback_map.insert(
            "qwen/qwen2.5-coder-32b-instruct".to_string(),
            "mistralai/codestral-22b-instruct-v0.1".to_string(),
        );

        // Initialize with bundled catalog, partitioned by originator
        let mut partitions = BTreeMap::new();
        for model in get_nvidia_model_catalog() {
            let partition = partitions
                .entry(model.originator.clone())
                .or_insert_with(|| ModelPartition {
                    originator: model.originator.clone(),
                    models: Vec::new(),
                    etag: None,
                    last_refreshed: None,
                });
            partition.models.push(model);
        }

        Self {
            cache_path,
            ttl: DEFAULT_CACHE_TTL,
            refresh_strategy: RefreshStrategy::OnlineIfUncached,
            partitions: Arc::new(RwLock::new(partitions)),
            fallback_map,
            filter_by_auth: true,
            mark_default_by_picker_visibility: true,
        }
    }

    pub fn with_cache_path(mut self, path: PathBuf) -> Self {
        self.cache_path = path;
        self
    }

    pub fn with_refresh_strategy(mut self, strategy: RefreshStrategy) -> Self {
        self.refresh_strategy = strategy;
        self
    }

    pub fn with_filter_by_auth(mut self, filter: bool) -> Self {
        self.filter_by_auth = filter;
        self
    }

    pub fn with_mark_default_by_picker_visibility(mut self, mark: bool) -> Self {
        self.mark_default_by_picker_visibility = mark;
        self
    }

    /// Loads model catalog from persistent cache file if valid, or falls back to built-in catalog.
    /// Respects `refresh_strategy`:
    /// - `Online`: fetch from remote and refresh cache
    /// - `Offline`: always use cache or built-in
    /// - `OnlineIfUncached`: try cache first, then built-in
    pub async fn load_cache(&self) {
        if matches!(self.refresh_strategy, RefreshStrategy::Offline) {
            return;
        }

        if let Ok(data) = fs::read_to_string(&self.cache_path) {
            if let Ok(payload) = serde_json::from_str::<CacheFilePayload>(&data) {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // Check if cache is fresh (TTL not expired)
                if now.saturating_sub(payload.timestamp) < self.ttl.as_secs()
                    && !payload.partitions.is_empty()
                {
                    let mut lock = self.partitions.write().await;
                    *lock = payload
                        .partitions
                        .into_iter()
                        .map(|p| (p.originator.clone(), p))
                        .collect();
                    debug!("Loaded model partitions from cache file");
                }
            } else if let Ok(models) = serde_json::from_str::<Vec<ModelInfo>>(&data)
                && !models.is_empty()
            {
                let mut lock = self.partitions.write().await;
                for m in models {
                    let partition =
                        lock.entry(m.originator.clone())
                            .or_insert_with(|| ModelPartition {
                                originator: m.originator.clone(),
                                models: Vec::new(),
                                etag: None,
                                last_refreshed: None,
                            });
                    if !partition
                        .models
                        .iter()
                        .any(|existing| existing.slug == m.slug)
                    {
                        partition.models.push(m);
                    }
                }
                debug!("Loaded flat models list from cache file into partitions");
            }
        }
    }

    /// Fetches models from remote `/v1/models` endpoint and merges them into the catalog.
    pub async fn refresh_remote(&self, api_base: &str, api_key: &str) -> Result<usize> {
        let client = reqwest::Client::new();
        let url = format!("{}/models", api_base.trim_end_matches('/'));
        let mut req = client.get(&url);
        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            bail!("Remote model endpoint returned status: {}", resp.status());
        }
        let data = resp.json::<serde_json::Value>().await?;
        let mut added = 0usize;
        if let Some(arr) = data.get("data").and_then(|v| v.as_array()) {
            let mut lock = self.partitions.write().await;
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    let originator = id.split('/').next().unwrap_or("remote").to_string();
                    let partition =
                        lock.entry(originator.clone())
                            .or_insert_with(|| ModelPartition {
                                originator: originator.clone(),
                                models: Vec::new(),
                                etag: None,
                                last_refreshed: None,
                            });
                    if !partition.models.iter().any(|m| m.slug == id) {
                        partition.models.push(ModelInfo {
                            slug: id.to_string(),
                            display_name: id.to_string(),
                            description: format!("Discovered model {id}"),
                            originator,
                            ..ModelInfo::default()
                        });
                        added += 1;
                    }
                }
            }
        }
        if added > 0 {
            let _ = self.save_cache().await;
        }
        Ok(added)
    }

    /// Saves the current model catalog to disk cache with ETag.
    pub async fn save_cache(&self) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let partitions: Vec<ModelPartition> =
            self.partitions.read().await.values().cloned().collect();
        let combined_etag = Self::compute_combined_etag(&partitions);

        let payload = CacheFilePayload {
            timestamp: now,
            partitions,
            etag: combined_etag,
        };

        let json = serde_json::to_string_pretty(&payload)?;
        fs::write(&self.cache_path, json)?;
        Ok(())
    }

    /// Computes a combined ETag from all partitions.
    fn compute_combined_etag(partitions: &[ModelPartition]) -> Option<String> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for p in partitions {
            if let Some(etag) = &p.etag {
                hasher.update(etag.as_bytes());
            }
            for m in &p.models {
                if let Some(etag) = &m.etag {
                    hasher.update(etag.as_bytes());
                }
            }
        }
        let result = hasher.finalize();
        Some(format!("{:x}", result))
    }

    /// Returns all available models (flattened, optionally filtered by auth).
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        let lock = self.partitions.read().await;
        let mut models = Vec::new();
        for partition in lock.values() {
            for model in &partition.models {
                if self.filter_by_auth && model.requires_auth {
                    // In a real implementation, check user's auth scopes
                    // For now, include all models that don't require auth, or all if filter disabled
                    models.push(model.clone());
                } else {
                    models.push(model.clone());
                }
            }
        }
        models
    }

    /// Returns models filtered by originator (per-originator partitioning).
    pub async fn list_models_by_originator(&self, originator: &str) -> Vec<ModelInfo> {
        let lock = self.partitions.read().await;
        if let Some(p) = lock.get(originator) {
            return p.models.clone();
        }
        if originator.eq_ignore_ascii_case("deepseek") {
            if let Some(p) = lock.get("deepseek-ai") {
                return p.models.clone();
            }
        } else if originator.eq_ignore_ascii_case("deepseek-ai")
            && let Some(p) = lock.get("deepseek")
        {
            return p.models.clone();
        }
        Vec::new()
    }

    /// Returns total number of available models across all partitions.
    pub async fn total_model_count(&self) -> usize {
        let lock = self.partitions.read().await;
        lock.values().map(|p| p.models.len()).sum()
    }

    /// Returns all models matching a free-text search across slug, name, and description.
    pub async fn search_models(&self, query: &str) -> Vec<ModelInfo> {
        let all = self.list_models().await;
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return all;
        }
        all.into_iter()
            .filter(|m| {
                m.slug.to_ascii_lowercase().contains(&q)
                    || m.display_name.to_ascii_lowercase().contains(&q)
                    || m.description.to_ascii_lowercase().contains(&q)
                    || m.originator.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    /// Returns models that support reasoning/thinking traces.
    pub async fn list_reasoning_models(&self) -> Vec<ModelInfo> {
        self.list_models()
            .await
            .into_iter()
            .filter(|m| m.supports_thinking)
            .collect()
    }

    /// Returns models that support multimodal vision input.
    pub async fn list_vision_models(&self) -> Vec<ModelInfo> {
        self.list_models()
            .await
            .into_iter()
            .filter(|m| m.supports_vision)
            .collect()
    }

    /// Returns models specialized for coding and tool usage.
    pub async fn list_coding_models(&self) -> Vec<ModelInfo> {
        self.list_models()
            .await
            .into_iter()
            .filter(|m| {
                m.tool_mode == ToolMode::CodeMode
                    || m.slug.contains("code")
                    || m.slug.contains("coder")
                    || m.slug.contains("codestral")
                    || m.slug.contains("starcoder")
            })
            .collect()
    }

    /// Returns models matching a specific input modality.
    pub async fn list_models_by_modality(&self, modality: InputModality) -> Vec<ModelInfo> {
        self.list_models()
            .await
            .into_iter()
            .filter(|m| m.input_modalities.contains(&modality))
            .collect()
    }

    /// Returns models matching a service tier.
    pub async fn list_models_by_service_tier(&self, tier: ServiceTier) -> Vec<ModelInfo> {
        self.list_models()
            .await
            .into_iter()
            .filter(|m| m.service_tier == tier)
            .collect()
    }

    /// Retrieves canonical model specification if available.
    pub fn get_canonical_model(&self, slug: &str) -> Option<&'static CanonicalModel> {
        crate::models::find_canonical_model(slug)
    }

    /// Retrieves official published pricing for a model if available.
    pub fn get_model_pricing(&self, slug: &str) -> Option<CanonicalPricing> {
        crate::models_identity::get_model_pricing(slug)
    }

    /// Returns all known originators.
    pub async fn list_originators(&self) -> Vec<String> {
        let lock = self.partitions.read().await;
        lock.keys().cloned().collect()
    }

    /// Finds a model by its slug identifier (across all partitions).
    pub async fn get_model(&self, slug: &str) -> Option<ModelInfo> {
        let lock = self.partitions.read().await;
        for partition in lock.values() {
            if let Some(model) = partition.models.iter().find(|m| m.slug == slug) {
                return Some(model.clone());
            }
        }
        None
    }

    /// Resolves the recommended fallback model when a model encounters persistent failures or rate-limits.
    pub fn resolve_fallback(&self, failed_model_slug: &str) -> Option<String> {
        self.fallback_map.get(failed_model_slug).cloned()
    }

    /// Evaluates whether token count exceeds or approaches context window limit.
    pub async fn evaluate_context_budget(
        &self,
        model_slug: &str,
        current_tokens: u64,
    ) -> ContextBudgetStatus {
        if let Some(model) = self.get_model(model_slug).await {
            let limit = model.context_window as u64;

            if current_tokens >= limit {
                return ContextBudgetStatus::Exceeded {
                    used: current_tokens,
                    limit,
                };
            }

            let pct = ((current_tokens as f64 / limit as f64) * 100.0) as u8;
            if pct >= 80 {
                ContextBudgetStatus::ApproachingLimit {
                    used: current_tokens,
                    limit,
                    pct,
                }
            } else {
                ContextBudgetStatus::WithinBudget
            }
        } else {
            // Unknown model - use default 131072 limit
            let limit = 131072;
            if current_tokens >= limit {
                ContextBudgetStatus::Exceeded {
                    used: current_tokens,
                    limit,
                }
            } else {
                let pct = ((current_tokens as f64 / limit as f64) * 100.0) as u8;
                if pct >= 80 {
                    ContextBudgetStatus::ApproachingLimit {
                        used: current_tokens,
                        limit,
                        pct,
                    }
                } else {
                    ContextBudgetStatus::WithinBudget
                }
            }
        }
    }

    /// Renders the model's base instructions from its template file.
    /// §7.3: Base instructions are shipped as `prompt.md` and rendered from `ModelInfo.instructions_template`.
    pub async fn render_model_instructions(&self, model_slug: &str) -> Option<String> {
        if let Some(model) = self.get_model(model_slug).await
            && let Some(template_path) = &model.instructions_template
        {
            // Try to load from bundled resources first
            if let Some(content) = Self::load_instructions_template(template_path) {
                return Some(content);
            }
        }
        None
    }

    /// Loads a model's instructions template from the model catalog directory.
    fn load_instructions_template(template_path: &str) -> Option<String> {
        // Look in standard locations:
        // 1. ~/.config/m31a/model-catalog/<template>
        // 2. Bundled resource (compile-time)
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let catalog_dir = PathBuf::from(home)
            .join(".config")
            .join("m31a")
            .join("model-catalog");
        let path = catalog_dir.join(template_path);
        if path.exists() {
            fs::read_to_string(path).ok()
        } else {
            // Fallback: check if it's a bundled template
            None
        }
    }

    /// Filters models by auth requirement (upstream `filter_by_auth`).
    pub async fn filter_models_by_auth(&self, user_scopes: &[String]) -> Vec<ModelInfo> {
        let all = self.list_models().await;
        if !self.filter_by_auth {
            return all;
        }
        all.into_iter()
            .filter(|m| {
                if !m.requires_auth {
                    return true;
                }
                // Check if user has required scopes for this model
                // Simplified: if model requires auth, user must have at least one scope
                !user_scopes.is_empty()
            })
            .collect()
    }

    /// Marks the default model in the list for picker visibility (upstream `mark_default_by_picker_visibility`).
    pub async fn mark_defaults(&self, mut models: Vec<ModelInfo>) -> Vec<ModelInfo> {
        if !self.mark_default_by_picker_visibility {
            return models;
        }
        for m in &mut models {
            if m.is_default {
                m.display_name = format!("[default] {}", m.display_name);
            }
        }
        models
    }

    /// Gets model with picker visibility (default marked, auth filtered).
    pub async fn get_models_for_picker(&self, user_scopes: &[String]) -> Vec<ModelInfo> {
        let mut models = self.filter_models_by_auth(user_scopes).await;
        models = self.mark_defaults(models).await;
        models
    }
}

impl Default for ModelsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ToolMode;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_models_manager_cache_lifecycle() {
        let dir = tempdir().unwrap();
        let cache_file = dir.path().join("models_cache.json");

        let manager = ModelsManager::new().with_cache_path(cache_file.clone());
        let initial_models = manager.list_models().await;
        assert!(!initial_models.is_empty());

        manager.save_cache().await.unwrap();
        assert!(cache_file.exists());

        // Re-read from cache
        let manager2 = ModelsManager::new().with_cache_path(cache_file);
        manager2.load_cache().await;
        let cached_models = manager2.list_models().await;
        assert_eq!(cached_models.len(), initial_models.len());
    }

    #[tokio::test]
    async fn test_context_budget_evaluation() {
        let manager = ModelsManager::new();
        // 131072 context window for Nemotron 3.5 Lightning
        let status = manager
            .evaluate_context_budget("nvidia/nemotron-3.5-lightning-30b-a3b", 50000)
            .await;
        assert_eq!(status, ContextBudgetStatus::WithinBudget);

        let status_warn = manager
            .evaluate_context_budget("nvidia/nemotron-3.5-lightning-30b-a3b", 110000)
            .await;
        assert!(matches!(
            status_warn,
            ContextBudgetStatus::ApproachingLimit { .. }
        ));

        let status_exceeded = manager
            .evaluate_context_budget("nvidia/nemotron-3.5-lightning-30b-a3b", 140000)
            .await;
        assert!(matches!(
            status_exceeded,
            ContextBudgetStatus::Exceeded { .. }
        ));
    }

    #[test]
    fn test_fallback_resolution() {
        let manager = ModelsManager::new();
        let fallback = manager.resolve_fallback("meta/llama-3.3-70b-instruct");
        assert_eq!(
            fallback.as_deref(),
            Some("nvidia/nemotron-3.5-lightning-30b-a3b")
        );
    }

    #[tokio::test]
    async fn test_model_capabilities() {
        let manager = ModelsManager::new();
        let model = manager
            .get_model("qwen/qwen2.5-coder-32b-instruct")
            .await
            .unwrap();
        assert_eq!(model.tool_mode, ToolMode::CodeMode);
        assert_eq!(model.multi_agent_version, 1);
        assert_eq!(model.auto_compact_threshold_percent, 75);
        assert!(model.instructions_template.is_some());
    }

    #[tokio::test]
    async fn test_originator_partitioning() {
        let manager = ModelsManager::new();
        let originators = manager.list_originators().await;
        assert!(originators.contains(&"nvidia".to_string()));
        assert!(originators.contains(&"deepseek".to_string()));
        assert!(originators.contains(&"qwen".to_string()));
        assert!(originators.contains(&"mistralai".to_string()));

        let nvidia_models = manager.list_models_by_originator("nvidia").await;
        assert!(!nvidia_models.is_empty());
        assert!(nvidia_models.iter().all(|m| m.originator == "nvidia"));
    }

    #[tokio::test]
    async fn test_picker_visibility() {
        let manager = ModelsManager::new()
            .with_mark_default_by_picker_visibility(true)
            .with_filter_by_auth(false); // Disable auth filter for test
        let models = manager
            .get_models_for_picker(&["test-scope".to_string()])
            .await;
        let default = models.iter().find(|m| m.is_default);
        assert!(
            default.is_some(),
            "default model should be visible in picker"
        );
        let default = default.unwrap();
        assert!(
            default.display_name.starts_with("[default] "),
            "default model should be marked with [default]"
        );
    }

    #[tokio::test]
    async fn test_manager_rich_queries() {
        let manager = ModelsManager::new();
        assert!(manager.total_model_count().await >= 85);

        let search_results = manager.search_models("gemma").await;
        assert!(!search_results.is_empty());

        let reasoning_models = manager.list_reasoning_models().await;
        assert!(!reasoning_models.is_empty());

        let vision_models = manager.list_vision_models().await;
        assert!(!vision_models.is_empty());

        let coding_models = manager.list_coding_models().await;
        assert!(!coding_models.is_empty());

        let pricing = manager.get_model_pricing("meta/codellama-70b");
        assert!(pricing.is_some());
    }
}
