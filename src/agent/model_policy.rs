//! Model selection policy and ModelCaller seam trait (D-04, AGT-05).
//!
//! Enforces that the runtime deterministically selects models based on declared policy
//! and hard requirements with ordered fallback. LLMs cannot select their own model or role.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Declarative model policy embedded in an AgentProfile (D-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPolicy {
    pub min_context_tokens: usize,
    pub supports_tool_calling: bool,
    pub preferred_model: String,
    pub fallback_models: Vec<String>,
    pub temperature_millicelsius: u32,
}

pub use crate::model::router::resolver::{ModelResolutionError, ResolvedModelSelection};
pub use crate::model::types::{ModelProposal, ModelToolCall, TokenUsage, UsageSource};

use crate::kernel::seams::context::CompiledContext;
use tokio_util::sync::CancellationToken;

/// Seam trait for invoking model reasoning (allows deterministic mocking in tests).
#[async_trait]
pub trait ModelCaller: Send + Sync {
    /// Invoke the model with the compiled context prompt, receiving a structured proposal.
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String>;

    /// Invoke the model with the compiled context prompt and cooperative cancellation token (D-02, Law 8).
    ///
    /// Cancellation is pre-checked before the model is invoked. Without this,
    /// `tokio::select!` may resolve the immediately ready `call_model` branch
    /// even when the token is already cancelled, letting a fast caller
    /// outrun canonical cancellation. No model call may start after
    /// cancellation is committed.
    async fn call_model_cancellable(
        &self,
        context: &str,
        cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        if cancellation.is_cancelled() {
            return Err("model invocation cancelled by runtime".to_string());
        }
        tokio::select! {
            _ = cancellation.cancelled() => Err("model invocation cancelled by runtime".to_string()),
            res = self.call_model(context) => res,
        }
    }

    /// Invoke the model with full compiled context (including multi-turn message history) and cooperative cancellation (D-02, D-05).
    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let mut full_text = compiled.system_prompt.clone();
        for msg in &compiled.messages {
            full_text.push('\n');
            match msg {
                crate::model::types::ChatMessage::System { content }
                | crate::model::types::ChatMessage::User { content }
                | crate::model::types::ChatMessage::Tool { content, .. } => {
                    full_text.push_str(content);
                }
                crate::model::types::ChatMessage::Assistant { content, .. } => {
                    if let Some(c) = content {
                        full_text.push_str(c);
                    }
                }
            }
        }
        self.call_model_cancellable(&full_text, cancellation).await
    }

    /// Usage-propagating invocation.
    ///
    /// Returns the proposal together with the authoritative provider-reported
    /// `TokenUsage` where the implementation observes it. Implementations
    /// that genuinely have no provider usage (such as test doubles without usage)
    /// must return an explicitly `Estimated` zero record — never a silently
    /// fabricated value. Callers settle budget reservations and record telemetry
    /// from the returned usage via `ActualUsage::from_token_usage`, so downstream
    /// accounting stays authoritative end to end:
    /// ```text
    /// real provider usage → canonical TokenUsage → budget settlement
    ///                     → telemetry → TUI projection
    /// ```
    async fn call_model_cancellable_with_usage(
        &self,
        context: &str,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        let proposal = self.call_model_cancellable(context, cancellation).await?;
        Ok((
            proposal,
            TokenUsage::new(0, 0, 0, 0, UsageSource::Estimated),
        ))
    }

    /// Usage-propagating full-context invocation. Same authoritative-vs-estimated
    /// contract as `call_model_cancellable_with_usage`.
    async fn call_model_with_context_and_usage(
        &self,
        compiled: &CompiledContext,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        let proposal = self.call_model_with_context(compiled, cancellation).await?;
        Ok((
            proposal,
            TokenUsage::new(0, 0, 0, 0, UsageSource::Estimated),
        ))
    }

    /// Usage-propagating full-context streaming invocation.
    async fn call_model_with_context_and_usage_streaming(
        &self,
        compiled: &CompiledContext,
        cancellation: &CancellationToken,
        _chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::model::types::StreamChunk>>,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        self.call_model_with_context_and_usage(compiled, cancellation)
            .await
    }

    /// Usage-propagating TOOL-FREE invocation for planning/discovery/review/verification.
    ///
    /// Typed tool-visibility authority: callers that KNOW the invocation must
    /// not receive executable tool schemas (planning, discovery, review,
    /// verification prompts) MUST use this entrypoint instead of relying on
    /// prompt-text heuristics. The default implementation forwards to the
    /// standard path (test doubles without tool plumbing); the production
    /// `RoutedModelCaller` serves these invocations with an explicitly empty
    /// tool set so tool visibility can never be inferred from prompt strings.
    async fn call_model_tool_free_cancellable_with_usage(
        &self,
        context: &str,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        self.call_model_cancellable_with_usage(context, cancellation)
            .await
    }

    /// Proposal-only TOOL-FREE invocation (same authority contract as above).
    async fn call_model_tool_free_cancellable(
        &self,
        context: &str,
        cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let (proposal, _usage) = self
            .call_model_tool_free_cancellable_with_usage(context, cancellation)
            .await?;
        Ok(proposal)
    }

    /// Typed PromptOS invocation: model call bound to an EffectivePrompt.
    ///
    /// Preferred production entrypoint for prompt-compiled callers
    /// (reviewers, diagnosticians, planners): the prompt authority travels
    /// as a typed [`ModelInvocation`](crate::runtime_authorities::ModelInvocation),
    /// never as a raw `system_prompt: &str` that could bypass PromptOS.
    /// Fails closed when the effective prompt carries no provenance.
    /// The default implementation renders the compiled prompt text and
    /// delegates to the cancellable path (provider-specific callers may
    /// override to bind invocation-kind tool visibility).
    async fn call_model_with_invocation(
        &self,
        invocation: &crate::runtime_authorities::ModelInvocation,
        cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        invocation.require_provenance()?;
        self.call_model_cancellable(&invocation.prompt.assembled_text, cancellation)
            .await
    }

    /// Snapshot of the dynamic model catalog this caller is bound to, if any.
    ///
    /// Authority observability (Invariant 6): proves the caller observes the
    /// CURRENT catalog lock rather than a stale replacement. Returns `None`
    /// for callers without catalog binding (test doubles).
    async fn bound_catalog_snapshot(&self) -> Option<crate::model::catalog::ModelCatalog> {
        None
    }
}

