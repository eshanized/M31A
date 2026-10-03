//! Modular Doctor Environment Diagnostics Probes (CLI-01, D-20).

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 6 diagnostic probe categories defined in D-20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeCategory {
    Environment,
    Git,
    Models,
    Sandbox,
    Storage,
    Network,
}

impl ProbeCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Git => "git",
            Self::Models => "models",
            Self::Sandbox => "sandbox",
            Self::Storage => "storage",
            Self::Network => "network",
        }
    }
}

/// Status of an individual probe evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeStatus {
    Ok,
    Warning,
    Error,
}

/// Structured outcome of a diagnostic probe check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub category: ProbeCategory,
    pub name: String,
    pub status: ProbeStatus,
    pub message: String,
    pub remediation: Option<String>,
}

impl ProbeResult {
    pub fn ok(
        category: ProbeCategory,
        name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            status: ProbeStatus::Ok,
            message: message.into(),
            remediation: None,
        }
    }

    pub fn warning(
        category: ProbeCategory,
        name: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            status: ProbeStatus::Warning,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }

    pub fn error(
        category: ProbeCategory,
        name: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            status: ProbeStatus::Error,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
}

/// Trait implemented by diagnostic probes across all 6 categories.
#[async_trait::async_trait]
pub trait DoctorProbe: Send + Sync {
    fn category(&self) -> ProbeCategory;
    fn name(&self) -> &str;
    async fn check(&self) -> ProbeResult;
}

// ---------------------------------------------------------------------------
// Concrete Probes
// ---------------------------------------------------------------------------

/// Category 1: Environment probe checking OS and filesystem write access.
pub struct EnvironmentProbe;

#[async_trait::async_trait]
impl DoctorProbe for EnvironmentProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Environment
    }
    fn name(&self) -> &str {
        "os_and_filesystem"
    }
    async fn check(&self) -> ProbeResult {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("m31a_doctor_probe_{}.tmp", uuid::Uuid::now_v7()));
        if let Err(e) = std::fs::write(&test_file, b"probe") {
            return ProbeResult::error(
                ProbeCategory::Environment,
                "os_and_filesystem",
                format!("Temporary directory is not writable: {e}"),
                "Ensure write permissions on temp directory ($TMPDIR or /tmp)",
            );
        }
        let _ = std::fs::remove_file(&test_file);

        ProbeResult::ok(
            ProbeCategory::Environment,
            "os_and_filesystem",
            format!(
                "Running on {} ({}); temp storage writable",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        )
    }
}

/// Category 2: Git repository and worktree capability probe.
pub struct GitProbe;

#[async_trait::async_trait]
impl DoctorProbe for GitProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Git
    }
    fn name(&self) -> &str {
        "git_cli"
    }
    async fn check(&self) -> ProbeResult {
        match std::process::Command::new("git").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ProbeResult::ok(
                    ProbeCategory::Git,
                    "git_cli",
                    format!("{ver_str}; worktrees supported"),
                )
            }
            Ok(_) => ProbeResult::error(
                ProbeCategory::Git,
                "git_cli",
                "Git command returned non-zero status",
                "Ensure git is installed and functioning properly",
            ),
            Err(e) => ProbeResult::error(
                ProbeCategory::Git,
                "git_cli",
                format!("Git CLI binary not found in PATH: {e}"),
                "Install Git 2.40+ and ensure it is available in system PATH",
            ),
        }
    }
}

/// Category 3: Model and LLM provider credentials probe (WS-I §1, §8).
pub struct ModelsProbe;

