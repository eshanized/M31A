//! Configuration integrity & architecture enforcement (integrity phase).
//!
//! These tests fail when architectural rules are violated:
//! - runtime subsystems must consume resolved canonical configuration,
//!   never invent operational defaults;
//! - changing canonical configuration must change runtime behavior;
//! - invalid settings must never persist; cancelled/invalid edits must not
//!   partially mutate the draft;
//! - immutable safety ceilings must hold regardless of configuration;
//! - source-level drift (new hidden model/provider/endpoint/timeout
//!   literals) must be caught by scope-aware static scans.

use m31a::config::canonical as C;
use m31a::config::schema::AppConfig;
use m31a::config::{ResolvedConfiguration, validate_config};

// ── Helpers ────────────────────────────────────────────────────────────────

fn write_workspace_config(ws: &std::path::Path, body: &str) {
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::fs::write(ws.join(".m31a").join("config.toml"), body).unwrap();
}

// ── 1. Runtime consumption: resolved config drives runtime behavior ────────

#[test]
fn dispatcher_runtime_timeout_follows_resolved_config() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    write_workspace_config(&ws, "[runtime]\ntimeout_secs = 450\n");
    let cfg = ResolvedConfiguration::for_workspace(&ws).expect("loads");
    assert_eq!(cfg.timeout_policy().runtime_secs, 450);

    let storage = dir.path().join("storage");
    std::fs::create_dir_all(&storage).unwrap();
    let disp = m31a::agent::dispatcher::ProductionWorkerDispatcher::new_with_roots_and_config(
        &ws,
        &storage,
        Some(&cfg),
    );
    assert_eq!(disp.runtime_timeout_secs(), 450);

    // Profile wall timeouts are bounded by the configured runtime timeout.
    assert_eq!(disp.effective_wall_timeout_secs(900), 450);
    assert_eq!(disp.effective_wall_timeout_secs(100), 100);
}

#[test]
fn dispatcher_fallback_applies_canonical_default_not_a_private_literal() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    let storage = dir.path().join("storage");
    std::fs::create_dir_all(&storage).unwrap();
    // Legacy shim without configuration: the single canonical default
    // applies (same authority as the resolver), never a second literal.
    let disp = m31a::agent::dispatcher::ProductionWorkerDispatcher::new_with_roots_and_config(
        &ws, &storage, None,
    );
    assert_eq!(disp.runtime_timeout_secs(), C::DEFAULT_RUNTIME_TIMEOUT_SECS);
}

#[test]
fn timeouts_and_resources_propagate_from_workspace_file() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    write_workspace_config(
        &ws,
        "[timeouts]\nworkflow_step_secs = 700\nverification_secs = 90\n\
         [resources]\ntool_timeout_secs = 45\ntool_max_output_bytes = 2097152\n",
    );
    let cfg = ResolvedConfiguration::for_workspace(&ws).expect("loads");
    let tp = cfg.timeout_policy();
    assert_eq!(tp.workflow_step_secs, 700);
    assert_eq!(tp.verification_secs, 90);
    // Untouched scopes keep canonical defaults.
    assert_eq!(tp.runtime_secs, C::DEFAULT_RUNTIME_TIMEOUT_SECS);
    let rp = cfg.resource_policy();
    assert_eq!(rp.tool_timeout_secs, 45);
    assert_eq!(rp.tool_max_output_bytes, 2097152);
}

#[test]
fn tool_default_limits_are_canonical_and_clamped_to_ceilings() {
    use m31a::tools::definition::ResourceLimits;
    assert_eq!(
        ResourceLimits::default().timeout_secs,
        C::DEFAULT_TOOL_TIMEOUT_SECS
    );
    assert_eq!(
        ResourceLimits::default().max_output_bytes,
        C::DEFAULT_TOOL_MAX_OUTPUT_BYTES
    );
    // Configuration can tighten below ceilings but never raise above them.
    let loose = ResourceLimits::new(9999, 999 * 1024 * 1024);
    let eff = ResourceLimits::effective(&loose, &m31a::config::ResourcesConfig::default());
    assert_eq!(eff.timeout_secs, ResourceLimits::MAX_TIMEOUT_SECS);
    assert_eq!(eff.max_output_bytes, ResourceLimits::MAX_OUTPUT_BYTES);
}

#[test]
fn scheduler_default_workers_are_canonical() {
    use m31a::scheduler::concurrency::ConcurrencyLimits;
    assert_eq!(
        ConcurrencyLimits::default().max_global_workers,
        C::DEFAULT_SCHEDULER_WORKERS
    );
}