/// Provider-neutral adapter wiring `ModelProvider` to `ModelCaller` seam (MDL-01, MDL-03).
pub struct ProviderModelCaller<P: crate::model::provider::ModelProvider> {
    pub provider: std::sync::Arc<P>,
    pub model_name: String,
    pub tools: Vec<serde_json::Value>,
}

impl<P: crate::model::provider::ModelProvider> ProviderModelCaller<P> {
    pub fn new(
        provider: std::sync::Arc<P>,
        model_name: impl Into<String>,
        tools: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            provider,
            model_name: model_name.into(),
            tools,
        }
    }
}

#[async_trait]
impl<P: crate::model::provider::ModelProvider> ModelCaller for ProviderModelCaller<P> {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        let token = CancellationToken::new();
        self.call_model_cancellable(context, &token).await
    }

    async fn call_model_cancellable(
        &self,
        context: &str,
        cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let (proposal, _usage) = self
            .provider
            .call_model(&self.model_name, context, self.tools.clone(), cancellation)
            .await
            .map_err(|e| e.to_string())?;

        Ok(proposal)
    }

    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let (proposal, _usage) = if !compiled.messages.is_empty() {
            self.provider
                .call_model_with_messages(
                    &self.model_name,
                    &compiled.messages,
                    self.tools.clone(),
                    cancellation,
                )
                .await
                .map_err(|e| e.to_string())?
        } else {
            self.provider
                .call_model(
                    &self.model_name,
                    &compiled.system_prompt,
                    self.tools.clone(),
                    cancellation,
                )
                .await
                .map_err(|e| e.to_string())?
        };

        Ok(proposal)
    }

    async fn call_model_cancellable_with_usage(
        &self,
        context: &str,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        self.provider
            .call_model(&self.model_name, context, self.tools.clone(), cancellation)
            .await
            .map_err(|e| e.to_string())
    }

    async fn call_model_with_context_and_usage(
        &self,
        compiled: &CompiledContext,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        if !compiled.messages.is_empty() {
            self.provider
                .call_model_with_messages(
                    &self.model_name,
                    &compiled.messages,
                    self.tools.clone(),
                    cancellation,
                )
                .await
                .map_err(|e| e.to_string())
        } else {
            self.provider
                .call_model(
                    &self.model_name,
                    &compiled.system_prompt,
                    self.tools.clone(),
                    cancellation,
                )
                .await
                .map_err(|e| e.to_string())
        }
    }

    async fn call_model_with_context_and_usage_streaming(
        &self,
        compiled: &CompiledContext,
        cancellation: &CancellationToken,
        chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::model::types::StreamChunk>>,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        if !compiled.messages.is_empty() {
            self.provider
                .call_model_with_messages_streaming(
                    &self.model_name,
                    &compiled.messages,
                    self.tools.clone(),
                    cancellation,
                    chunk_tx,
                )
                .await
                .map_err(|e| e.to_string())
        } else {
            self.provider
                .call_model(
                    &self.model_name,
                    &compiled.system_prompt,
                    self.tools.clone(),
                    cancellation,
                )
                .await
                .map_err(|e| e.to_string())
        }
    }
}

/// Two-stage model caller routing requests through ModelRouter to a ModelProvider (MDL-01, MDL-02).
pub struct RoutedModelCaller {
    pub router: std::sync::Arc<crate::model::router::resolver::ModelRouter>,
    pub provider: Option<std::sync::Arc<dyn crate::model::provider::ModelProvider>>,
    pub health_registry: std::sync::Arc<crate::model::router::health::CircuitBreakerRegistry>,
    pub candidates: Vec<crate::model::router::resolver::ModelCandidate>,
    pub catalog: Option<std::sync::Arc<crate::model::catalog::ModelCatalog>>,
    pub dynamic_catalog:
        Option<std::sync::Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>>>,
    pub configured_model: Option<String>,
    pub preferred_tier: crate::model::router::resolver::ModelTier,
    pub role: crate::state_machine::agent::AgentRole,
    pub tools: Vec<serde_json::Value>,
    pub configured_provider: String,
    pub provider_status: crate::model::types::ProviderCapabilityStatus,
}