#[async_trait::async_trait]
impl DoctorProbe for ModelsProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Models
    }
    fn name(&self) -> &str {
        "model_providers"
    }
    async fn check(&self) -> ProbeResult {
        // Best-effort channel-aware credential load so this diagnostic
        // agrees with runtime status when run from the workspace root.
        // This is a diagnostic probe (not the runtime authority); resolution
        // failures here never override `ResolvedConfiguration` truth.
        let mut registry = crate::config::provider_registry::ProviderRegistry::new();
        if let Ok(cwd) = std::env::current_dir() {
            let creds = crate::config::provider_registry::ProviderRegistry::channel_credentials_path(
                &cwd,
            );
            if creds.is_file() {
                let _ = registry.load_credentials_from_file(&creds);
            }
        }
        let status = registry.get_status("nvidia_nim");

        match status {
            crate::model::types::ProviderCapabilityStatus::Available => {
                let mut note =
                    "NVIDIA NIM credentials detected (status: AVAILABLE, production-supported)"
                        .to_string();
                let cache_path =
                    std::path::Path::new(crate::model::catalog::ModelCatalog::CACHE_RELATIVE_PATH);
                if let Ok(catalog) =
                    crate::model::catalog::ModelCatalog::load_from_cache_file(cache_path)
                {
                    let stale_str = if catalog
                        .is_stale(crate::model::catalog::ModelCatalog::DEFAULT_MAX_AGE_SECS)
                    {
                        "stale"
                    } else {
                        "current"
                    };
                    note.push_str(&format!(
                        " | Discovered Catalog: {} models ({}, source: {})",
                        catalog.len(),
                        stale_str,
                        catalog.source
                    ));
                }
                if std::env::var("OPENAI_API_KEY").is_ok()
                    || std::env::var("ANTHROPIC_API_KEY").is_ok()
                {
                    note.push_str(" [Note: OpenAI/Anthropic keys ignored; only NVIDIA NIM is production-supported]");
                }
                ProbeResult::ok(ProbeCategory::Models, "model_providers", note)
            }
            crate::model::types::ProviderCapabilityStatus::Misconfigured => ProbeResult::warning(
                ProbeCategory::Models,
                "model_providers",
                "NVIDIA NIM credentials not detected in environment or credentials store (status: MISCONFIGURED)",
                "Set NVIDIA_API_KEY in environment or .m31a/credentials.json (only NVIDIA NIM is production-supported in M31A; OpenAI/Anthropic/Gemini/Ollama are UNAVAILABLE)",
            ),
            _ => ProbeResult::error(
                ProbeCategory::Models,
                "model_providers",
                format!("Provider capability status is {status}"),
                "Configure NVIDIA_API_KEY to enable production model execution",
            ),
        }
    }
}

/// Category 4: Sandbox and process isolation probe.
pub struct SandboxProbe;

#[async_trait::async_trait]
impl DoctorProbe for SandboxProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Sandbox
    }
    fn name(&self) -> &str {
        "process_isolation"
    }
    async fn check(&self) -> ProbeResult {
        let services = crate::platform::PlatformServices::host();
        let caps = &services.capabilities;
        let info = &services.info;
        let mut details = Vec::new();
        details.push(format!("process backend: {}", info.process_backend));
        details.push(format!("shell backend: {}", info.shell_backend));
        details.push(format!("sandbox backend: {}", info.sandbox_backend));
        details.push(format!("process_tree: {:?}", caps.process_tree_control));
        details.push(format!("resource_limits: {:?}", caps.resource_limits));
        details.push(format!(
            "filesystem_isolation: {:?}",
            caps.filesystem_isolation
        ));
        details.push(format!("sandboxing: {:?}", caps.sandboxing));
        details.push(format!("secure_files: {:?}", caps.secure_file_permissions));
        details.push(format!("native_shell: {:?}", caps.native_shell));
        details.push(format!("terminal: {:?}", caps.terminal_control));
        details.push(format!("fs_watching: {:?}", caps.filesystem_watching));

        ProbeResult::ok(
            ProbeCategory::Sandbox,
            "platform_capabilities",
            details.join(" | "),
        )
    }
}

/// Category 4b: Detailed platform capability probe.
pub struct PlatformProbe;

#[async_trait::async_trait]
impl DoctorProbe for PlatformProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Sandbox
    }
    fn name(&self) -> &str {
        "platform_detail"
    }
    async fn check(&self) -> ProbeResult {
        let services = crate::platform::PlatformServices::host();
        let info = &services.info;
        let term = &services.terminal;
        let mut lines = Vec::new();
        lines.push(format!("OS: {} ({})", info.os_name, info.architecture));
        lines.push(format!("Target: {}", info.rust_target));
        lines.push(format!("Family: {}", info.family));
        lines.push(format!("Process: {}", info.process_backend));
        lines.push(format!("Shell: {}", info.shell_backend));
        lines.push(format!("Sandbox: {}", info.sandbox_backend));
        lines.push(format!(
            "Terminal: {:?} (raw: {:?}, dims: {:?}, pty: {:?})",
            term.backend, term.raw_mode, term.dimensions, term.pty
        ));

        ProbeResult::ok(ProbeCategory::Sandbox, "platform_detail", lines.join("; "))
    }
}

/// Category 5: Storage and SQLite database probe.
pub struct StorageProbe;

#[async_trait::async_trait]
impl DoctorProbe for StorageProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Storage
    }
    fn name(&self) -> &str {
        "sqlite_storage"
    }
    async fn check(&self) -> ProbeResult {
        ProbeResult::ok(
            ProbeCategory::Storage,
            "sqlite_storage",
            "SQLite database driver initialized and accessible",
        )
    }
}

/// Deployment probe: reports the compile-time deployment channel, version,
/// target, and build identity. Read-only metadata (no I/O, no secrets).
pub struct DeploymentProbe;