#[test]
fn genesis_options_default_matches_canonical_authority() {
    let opts = m31a::workflow::genesis::intake::GenesisOptions::default();
    assert_eq!(opts.max_discovery_turns, C::DEFAULT_MAX_DISCOVERY_TURNS);
    assert_eq!(
        opts.ambiguity_threshold_percent,
        C::DEFAULT_AMBIGUITY_THRESHOLD_PERCENT
    );
    assert_eq!(opts.research_concurrency, C::DEFAULT_RESEARCH_CONCURRENCY);
}

// ── 2. Invalid configuration can never persist via /settings ──────────────

#[test]
fn invalid_operational_values_rejected_by_schema_validation() {
    let mut bad = AppConfig::default();
    bad.runtime.timeout_secs = 0;
    assert!(validate_config(&bad).is_err());
    let mut bad = AppConfig::default();
    bad.runtime.timeout_secs = 999_999;
    assert!(validate_config(&bad).is_err());
    let mut bad = AppConfig::default();
    bad.resources.runtime_concurrency = 0;
    assert!(validate_config(&bad).is_err());
    let mut bad = AppConfig::default();
    bad.provider.default = "openai".to_string();
    assert!(validate_config(&bad).is_err());
    let mut bad = AppConfig::default();
    bad.agents.max_tokens = 10;
    assert!(validate_config(&bad).is_err());
}

#[test]
fn persist_refuses_invalid_draft_and_writes_nothing() {
    use m31a::tui::surface::settings::persist_workspace_config;
    let dir = tempfile::tempdir().unwrap();
    let mut draft = AppConfig::default();
    draft.timeouts.verification_secs = 0;
    let target = m31a::config::paths::PlatformPaths::workspace_config_file(dir.path());
    assert!(
        persist_workspace_config(dir.path(), &draft).is_err(),
        "invalid draft must not persist"
    );
    assert!(
        !target.is_file(),
        "failed persist must leave no file behind"
    );
}

#[test]
fn invalid_settings_edits_do_not_partially_mutate_draft() {
    use m31a::tui::surface::settings::apply_edit_to_draft;
    let mut draft = AppConfig::default();
    let before = draft.clone();
    assert!(apply_edit_to_draft(&mut draft, "runtime.timeout_secs", "not-a-number").is_err());
    assert_eq!(draft, before, "failed edit must leave draft untouched");
    assert!(apply_edit_to_draft(&mut draft, "agents.default_model", "").is_err());
    assert_eq!(draft, before);
    assert!(apply_edit_to_draft(&mut draft, "provider.default", "anthropic").is_err());
    assert_eq!(draft, before);
    assert!(apply_edit_to_draft(&mut draft, "bogus.unknown_key", "1").is_err());
    assert_eq!(draft, before);
}

#[test]
fn settings_restart_required_flags_are_explicit() {
    use m31a::tui::surface::settings::apply_edit_to_draft;
    let mut draft = AppConfig::default();
    assert!(apply_edit_to_draft(&mut draft, "provider.default", "nvidia_nim").unwrap());
    assert!(
        apply_edit_to_draft(
            &mut draft,
            "provider.nvidia_nim.base_url",
            "https://integrate.api.nvidia.com/v1"
        )
        .unwrap()
    );
    assert!(apply_edit_to_draft(&mut draft, "runtime.sandbox_mode", "standard").unwrap());
    // Operational tunables apply without restart.
    assert!(!apply_edit_to_draft(&mut draft, "runtime.timeout_secs", "450").unwrap());
    assert!(!apply_edit_to_draft(&mut draft, "timeouts.verification_secs", "90").unwrap());
}

// ── 3. /settings round-trip: config → editor → save → reload ───────────────

