//! Single authoritative configuration defaults (CFG-01).
//!
//! This module is the ONLY place where ordinary runtime defaults live.
//! Every other subsystem consumes these values via the configuration
//! resolver ([`crate::config::ResolvedConfiguration`]) — never by
//! re-stating the literal.
//!
//! Split of responsibilities:
//! - Configurable defaults → here (consumed by `schema.rs` `default_*` fns).
//! - Immutable security floors / protocol constants → remain in code at
//!   their owning security layer (e.g. endpoint trust, egress policy,
//!   sandbox ceilings, query ceilings).
//! - Serialization compatibility defaults → near the schema.
//!
//! Lower layers MUST NOT `unwrap_or("<literal>")` for any value listed here.
//! They receive the resolved value from `ResolvedConfiguration` or fail closed.

/// Canonical default primary model.
///
/// Single authority for the ordinary default model. Consumed by
/// `schema::default_model`, `ProviderConfig::default`, and — via the
/// resolver — by every runtime consumer (dispatcher, registry, TUI).
pub const CANONICAL_DEFAULT_MODEL: &str = "meta/llama-3.2-11b-vision-instruct";

/// Canonical default provider id (production).
pub const CANONICAL_DEFAULT_PROVIDER: &str = "nvidia_nim";

/// Canonical production NVIDIA endpoint.
///
/// Immutable production identity. The endpoint *trust* authority remains
/// [`crate::model::provider::endpoint`] (it re-exports this constant);
/// this module is the single place the URL string is stated.
pub const CANONICAL_NVIDIA_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

// ── Timeout hierarchy (seconds unless noted) ──────────────────────────────
// Explicit scopes; transport timeouts are separate from workflow semantics.

/// Mission/agent wall-clock default (runtime timeout).
pub const DEFAULT_RUNTIME_TIMEOUT_SECS: u64 = 300;
/// Workflow step default.
pub const DEFAULT_WORKFLOW_STEP_TIMEOUT_SECS: u64 = 600;
/// Verification executor default.
pub const DEFAULT_VERIFICATION_TIMEOUT_SECS: u64 = 60;
/// Local verification capability default.
pub const DEFAULT_LOCAL_VERIFICATION_TIMEOUT_SECS: u64 = 300;
/// Process supervisor foreground default.
pub const DEFAULT_PROCESS_TIMEOUT_SECS: u64 = 30;
/// Approval wait default.
pub const DEFAULT_APPROVAL_TIMEOUT_SECS: u64 = 60;
/// Git command default.
pub const DEFAULT_GIT_TIMEOUT_SECS: u64 = 120;
/// NVIDIA HTTP transport default.
pub const DEFAULT_NVIDIA_HTTP_TIMEOUT_SECS: u64 = 300;
/// Remote metadata HTTP / connect timeouts (transport-level).
pub const DEFAULT_METADATA_HTTP_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_METADATA_CONNECT_TIMEOUT_SECS: u64 = 5;
/// Provider probe default.
pub const DEFAULT_PROVIDER_PROBE_TIMEOUT_SECS: u64 = 5;
/// Git authorization TTL default.
pub const DEFAULT_GIT_AUTH_TTL_SECS: u64 = 600;
/// Generic outbound HTTP transport default (non-NVIDIA).
pub const DEFAULT_OUTBOUND_HTTP_TIMEOUT_SECS: u64 = 30;
/// Workflow default max retries.
pub const DEFAULT_WORKFLOW_MAX_RETRIES: usize = 3;

// ── Concurrency / resource defaults ─────────────────────────────────────────

/// Runtime global concurrency default.
pub const DEFAULT_RUNTIME_CONCURRENCY: usize = 4;
/// Scheduler global workers default.
pub const DEFAULT_SCHEDULER_WORKERS: usize = 8;
/// Per-role concurrency default.
pub const DEFAULT_PER_ROLE_CONCURRENCY: usize = 2;
/// Global job admission default.
pub const DEFAULT_MAX_GLOBAL_JOBS: usize = 16;
/// Per-mission job admission default.
pub const DEFAULT_MAX_PER_MISSION_JOBS: usize = 4;
/// Research concurrency default.
pub const DEFAULT_RESEARCH_CONCURRENCY: usize = 4;
/// Metadata fetch concurrency default.
pub const DEFAULT_METADATA_CONCURRENCY: usize = 8;
/// Tool default execution bound.
pub const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 30;
/// Tool default max output (bytes).
pub const DEFAULT_TOOL_MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Canonical default TUI action channel capacity (bounded backpressure).
pub const DEFAULT_TUI_ACTION_CHANNEL_CAPACITY: usize = 256;
/// Canonical default TUI interaction event channel capacity (bounded backpressure).
pub const DEFAULT_TUI_INTERACTION_CHANNEL_CAPACITY: usize = 1024;
/// Maximum events drained per TUI frame tick to prevent frame starvation.
pub const DEFAULT_TUI_MAX_EVENTS_PER_TICK: usize = 128;