impl RoutedModelCaller {
    /// Upper bound (seconds) honored from a provider `RateLimited` cooldown
    /// per caller invocation. Longer provider cooldowns still open the
    /// circuit (fast-fail instead of hammering); the sleep itself is capped
    /// so one throttled call can never stall a mission unboundedly.
    const MAX_RATE_LIMIT_SLEEP_SECS: u64 = 60;

    /// Sleep through a rate-limit cooldown, aborting early on cancellation.
    /// Returns true when the sleep completed (caller may retry once).
    /// Sleeps the capped cooldown plus a 1s margin so the circuit's
    /// half-open transition has observably passed on wake (avoids a
    /// wake-exactly-at-deadline race that would fail re-resolution).
    async fn sleep_rate_limit_cooldown(
        cooldown_secs: u64,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> bool {
        let capped = cooldown_secs
            .min(Self::MAX_RATE_LIMIT_SLEEP_SECS)
            .saturating_add(1);
        tokio::select! {
            _ = cancellation.cancelled() => false,
            _ = tokio::time::sleep(std::time::Duration::from_secs(capped)) => true,
        }
    }

    /// Resolve a model, waiting out a live circuit cooldown once when one is
    /// in effect (typically tripped by a concurrent invocation sharing this
    /// caller). Genuine misconfiguration — no cooling breakers — fails fast
    /// with the standard routing error.
    async fn resolve_with_cooldown_wait(
        &self,
        routing_req: &crate::model::router::resolver::RoutingRequest,
        candidates: &[crate::model::router::resolver::ModelCandidate],
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<crate::model::router::resolver::ResolvedModelSelection, String> {
        match self
            .router
            .resolve_model(routing_req, candidates, &self.health_registry)
        {
            Ok(selection) => Ok(selection),
            Err(e) => {
                if let Some(remaining) = self.health_registry.min_cooldown_remaining_secs()
                    && Self::sleep_rate_limit_cooldown(remaining, cancellation).await
                {
                    return self
                        .router
                        .resolve_model(routing_req, candidates, &self.health_registry)
                        .map_err(|e2| format!("model routing failed: {e2}"));
                }
                Err(format!("model routing failed: {e}"))
            }
        }
    }

    pub fn new(
        provider: Option<std::sync::Arc<dyn crate::model::provider::ModelProvider>>,
        preferred_tier: crate::model::router::resolver::ModelTier,
        tools: Vec<serde_json::Value>,
    ) -> Self {
        // Tier-5 environment is consumed ONLY by the configuration resolver.
        // This constructor never probes ambient env for model selection and
        // never fabricates capability metadata: without authoritative catalog
        // data there are no candidates and routing fails closed.
        let candidates = Vec::new();

        let provider_status = if provider.is_some() {
            crate::model::types::ProviderCapabilityStatus::Available
        } else {
            crate::model::types::ProviderCapabilityStatus::Misconfigured
        };

        Self {
            router: std::sync::Arc::new(crate::model::router::resolver::ModelRouter::new()),
            provider,
            health_registry: std::sync::Arc::new(
                crate::model::router::health::CircuitBreakerRegistry::default(),
            ),
            candidates,
            catalog: None,
            dynamic_catalog: None,
            configured_model: None,
            preferred_tier,
            role: crate::state_machine::agent::AgentRole::implementer(),
            tools,
            configured_provider: crate::config::canonical::CANONICAL_DEFAULT_PROVIDER.to_string(),
            provider_status,
        }
    }

    pub fn with_role(mut self, role: crate::state_machine::agent::AgentRole) -> Self {
        self.role = role;
        self
    }

    pub fn with_candidates(
        mut self,
        candidates: Vec<crate::model::router::resolver::ModelCandidate>,
    ) -> Self {
        self.candidates = candidates;
        self
    }

    pub fn with_catalog(
        mut self,
        catalog: std::sync::Arc<crate::model::catalog::ModelCatalog>,
    ) -> Self {
        self.candidates = catalog.models.clone();
        self.catalog = Some(catalog);
        self
    }

    pub fn with_catalog_lock(
        mut self,
        catalog: std::sync::Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>>,
    ) -> Self {
        self.dynamic_catalog = Some(catalog);
        self
    }

    pub fn with_configured_model(mut self, model: Option<String>) -> Self {
        self.configured_model = model;
        self
    }

    pub fn with_provider(
        mut self,
        provider: std::sync::Arc<dyn crate::model::provider::ModelProvider>,
    ) -> Self {
        self.provider = Some(provider);
        self.provider_status = crate::model::types::ProviderCapabilityStatus::Available;
        self
    }

    pub fn with_provider_status(
        mut self,
        provider_name: impl Into<String>,
        status: crate::model::types::ProviderCapabilityStatus,
    ) -> Self {
        self.configured_provider = provider_name.into();
        self.provider_status = status;
        self
    }

    pub fn provider_status(&self) -> crate::model::types::ProviderCapabilityStatus {
        self.provider_status
    }

    pub fn configured_provider(&self) -> &str {
        &self.configured_provider
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        use crate::model::router::resolver::ModelCandidate;
        let model_str = model.into();
        self.configured_model = Some(model_str.clone());

        // Precedence: authoritative catalog metadata first; otherwise the
        // configured model remains UNKNOWN (no fabricated 131K context, no
        // invented tool support, no tier claim). Unknown candidates fail
        // closed in the router when tool calling or context is required.
        if let Some(ref cat) = self.catalog
            && let Some(known) = cat.find_model(&model_str)
        {
            self.candidates = vec![known.clone()];
            return self;
        }

        let has_only_unknown = !self.candidates.is_empty()
            && self
                .candidates
                .iter()
                .all(|c| c.context_provenance() == Some("unknown"));

        if self.candidates.is_empty() || has_only_unknown {
            let provider = crate::config::provider_registry::PRODUCTION_PROVIDER_ID.to_string();
            let mut candidate = ModelCandidate::new_unknown(model_str, provider);
            let is_test = self.provider.as_ref().is_some_and(|p| p.is_test_double());
            if is_test {
                candidate.context_capacity = 131_072;
                candidate.supports_tools = true;
                candidate.tool_support = crate::model::CapabilitySupport::Supported;
                candidate.source = "test_fixture".to_string();
            } else {
                use crate::model::provider::nvidia_metadata::ProviderModelMetadataSource;
                let resolver = crate::model::provider::nvidia_metadata::NvidiaModelMetadataResolver::new_offline();
                let _ = futures::executor::block_on(resolver.enrich_candidate(&mut candidate));
            }
            self.candidates = vec![candidate];
        }
        self
    }

    /// Pure local candidate resolution.
    ///
    /// Read-only selection over already-known candidates with explicit
    /// priority: populated dynamic catalog → populated static catalog →
    /// statically configured fallback candidates. Never performs network
    /// I/O: when nothing is known the result is empty and the router fails
    /// closed with `NoEligibleModel`. Network discovery happens only through
    /// the explicit [`RoutedModelCaller::refresh_candidates_from_provider`]
    /// boundary, which is cancellable and returns a typed failure.
    async fn resolve_candidates(&self) -> Vec<crate::model::router::resolver::ModelCandidate> {
        // Priority 1: Populated dynamic catalog
        if let Some(ref dyn_cat) = self.dynamic_catalog {
            let guarded = dyn_cat.read().await;
            if !guarded.models.is_empty() {
                if let Some(ref pref) = self.configured_model {
                    let pref_available: Vec<_> = guarded
                        .models
                        .iter()
                        .filter(|c| {
                            c.model_id == *pref
                                && c.availability
                                    == crate::model::types::ProviderCapabilityStatus::Available
                        })
                        .cloned()
                        .collect();
                    if !pref_available.is_empty() {
                        return pref_available;
                    }
                }
                return guarded.models.clone();
            }
        }

        // Priority 2: Populated static catalog
        if let Some(ref cat) = self.catalog
            && !cat.models.is_empty()
        {
            if let Some(ref pref) = self.configured_model {
                let pref_available: Vec<_> = cat
                    .models
                    .iter()
                    .filter(|c| {
                        c.model_id == *pref
                            && c.availability
                                == crate::model::types::ProviderCapabilityStatus::Available
                    })
                    .cloned()
                    .collect();
                if !pref_available.is_empty() {
                    return pref_available;
                }
            }
            return cat.models.clone();
        }

        // Priority 3: Statically configured fallback candidates
        if let Some(ref pref) = self.configured_model {
            let pref_available: Vec<_> = self
                .candidates
                .iter()
                .filter(|c| {
                    c.model_id == *pref
                        && c.availability
                            == crate::model::types::ProviderCapabilityStatus::Available
                })
                .cloned()
                .collect();
            if !pref_available.is_empty() {
                return pref_available;
            }
        }

        self.candidates.clone()
    }

    /// Explicit model-catalog refresh boundary.
    ///
    /// The only resolution-adjacent path allowed to perform network
    /// discovery (`discover_models`). Refreshes the dynamic catalog (when
    /// attached) or returns freshly discovered candidates; honors
    /// cancellation; surfaces provider failures as a typed `Err` (never a
    /// silent empty list); never recurses into resolution.
    pub async fn refresh_candidates_from_provider(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<crate::model::router::resolver::ModelCandidate>, String> {
        if cancellation.is_cancelled() {
            return Err("model catalog refresh cancelled by runtime".to_string());
        }
        let provider = self.provider.as_ref().ok_or_else(|| {
            format!(
                "No model provider configured for '{}': catalog refresh unavailable",
                self.configured_provider
            )
        })?;
        let discovered = tokio::select! {
            _ = cancellation.cancelled() => {
                return Err("model catalog refresh cancelled by runtime".to_string());
            }
            res = provider.discover_models() => res.map_err(|e| e.to_string())?,
        };
        if let Some(ref dyn_cat) = self.dynamic_catalog {
            let mut cat_write = dyn_cat.write().await;
            cat_write.update_from_provider(&self.configured_provider, discovered.clone());
        }
        Ok(discovered)
    }
}

#[async_trait]
impl ModelCaller for RoutedModelCaller {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        let token = tokio_util::sync::CancellationToken::new();
        self.call_model_cancellable(context, &token).await
    }

    async fn call_model_cancellable(
        &self,
        context: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        // Single explicit seam: authoritative usage is preserved by
        // `call_model_cancellable_with_usage`; this proposal-only compatibility
        // entrypoint discards the usage record. Callers needing governance
        // accounting must use the with_usage variant so budget settlement and
        // telemetry stay authoritative.
        let (proposal, _usage) = self
            .call_model_cancellable_with_usage(context, cancellation)
            .await?;
        Ok(proposal)
    }

    async fn call_model_cancellable_with_usage(
        &self,
        context: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        // Tool-visibility authority: the governed tool set configured on this
        // caller is ALWAYS served here. Tool-free invocations (planning,
        // discovery, review, verification) MUST use the explicit
        // `call_model_tool_free_*` entrypoints; prompt text is never inspected
        // to decide tool visibility (the legacy `is_structured_prompt`
        // heuristic lives in `crate::runtime_authorities` for `Unspecified`
        // compatibility routing only).
        Self::routed_invoke(self, context, self.tools.clone(), cancellation).await
    }

    async fn call_model_tool_free_cancellable_with_usage(
        &self,
        context: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        // Explicitly tool-free: an empty schema set is served regardless of
        // the configured tools or the prompt content.
        Self::routed_invoke(self, context, Vec::new(), cancellation).await
    }

    async fn bound_catalog_snapshot(&self) -> Option<crate::model::catalog::ModelCatalog> {
        match self.dynamic_catalog.as_ref() {
            Some(lock) => Some(lock.read().await.clone()),
            None => None,
        }
    }

    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        // Single explicit seam: discard usage record for compatibility caller.
        let (proposal, _usage) = self
            .call_model_with_context_and_usage(compiled, cancellation)
            .await?;
        Ok(proposal)
    }

    async fn call_model_with_context_and_usage(
        &self,
        compiled: &CompiledContext,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        self.call_model_with_context_and_usage_streaming(compiled, cancellation, None)
            .await
    }

    async fn call_model_with_context_and_usage_streaming(
        &self,
        compiled: &CompiledContext,
        cancellation: &tokio_util::sync::CancellationToken,
        chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::model::types::StreamChunk>>,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        if cancellation.is_cancelled() {
            return Err("model invocation cancelled by runtime".to_string());
        }
        if self.provider_status == crate::model::types::ProviderCapabilityStatus::Unavailable {
            return Err(format!(
                "configured provider '{}' is UNAVAILABLE: only NVIDIA NIM is production-supported in M31A",
                self.configured_provider
            ));
        }
        if self.provider_status == crate::model::types::ProviderCapabilityStatus::Misconfigured {
            return Err(format!(
                "configured provider '{}' is MISCONFIGURED: missing or invalid credentials / configuration",
                self.configured_provider
            ));
        }

        let provider = self.provider.as_ref().ok_or_else(|| {
            format!(
                "No model provider configured for '{}': set NVIDIA_API_KEY or inject a ModelCaller via with_model_caller()",
                self.configured_provider
            )
        })?;

        let candidates = self.resolve_candidates().await;

        let routing_req = crate::model::router::resolver::RoutingRequest::new(
            self.role.clone(),
            self.preferred_tier,
        )
        .with_tool_calling(!self.tools.is_empty());

        // Same bounded rate-limit retry contract as call_model_cancellable:
        // at most two provider attempts with one capped, cancellable
        // cooldown sleep between them.
        let mut last_err = String::new();
        for attempt in 0..2 {
            let selection = self
                .resolve_with_cooldown_wait(&routing_req, &candidates, cancellation)
                .await?;

            let call_result = if !compiled.messages.is_empty() {
                provider
                    .call_model_with_messages_streaming(
                        &selection.model_name,
                        &compiled.messages,
                        self.tools.clone(),
                        cancellation,
                        chunk_tx.clone(),
                    )
                    .await
            } else {
                provider
                    .call_model(
                        &selection.model_name,
                        &compiled.system_prompt,
                        self.tools.clone(),
                        cancellation,
                    )
                    .await
            };

            match call_result {
                Ok((proposal, usage)) => {
                    self.health_registry
                        .record_success(&selection.provider, &selection.model_name);
                    return Ok((proposal, usage));
                }
                Err(crate::model::types::ModelError::RateLimited { cooldown_secs })
                    if attempt == 0 =>
                {
                    self.health_registry.record_failure(
                        &selection.provider,
                        &selection.model_name,
                        &crate::model::types::ModelError::RateLimited { cooldown_secs },
                    );
                    if Self::sleep_rate_limit_cooldown(cooldown_secs, cancellation).await {
                        continue;
                    }
                    return Err(crate::model::types::ModelError::Cancelled.to_string());
                }
                Err(e) => {
                    self.health_registry.record_failure(
                        &selection.provider,
                        &selection.model_name,
                        &e,
                    );
                    last_err = e.to_string();
                    break;
                }
            }
        }
        Err(last_err)
    }
}

impl RoutedModelCaller {
    /// Shared routed invocation serving an EXPLICIT tool set.
    ///
    /// Governed callers pass their configured schemas; tool-free planning /
    /// discovery / review / verification callers pass an empty set. The tool
    /// set is a typed parameter — never inferred from prompt text.
    async fn routed_invoke(
        caller: &RoutedModelCaller,
        context: &str,
        effective_tools: Vec<serde_json::Value>,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        if cancellation.is_cancelled() {
            return Err("model invocation cancelled by runtime".to_string());
        }
        if caller.provider_status == crate::model::types::ProviderCapabilityStatus::Unavailable {
            return Err(format!(
                "configured provider '{}' is UNAVAILABLE: only NVIDIA NIM is production-supported in M31A",
                caller.configured_provider
            ));
        }
        if caller.provider_status == crate::model::types::ProviderCapabilityStatus::Misconfigured {
            return Err(format!(
                "configured provider '{}' is MISCONFIGURED: missing or invalid credentials / configuration",
                caller.configured_provider
            ));
        }

        let provider = caller.provider.as_ref().ok_or_else(|| {
            format!(
                "No model provider configured for '{}': set NVIDIA_API_KEY or inject a ModelCaller via with_model_caller()",
                caller.configured_provider
            )
        })?;

        let candidates = caller.resolve_candidates().await;

        let routing_req = crate::model::router::resolver::RoutingRequest::new(
            caller.role.clone(),
            caller.preferred_tier,
        )
        .with_tool_calling(!effective_tools.is_empty());

        // At most two provider attempts: on a live `RateLimited` response the
        // caller honors the provider cooldown once (capped, cancellable) and
        // retries through the circuit's half-open probe. Any second failure
        // — or a non-rate-limit error — is recorded and returned. Error
        // strings surfaced to callers are unchanged.
        let mut last_err = String::new();
        for attempt in 0..2 {
            let selection = caller
                .resolve_with_cooldown_wait(&routing_req, &candidates, cancellation)
                .await?;

            // Circuit-breaker accounting: record the typed provider outcome
            // so the health registry reflects reality. 429 rate limits open a
            // cooldown (fast-fail instead of hammering); 4xx client errors are
            // non-degrading by design; success resets to Healthy.
            match provider
                .call_model(
                    &selection.model_name,
                    context,
                    effective_tools.clone(),
                    cancellation,
                )
                .await
            {
                Ok((proposal, usage)) => {
                    caller
                        .health_registry
                        .record_success(&selection.provider, &selection.model_name);
                    return Ok((proposal, usage));
                }
                Err(crate::model::types::ModelError::RateLimited { cooldown_secs })
                    if attempt == 0 =>
                {
                    caller.health_registry.record_failure(
                        &selection.provider,
                        &selection.model_name,
                        &crate::model::types::ModelError::RateLimited { cooldown_secs },
                    );
                    if Self::sleep_rate_limit_cooldown(cooldown_secs, cancellation).await {
                        continue;
                    }
                    return Err(crate::model::types::ModelError::Cancelled.to_string());
                }
                Err(e) => {
                    caller.health_registry.record_failure(
                        &selection.provider,
                        &selection.model_name,
                        &e,
                    );
                    last_err = e.to_string();
                    break;
                }
            }
        }
        Err(last_err)
    }
}

/// Explicit mock model caller for unit tests.
#[derive(Debug, Clone)]
pub struct TestModelCaller {
    pub proposals: std::sync::Arc<tokio::sync::Mutex<Vec<Result<ModelProposal, String>>>>,
    pub default_summary: String,
}

impl TestModelCaller {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            proposals: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
            default_summary: summary.into(),
        }
    }

    pub fn from_proposals(proposals: Vec<Result<ModelProposal, String>>) -> Self {
        Self {
            proposals: std::sync::Arc::new(tokio::sync::Mutex::new(proposals)),
            default_summary: "completed".to_string(),
        }
    }

    pub fn with_proposal(proposal: ModelProposal) -> Self {
        Self {
            proposals: std::sync::Arc::new(tokio::sync::Mutex::new(vec![Ok(proposal)])),
            default_summary: "completed".to_string(),
        }
    }
}