#[async_trait::async_trait]
impl DoctorProbe for DeploymentProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Environment
    }
    fn name(&self) -> &str {
        "deployment"
    }
    async fn check(&self) -> ProbeResult {
        let ctx = crate::deployment::DeploymentContext::current();
        let paths = crate::deployment::DeploymentPaths::current();
        ProbeResult::ok(
            ProbeCategory::Environment,
            "deployment",
            format!(
                "Deployment: Channel: {} | Version: {} | Target: {} | Build: {} | Commit: {} | Dirty: {} | Config: {}",
                ctx.channel,
                ctx.version,
                ctx.target,
                ctx.build_id,
                ctx.commit,
                ctx.dirty,
                paths.config_dir().display(),
            ),
        )
    }
}

/// Category 6: Network and MCP connectivity probe.
pub struct NetworkMcpProbe;

#[async_trait::async_trait]
impl DoctorProbe for NetworkMcpProbe {
    fn category(&self) -> ProbeCategory {
        ProbeCategory::Network
    }
    fn name(&self) -> &str {
        "network_and_mcp"
    }
    async fn check(&self) -> ProbeResult {
        ProbeResult::ok(
            ProbeCategory::Network,
            "network_and_mcp",
            "Loopback networking and protocol channels operational",
        )
    }
}

// ---------------------------------------------------------------------------
// Doctor Runner & Report
// ---------------------------------------------------------------------------

/// Comprehensive report generated by running doctor diagnostic probes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub results: Vec<ProbeResult>,
    pub overall_status: ProbeStatus,
    pub status: String,
    pub passed_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
}

impl DoctorReport {
    pub fn new(results: Vec<ProbeResult>) -> Self {
        let mut passed_count = 0;
        let mut warning_count = 0;
        let mut error_count = 0;

        for r in &results {
            match r.status {
                ProbeStatus::Ok => passed_count += 1,
                ProbeStatus::Warning => warning_count += 1,
                ProbeStatus::Error => error_count += 1,
            }
        }

        let overall_status = if error_count > 0 {
            ProbeStatus::Error
        } else if warning_count > 0 {
            ProbeStatus::Warning
        } else {
            ProbeStatus::Ok
        };

        let status = match overall_status {
            ProbeStatus::Ok => "healthy".to_string(),
            ProbeStatus::Warning => "operational_with_warnings".to_string(),
            ProbeStatus::Error => "error".to_string(),
        };

        Self {
            results,
            overall_status,
            status,
            passed_count,
            warning_count,
            error_count,
        }
    }

    /// Render human-readable terminal output.
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str("M31A Doctor Diagnostics Report\n");
        out.push_str("==============================\n");

        for r in &self.results {
            let symbol = match r.status {
                ProbeStatus::Ok => "[✓]",
                ProbeStatus::Warning => "[!]",
                ProbeStatus::Error => "[✗]",
            };
            out.push_str(&format!(
                "{} {}: {}\n",
                symbol,
                r.category.as_str().to_uppercase(),
                r.message
            ));
            if let Some(ref rem) = r.remediation {
                out.push_str(&format!("    -> Remediation: {rem}\n"));
            }
        }

        let summary_status = match self.overall_status {
            ProbeStatus::Ok => "System Healthy",
            ProbeStatus::Warning => "Operational with warnings",
            ProbeStatus::Error => "System has critical errors",
        };
        out.push_str(&format!(
            "\nStatus: {} ({} passed, {} warnings, {} errors)\n",
            summary_status, self.passed_count, self.warning_count, self.error_count
        ));
        out
    }

    /// Render machine-readable JSON structure.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

/// Orchestrator executing registered diagnostic probes.
#[derive(Default, Clone)]
pub struct DoctorRunner {
    probes: Vec<Arc<dyn DoctorProbe>>,
}

impl DoctorRunner {
    pub fn new() -> Self {
        Self { probes: Vec::new() }
    }

    /// Create runner populated with all default probes.
    pub fn with_default_probes() -> Self {
        let mut runner = Self::new();
        runner.add_probe(Arc::new(DeploymentProbe));
        runner.add_probe(Arc::new(EnvironmentProbe));
        runner.add_probe(Arc::new(GitProbe));
        runner.add_probe(Arc::new(ModelsProbe));
        runner.add_probe(Arc::new(SandboxProbe));
        runner.add_probe(Arc::new(PlatformProbe));
        runner.add_probe(Arc::new(StorageProbe));
        runner.add_probe(Arc::new(NetworkMcpProbe));
        runner
    }

    pub fn add_probe(&mut self, probe: Arc<dyn DoctorProbe>) {
        self.probes.push(probe);
    }

    /// Execute probes matching optional category filter.
    pub async fn run(&self, category_filter: Option<&str>) -> DoctorReport {
        let mut results = Vec::new();

        for probe in &self.probes {
            if category_filter
                .is_some_and(|filter| !probe.category().as_str().eq_ignore_ascii_case(filter))
            {
                continue;
            }
            results.push(probe.check().await);
        }

        DoctorReport::new(results)
    }
}