#[test]
fn settings_full_round_trip_preserves_semantics() {
    use m31a::tui::surface::settings::{apply_edit_to_draft, persist_workspace_config};
    let dir = tempfile::tempdir().unwrap();
    let mut draft = AppConfig::default();
    for (key, value) in [
        ("agents.default_model", "meta/llama-3.2-11b-vision-instruct"),
        ("agents.max_tokens", "4096"),
        ("runtime.timeout_secs", "450"),
        ("runtime.concurrency_limit", "6"),
        ("timeouts.verification_secs", "90"),
        ("timeouts.workflow_step_secs", "700"),
        ("resources.tool_timeout_secs", "45"),
        ("budget.max_agent_steps", "25"),
        ("cache.catalog_freshness_secs", "1800"),
        ("tui.fps", "60"),
        ("git.branch_prefix", "m31a/mission"),
        ("workspace.project_type", "rust"),
    ] {
        apply_edit_to_draft(&mut draft, key, value)
            .unwrap_or_else(|e| panic!("edit {key}={value} must validate: {e}"));
    }
    persist_workspace_config(dir.path(), &draft).expect("valid draft persists");

    let reloaded = ResolvedConfiguration::for_workspace(dir.path()).expect("reload");
    let app = &reloaded.app_config;
    assert_eq!(
        app.agents.default_model,
        "meta/llama-3.2-11b-vision-instruct"
    );
    assert_eq!(app.agents.max_tokens, 4096);
    assert_eq!(app.runtime.timeout_secs, 450);
    assert_eq!(app.runtime.concurrency_limit, 6);
    assert_eq!(app.timeouts.verification_secs, 90);
    assert_eq!(app.timeouts.workflow_step_secs, 700);
    assert_eq!(app.resources.tool_timeout_secs, 45);
    assert_eq!(app.budget.max_agent_steps, Some(25));
    assert_eq!(app.cache.catalog_freshness_secs, 1800);
    assert_eq!(app.tui.fps, 60);
    assert_eq!(app.git.branch_prefix, "m31a/mission");
    assert_eq!(app.workspace.project_type.as_deref(), Some("rust"));

    // Provenance: edited keys report the workspace tier as winner.
    let explain = reloaded.explain("runtime.timeout_secs").expect("explain");
    assert_eq!(explain.resolved_value, serde_json::json!(450));
    assert!(
        explain.winning_tier.to_lowercase().contains("workspace"),
        "unexpected winning tier: {}",
        explain.winning_tier
    );
}

// ── 4. Scope-aware static guards: no new hidden authorities ────────────────

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

fn src_files() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    collect_rs_files(std::path::Path::new("src"), &mut out);
    out
}

fn is_test_content(path: &std::path::Path, line: &str) -> bool {
    // Unit-test modules and fixture constructors are normative artifacts,
    // not production authorities.
    let p = path.to_string_lossy();
    p.contains("testing/")
        || p.contains("test_")
        || line.contains("Normative Artifact")
        || line.contains("test-only")
        || line.contains("test fixture")
}

/// Known-classification allowlist: (file suffix, pattern that must appear on
/// the matching line or in the file to justify the literal).
fn line_justified(path: &std::path::Path, content: &str, lineno: usize, line: &str) -> bool {
    let p = path.to_string_lossy().replace('\\', "/");
    let justified_markers = [
        "ALGORITHMIC",
        "IMMUTABLE",
        "COMPATIBILITY",
        "Normative Artifact",
        "single authority",
        "Single authority",
        "canonical::",
        "canonical defaults",
        "display-only",
        "Display-only",
        "layout constant",
        "estimation constant",
        "transport heuristic",
        "ordering midpoint",
        "scheduling-estimate",
        "FALLBACK_ESTIMATED_TOKENS",
        "DEFAULT_DAG_TASK_PRIORITY",
        "COMPOSER_EMPTY_LABEL_WIDTH",
        "RATE_LIMIT_DEFAULT_COOLDOWN",
        "PLAN_TASK_ESTIMATE",
        "WIZARD_DISPLAY_DEFAULT",
    ];
    // A justification marker on the same line, or a file-level authority
    // comment covering small subsystem-intrinsic files.
    if justified_markers.iter().any(|m| line.contains(m)) {
        return true;
    }
    let _ = (content, lineno);
    // Live-model test-harness authority: the single identifier every
    // real-model behavioral test resolves to (TEST FIXTURE class). It never
    // selects production models; production selection is
    // `effective_model_selection`.
    if p.ends_with("model/catalog.rs")
        && (line.contains("CANONICAL_REAL_MODEL_") || line.contains("resolve_real_model_id"))
    {
        return true;
    }
    // Curated capability-metadata registry (context windows, tiers) derived
    // from official model cards. LEGITIMATE BUILTIN ASSET: capability
    // metadata, not model selection — the router still resolves the active
    // model from configuration + candidates.
    if p.ends_with("model/provider/nvidia_metadata.rs")
        && content.contains("load_official_nvidia_references")
    {
        return true;
    }
    // Retired-provider descriptors exist solely for deterministic rejection.
    if p.ends_with("config/provider_registry.rs")
        && (line.contains("is_production_supported: false")
            || content.contains("retired entries retained solely for deterministic rejection"))
    {
        return true;
    }
    // Wizard provider labels are display-only; selection is NVIDIA-gated.
    if p.ends_with("tui/screens/wizard.rs") && line.contains("UNAVAILABLE") {
        return true;
    }
    // Schema compat shims and validation bounds are the authority itself.
    if p.ends_with("config/schema.rs") {
        return true;
    }
    // Canonical authority states every ordinary default exactly once.
    if p.ends_with("config/canonical.rs") {
        return true;
    }
    // Endpoint trust layer owns URL *validation*; the string itself is
    // re-exported from canonical.
    if p.ends_with("model/provider/endpoint.rs") && line.contains("CANONICAL_NVIDIA_BASE_URL") {
        return true;
    }
    false
}