#[async_trait]
impl ModelCaller for TestModelCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        let mut queue = self.proposals.lock().await;
        if !queue.is_empty() {
            queue.remove(0)
        } else {
            Ok(ModelProposal::Complete {
                summary: self.default_summary.clone(),
                artifacts: Vec::new(),
            })
        }
    }
}

/// Deterministic test double for offline unit and state-machine tests.
///
/// Dispatches valid canned structured responses matching M31A prompt contracts
/// (`genesis.dynamic_questions`, `planning.decompose`, `planning.revision`, `planning.task_revision`).
/// Never used in live-model acceptance tests.
#[derive(Debug, Default, Clone)]
pub struct DeterministicLifecycleModelCaller {
    pub fail_next: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl DeterministicLifecycleModelCaller {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing() -> Self {
        let caller = Self::default();
        caller
            .fail_next
            .store(true, std::sync::atomic::Ordering::SeqCst);
        caller
    }
}

#[async_trait]
impl ModelCaller for DeterministicLifecycleModelCaller {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        if self
            .fail_next
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err("Injected deterministic model failure".to_string());
        }

        if context.contains("Discovery Analyst")
            || context.contains("Dynamic Socratic Questioning")
            || context.contains("genesis.dynamic_questions")
            || context.contains("Dynamic Questions")
            || context.contains("Unresolved Unknowns")
            || context.contains("UNRESOLVED UNKNOWNS")
        {
            if context.contains("unk_arch_choice") {
                let questions = r#"[
                    {
                        "question_id": "q_unk_arch_choice",
                        "text": "What is the intended preference or architecture?",
                        "target_unknown": "unk_arch_choice",
                        "reason": "Application architecture and deployment boundaries are unspecified",
                        "options": ["Lightweight", "Full-stack"]
                    }
                ]"#;
                return Ok(ModelProposal::Complete {
                    summary: questions.to_string(),
                    artifacts: Vec::new(),
                });
            } else {
                return Ok(ModelProposal::Complete {
                    summary: "[]".to_string(),
                    artifacts: Vec::new(),
                });
            }
        }

        if context.contains("planning.revision") || context.contains("Lead Planner (Plan Revision)")
        {
            let revised = r#"{
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Implement requested functionality with revisions",
                        "description": "Deterministic revised task with user feedback incorporated",
                        "depends_on": [],
                        "role": "implementer",
                        "required_capabilities": ["fs.write"],
                        "verification": "compilation"
                    }
                ]
            }"#;
            return Ok(ModelProposal::Complete {
                summary: revised.to_string(),
                artifacts: Vec::new(),
            });
        }

        if context.contains("planning.task_revision")
            || context.contains("Task Decomposition Planner")
        {
            let tasks = r#"[
                {
                    "id": "TASK-01",
                    "objective": "Implement regenerated candidate task",
                    "description": "Deterministic regenerated candidate task",
                    "role": "implementer",
                    "capabilities": ["fs.write"],
                    "verification": "compilation",
                    "resource_estimate": { "estimated_tokens": 100, "estimated_duration_seconds": 10 }
                }
            ]"#;
            return Ok(ModelProposal::Complete {
                summary: tasks.to_string(),
                artifacts: Vec::new(),
            });
        }

        // Default: planning.decompose
        let decomp = r#"{
            "tasks": [
                {
                    "id": "TASK-01",
                    "title": "Execute mission objective",
                    "description": "Standard deterministic implementation task",
                    "depends_on": [],
                    "role": "implementer",
                    "required_capabilities": ["fs.write"],
                    "verification": "compilation"
                }
            ]
        }"#;
        Ok(ModelProposal::Complete {
            summary: decomp.to_string(),
            artifacts: Vec::new(),
        })
    }
}

