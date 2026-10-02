//! Hierarchical Configuration Engine & Validation (CFG-01–CFG-04, D-12–D-14).

pub mod env;
pub mod hierarchy;
pub mod merge;
pub mod paths;
pub mod profile;
pub mod provenance;
pub mod provider_registry;
pub mod resolved;
pub mod schema;

pub use env::{SafeEnvironmentStatus, load_dotenv, load_dotenv_from_workspace};
pub use hierarchy::{ConfigPrecedenceEngine, ConfigTier};
pub use merge::{ConfigError, MonotonicSecurityMerger, deep_merge_toml};
pub use paths::PlatformPaths;
pub use profile::{ProfileConfig, ProfileResolver};
pub use provenance::{ConfigLayer, ConfigurationService, ProvenanceError, ResolvedValue};
pub use provider_registry::{
    MaskedSecret, NVIDIA_ONLY_ERROR, PRODUCTION_PROVIDER_ID, ProviderDescriptor, ProviderError,
    ProviderRegistry, ProviderType, is_retired_provider, normalize_provider_id,
};
pub use resolved::{
    ConfigExplain, ResolvedConfigBuilder, ResolvedConfiguration, TierOverrideRecord, is_secret_key,
    mask_value,
};
pub use schema::{
    AgentsConfig, AppConfig, BudgetConfig, ConfigValidationError, GitConfig, PolicyConfig,
    ProviderConfig, ProviderTableConfig, RuntimeConfig, TuiConfig, WorkflowConfig, WorkspaceConfig,
    WorkspaceVerificationConfig, parse_and_validate_config, validate_config,
};