fn scan_forbidden<F>(check: F) -> Vec<String>
where
    F: Fn(&std::path::Path, &str) -> Option<String>,
{
    let mut violations = Vec::new();
    for path in src_files() {
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        // Heuristic test-module boundary: unit tests live in a trailing
        // `mod tests` module — normative fixtures, never production
        // authorities. Everything from that marker on is exempt.
        let test_mod_start = if content.contains("#[cfg(test)]") {
            lines
                .iter()
                .position(|l| l.trim_start().starts_with("mod tests"))
        } else {
            None
        };
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if is_test_content(&path, line) {
                continue;
            }
            if let Some(start) = test_mod_start
                && idx >= start
            {
                continue;
            }
            if let Some(reason) = check(&path, line) {
                if !line_justified(&path, &content, 0, line) {
                    violations.push(format!("{}: {reason} :: {}", path.display(), line.trim()));
                }
            }
        }
    }
    violations
}

#[test]
fn no_bare_operational_timeout_resource_fallbacks() {
    // Bare numeric `unwrap_or` for operational magnitudes must reference a
    // named canonical/subsystem constant instead of restating the literal.
    let hits = scan_forbidden(|_path, line| {
        for lit in [
            "unwrap_or(25)",
            "unwrap_or(30)",
            "unwrap_or(50)",
            "unwrap_or(60)",
            "unwrap_or(100)",
            "unwrap_or(300)",
            "unwrap_or(600)",
            "unwrap_or(1000)",
            "unwrap_or(1024)",
            "unwrap_or(4096)",
            "unwrap_or(65536)",
            "unwrap_or(1048576)",
        ] {
            if line.contains(lit) {
                return Some(format!("bare operational fallback {lit}"));
            }
        }
        None
    });
    assert!(
        hits.is_empty(),
        "hidden operational fallbacks must use canonical/named constants:\n{}",
        hits.join("\n")
    );
}

#[test]
fn no_scattered_production_model_or_endpoint_literals() {
    // Production model/provider/endpoint strings may appear only in the
    // canonical authority, its re-exports, retired-descriptor compat, and
    // explicitly display-only/test sites.
    let hits = scan_forbidden(|_path, line| {
        for lit in [
            "integrate.api.nvidia.com",
            "api.openai.com",
            "api.anthropic.com",
            "generativelanguage.googleapis.com",
            "gpt-4o",
            "claude-3-5-sonnet",
            "gemini-1.5-pro",
            "llama3:latest",
            "nemotron-3-ultra",
        ] {
            if line.contains(lit) {
                return Some(format!("scattered production literal {lit}"));
            }
        }
        // Bare model selection outside the resolver/registry/catalog.
        if line.contains("unwrap_or(\"meta/")
            || line.contains("unwrap_or_else(|| \"meta/")
            || line.contains("unwrap_or(\"nvidia/")
        {
            return Some("bare model-selection fallback".to_string());
        }
        None
    });
    assert!(
        hits.is_empty(),
        "production model/endpoint literals outside canonical authority:\n{}",
        hits.join("\n")
    );
}

#[test]
fn canonical_consumers_reference_canonical_authority() {
    // Positive pins: known runtime sites must consume the canonical
    // authority rather than restating literals. Fails on regression.
    for (file, marker) in [
        ("src/tools/process/mod.rs", "DEFAULT_TOOL_TIMEOUT_SECS"),
        (
            "src/controller/dependencies.rs",
            "DEFAULT_RUNTIME_CONCURRENCY",
        ),
        ("src/tui/screens/wizard.rs", "DEFAULT_RUNTIME_CONCURRENCY"),
        ("src/planning/service.rs", "DEFAULT_RUNTIME_TIMEOUT_SECS"),
        ("src/config/schema.rs", "DEFAULT_AGENT_MAX_TOKENS"),
        ("src/config/schema.rs", "DEFAULT_MAX_DISCOVERY_TURNS"),
        (
            "src/workflow/genesis/intake.rs",
            "DEFAULT_MAX_DISCOVERY_TURNS",
        ),
        ("src/repo/query.rs", "DEFAULT_QUERY_MAX_RESULTS"),
        ("src/agent/registry.rs", "DEFAULT_PER_ROLE_CONCURRENCY"),
        (
            "src/model/provider/nvidia.rs",
            "DEFAULT_NVIDIA_HTTP_TIMEOUT_SECS",
        ),
    ] {
        let content =
            std::fs::read_to_string(file).unwrap_or_else(|_| panic!("must be able to read {file}"));
        assert!(
            content.contains(marker),
            "{file} must consume canonical {marker}"
        );
    }
}