/// Error raised when an agent exceeds its metered step budget (D-03, AGT-05).
#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[error("step limit exceeded: limit={limit}, consumed={consumed}")]
pub struct StepLimitExceeded {
    pub limit: u32,
    pub consumed: u32,
}

/// Two-tier step budget tracking model turns and tool executions separately (D-03).
///
/// 1 Step = 1 model decision cycle (model request + validation + action execution).
/// Tool calls are metered separately and do not increment or reset the step count.
///
/// AUTHORITY CONTRACT: this is a LOCAL per-agent turn guard enforcing the
/// role profile's immutable step ceiling within one run. It is NOT an
/// admission authority: token/cost/worker/artifact admission lives solely in
/// [`BudgetEnforcer`](crate::budget::enforcer::BudgetEnforcer), whose
/// settlement records the steps consumed here. It must never independently
/// allow work the enforcer denied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepBudget {
    max_steps: u32,
    steps_consumed: u32,
    tool_calls_consumed: u32,
}

impl StepBudget {
    /// Create a new step budget with a given maximum step limit.
    pub fn new(max_steps: u32) -> Self {
        Self {
            max_steps,
            steps_consumed: 0,
            tool_calls_consumed: 0,
        }
    }

    /// Pre-step admission check. Fails closed if steps consumed reached or exceeded limit.
    pub fn check_admission(&self) -> Result<(), StepLimitExceeded> {
        if self.steps_consumed >= self.max_steps {
            Err(StepLimitExceeded {
                limit: self.max_steps,
                consumed: self.steps_consumed,
            })
        } else {
            Ok(())
        }
    }