// ── Repo query operator defaults (ceilings stay in `repo::query`) ───────────

/// Operator-configurable repo query defaults (clamped by immutable ceilings).
pub const DEFAULT_QUERY_MAX_RESULTS: usize = 50;
pub const DEFAULT_QUERY_MAX_DEPTH: usize = 2;
pub const DEFAULT_QUERY_MAX_BYTES: usize = 64 * 1024;

// ── Model catalog cache policy defaults ─────────────────────────────────────

// ── Agent / discovery / TUI operator defaults ─────────────────────────────
// These are operator-configurable via the schema (`AppConfig`) and resolved
// through `ResolvedConfiguration`; subsystems consume the resolved value.

/// Agent default max tokens (operator-configurable via `agents.max_tokens`).
pub const DEFAULT_AGENT_MAX_TOKENS: u32 = 8192;
/// TUI frame-rate default (operator-configurable via `tui.fps`).
pub const DEFAULT_TUI_FPS: u32 = 30;
/// Discovery default: max interactive question turns (`workflow.max_discovery_turns`).
pub const DEFAULT_MAX_DISCOVERY_TURNS: usize = 4;
/// Discovery default: ambiguity threshold percent (`workflow.ambiguity_threshold_percent`).
pub const DEFAULT_AMBIGUITY_THRESHOLD_PERCENT: u8 = 15;

// ── Operator-facing string/enum defaults ────────────────────────────────────
// Single authority for the string spellings below; schema `default_*` fns,
// the wizard prefill, and TUI theme persistence all consume these.

/// Default sandbox mode (`runtime.sandbox_mode`).
pub const DEFAULT_SANDBOX_MODE: &str = "standard";
/// Default mission branch prefix (`git.branch_prefix`).
pub const DEFAULT_BRANCH_PREFIX: &str = "m31a/mission";
/// Default TUI theme id (`tui.theme`).
pub const DEFAULT_TUI_THEME: &str = "dark-slate-cyan";
/// Default planning projection directory (`workflow.projection_dir`).
pub const DEFAULT_PROJECTION_DIR: &str = ".planning";

/// Local catalog freshness default (seconds).
pub const DEFAULT_CATALOG_FRESHNESS_SECS: u64 = 3600;
/// Remote metadata TTL default (seconds).
pub const DEFAULT_REMOTE_METADATA_TTL_SECS: u64 = 86400;

/// Typed timeout hierarchy resolved from configuration.
///
/// Each field has an explicit scope. Subsystems consume the resolved value;
/// they never hardcode their own ordinary default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutPolicy {
    /// Mission/agent wall clock.
    pub runtime_secs: u64,
    /// Workflow step timeout.
    pub workflow_step_secs: u64,
    /// Verification timeout.
    pub verification_secs: u64,
    /// Process/tool timeout.
    pub process_secs: u64,
    /// Approval wait timeout.
    pub approval_secs: u64,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            runtime_secs: DEFAULT_RUNTIME_TIMEOUT_SECS,
            workflow_step_secs: DEFAULT_WORKFLOW_STEP_TIMEOUT_SECS,
            verification_secs: DEFAULT_VERIFICATION_TIMEOUT_SECS,
            process_secs: DEFAULT_PROCESS_TIMEOUT_SECS,
            approval_secs: DEFAULT_APPROVAL_TIMEOUT_SECS,
        }
    }
}

/// Typed resource policy resolved from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePolicy {
    pub runtime_concurrency: usize,
    pub mission_concurrency: usize,
    pub per_role_concurrency: usize,
    pub process_concurrency: usize,
    pub research_concurrency: usize,
    pub metadata_concurrency: usize,
    pub tool_timeout_secs: u64,
    pub tool_max_output_bytes: usize,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self {
            runtime_concurrency: DEFAULT_RUNTIME_CONCURRENCY,
            mission_concurrency: DEFAULT_MAX_PER_MISSION_JOBS,
            per_role_concurrency: DEFAULT_PER_ROLE_CONCURRENCY,
            process_concurrency: DEFAULT_MAX_GLOBAL_JOBS,
            research_concurrency: DEFAULT_RESEARCH_CONCURRENCY,
            metadata_concurrency: DEFAULT_METADATA_CONCURRENCY,
            tool_timeout_secs: DEFAULT_TOOL_TIMEOUT_SECS,
            tool_max_output_bytes: DEFAULT_TOOL_MAX_OUTPUT_BYTES,
        }
    }
}

/// Typed model-catalog cache policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCatalogCachePolicy {
    pub local_freshness_secs: u64,
    pub remote_metadata_ttl_secs: u64,
    pub refresh_on_start: bool,
    pub refresh_on_demand: bool,
}

impl Default for ModelCatalogCachePolicy {
    fn default() -> Self {
        Self {
            local_freshness_secs: DEFAULT_CATALOG_FRESHNESS_SECS,
            remote_metadata_ttl_secs: DEFAULT_REMOTE_METADATA_TTL_SECS,
            refresh_on_start: false,
            refresh_on_demand: true,
        }
    }
}