    /// Record consumption of one model turn step.
    pub fn record_step(&mut self) {
        self.steps_consumed = self.steps_consumed.saturating_add(1);
    }

    /// Record consumption of one tool call action (metered separately from steps).
    pub fn record_tool_call(&mut self) {
        self.tool_calls_consumed = self.tool_calls_consumed.saturating_add(1);
    }

    /// Current number of model turn steps consumed.
    pub fn steps_consumed(&self) -> u32 {
        self.steps_consumed
    }

    /// Current number of tool calls consumed.
    pub fn tool_calls_consumed(&self) -> u32 {
        self.tool_calls_consumed
    }

    /// Configured maximum step ceiling.
    pub fn max_steps(&self) -> u32 {
        self.max_steps
    }

    /// Remaining steps before ceiling is reached.
    pub fn remaining_steps(&self) -> u32 {
        self.max_steps.saturating_sub(self.steps_consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_budget_metering_and_admission() {
        let mut budget = StepBudget::new(3);
        assert_eq!(budget.remaining_steps(), 3);
        assert!(budget.check_admission().is_ok());

        budget.record_tool_call();
        budget.record_tool_call();
        assert_eq!(budget.tool_calls_consumed(), 2);
        assert_eq!(budget.steps_consumed(), 0);
        assert!(budget.check_admission().is_ok());

        budget.record_step();
        assert_eq!(budget.steps_consumed(), 1);
        assert_eq!(budget.remaining_steps(), 2);
        assert!(budget.check_admission().is_ok());

        budget.record_step();
        budget.record_step();
        assert_eq!(budget.steps_consumed(), 3);
        assert_eq!(budget.remaining_steps(), 0);

        let err = budget.check_admission().unwrap_err();
        assert_eq!(
            err,
            StepLimitExceeded {
                limit: 3,
                consumed: 3
            }
        );
    }

    /// Deterministic contract test: the routed caller records
    /// typed provider outcomes in the circuit registry — 429 opens a
    /// cooldown, success resets to Healthy. The caller honors a 1s test
    /// cooldown once, then surfaces the second failure. No network;
    /// MockProvider only.
    #[tokio::test]
    async fn test_routed_caller_records_circuit_outcomes() {
        use crate::model::provider::mock::MockProvider;
        use crate::model::router::resolver::{ModelCandidate, ModelTier};
        use crate::model::types::ModelError;
        use std::sync::Arc;

        let mock = Arc::new(MockProvider::new());
        mock.push_response(Err(ModelError::RateLimited { cooldown_secs: 1 }))
            .await;
        mock.push_response(Err(ModelError::RateLimited { cooldown_secs: 1 }))
            .await;
        // Explicit authoritative test metadata (not a fallback fabrication).
        let caller = RoutedModelCaller::new(
            Some(mock.clone()),
            crate::model::router::resolver::ModelTier::Standard,
            Vec::new(),
        )
        .with_candidates(vec![ModelCandidate::new(
            "test-model",
            crate::config::provider_registry::PRODUCTION_PROVIDER_ID,
            ModelTier::Standard,
            8192,
        )]);

        let token = tokio_util::sync::CancellationToken::new();
        let first = caller.call_model_cancellable("ctx", &token).await;
        let err = first.unwrap_err();
        assert!(err.contains("rate limit"), "unexpected error: {err}");
        // Both provider attempts consumed (initial + one cooldown-honoring retry).
        assert_eq!(mock.recorded_calls().await.len(), 2);
        assert_eq!(
            caller.health_registry.get_state_for_model("test-model"),
            crate::model::router::health::CircuitState::Open,
            "429 must open the model circuit"
        );

        // The 1s test cooldown has expired by now, so the next call is
        // admitted as a half-open probe, succeeds on the mock default, and
        // resets the circuit to Healthy — the full breaker lifecycle.
        let second = caller.call_model_cancellable("ctx", &token).await;
        assert!(
            second.is_ok(),
            "expired cooldown must re-admit the model, got: {second:?}"
        );
        assert_eq!(
            caller.health_registry.get_state_for_model("test-model"),
            crate::model::router::health::CircuitState::Healthy,
            "success must reset the circuit"
        );
    }

    /// Deterministic contract test: when a concurrent invocation
    /// tripped the breaker, a new caller invocation waits out the live
    /// cooldown once and proceeds (exactly one provider call — no hammering
    /// during cooldown, no instant failure).
    #[tokio::test]
    async fn test_routed_caller_waits_out_live_cooldown_on_resolve() {
        use crate::model::provider::mock::MockProvider;
        use crate::model::router::resolver::{ModelCandidate, ModelTier};
        use crate::model::types::ModelError;
        use std::sync::Arc;

        let mock = Arc::new(MockProvider::new());
        let caller = RoutedModelCaller::new(
            Some(mock.clone()),
            crate::model::router::resolver::ModelTier::Standard,
            Vec::new(),
        )
        .with_candidates(vec![ModelCandidate::new(
            "test-model",
            crate::config::provider_registry::PRODUCTION_PROVIDER_ID,
            ModelTier::Standard,
            8192,
        )]);

        // Simulate a concurrent invocation's 429 with a 1s cooldown.
        caller.health_registry.record_failure(
            crate::config::provider_registry::PRODUCTION_PROVIDER_ID,
            "test-model",
            &ModelError::RateLimited { cooldown_secs: 1 },
        );

        let token = tokio_util::sync::CancellationToken::new();
        let out = caller.call_model_cancellable("ctx", &token).await;
        assert!(
            out.is_ok(),
            "live cooldown must be waited out, got: {out:?}"
        );
        assert_eq!(mock.recorded_calls().await.len(), 1);
    }

    /// Regression: an arbitrary configured model must NEVER silently become a
    /// 131K context, tool-capable NVIDIA model. Unknown metadata stays unknown
    /// and routing fails closed when capabilities are required.
    #[tokio::test]
    async fn arbitrary_model_does_not_fabricate_capabilities() {
        use crate::model::router::resolver::{CapabilitySupport, RoutingRequest};
        use crate::model::router::{health::CircuitBreakerRegistry, resolver::ModelTier};

        let caller = RoutedModelCaller::new(
            None,
            ModelTier::Standard,
            vec![serde_json::json!({"type": "function"})],
        )
        .with_model("arbitrary-operator-model-xyz");

        assert_eq!(caller.candidates.len(), 1);
        let cand = &caller.candidates[0];
        assert_eq!(cand.model_id, "arbitrary-operator-model-xyz");
        assert_ne!(
            cand.context_capacity, 131072,
            "fabricated 131K context must not appear"
        );
        assert_eq!(cand.context_capacity, 0);
        assert_eq!(cand.context_provenance(), Some("unknown"));
        assert_eq!(cand.tool_support, CapabilitySupport::Unknown);
        assert!(!cand.supports_tools);

        // Router fails closed when tool calling is required.
        let router = crate::model::router::resolver::ModelRouter::new();
        let health = CircuitBreakerRegistry::default();
        let req = RoutingRequest::new(
            crate::state_machine::agent::AgentRole::implementer(),
            ModelTier::Standard,
        )
        .with_tool_calling(true);
        let err = router
            .resolve_model(&req, &caller.candidates, &health)
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown/unverified")
                || err.to_string().contains("context")
                || err.to_string().contains("NoEligibleModel")
                || err.to_string().contains("no available model"),
            "unknown model must fail closed, got: {err}"
        );
    }
}
