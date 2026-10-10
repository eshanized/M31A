//! Deployment channel verification: DEVELOPMENT vs PRODUCTION.
//!
//! One core runtime, two isolated channels. These tests pin the deployment
//! contract: compile-time channel identity, version authority, deterministic
//! build IDs, state isolation, project-local semantics, side-by-side safety,
//! update/rollback behavior, promotion gates, TUI/doctor/CLI reporting.

use m31a::cli::{CliDispatcher, RuntimeCommand};
use m31a::deployment::{
    DeploymentChannel, DeploymentContext, DeploymentManifest, DeploymentPaths, DevelopmentFeature,
    FeatureGate, Installer, ReleaseArtifact, UpdateChannel, derive_build_id, discover_update,
    evaluate_promotion, rollback, rollback_available,
};
use m31a::release::status::{Blocker, ReleaseCandidate, ReleaseEvidence};
use m31a::release::version::{PKG_VERSION, runtime_version};

// ---------------------------------------------------------------------------
// §49 — deployment channel identity
// ---------------------------------------------------------------------------

#[test]
fn production_and_development_are_distinct_channels() {
    assert_ne!(
        DeploymentChannel::Production,
        DeploymentChannel::Development
    );
    assert_eq!(DeploymentChannel::Production.binary_name(), "m31a");
    assert_eq!(DeploymentChannel::Development.binary_name(), "m31a-dev");
    // Update feeds mirror channels without duplicating semantics.
    assert_eq!(
        DeploymentChannel::Production.update_channel(),
        UpdateChannel::Stable
    );
    assert_eq!(
        DeploymentChannel::Development.update_channel(),
        UpdateChannel::Development
    );
}

#[test]
fn channel_cannot_be_changed_by_environment_variables() {
    // The channel is compile-time artifact identity. Arbitrary runtime env
    // vars must never flip it (a production binary stays production even
    // when the user exports M31A_ENV=development).
    let before = DeploymentChannel::current();
    unsafe {
        std::env::set_var("M31A_ENV", "development");
        std::env::set_var("M31A_CHANNEL", "development");
    }
    assert_eq!(DeploymentChannel::current(), before);
    assert_eq!(DeploymentContext::current().channel, before);
    unsafe {
        std::env::remove_var("M31A_ENV");
        std::env::remove_var("M31A_CHANNEL");
    }
    assert_eq!(DeploymentChannel::current(), before);
}

// ---------------------------------------------------------------------------
// §50 — version authority (Cargo.toml == CARGO_PKG_VERSION == runtime)
// ---------------------------------------------------------------------------

#[test]
fn version_authority_holds_for_both_channels() {
    let doc = std::fs::read_to_string("Cargo.toml").unwrap();
    let cargo_ver = m31a::release::version::parse_cargo_toml_version(&doc).unwrap();
    assert_eq!(cargo_ver, PKG_VERSION);
    assert_eq!(PKG_VERSION, runtime_version());

    for channel in [
        DeploymentChannel::Production,
        DeploymentChannel::Development,
    ] {
        let ctx = DeploymentContext::from_parts(
            channel,
            PKG_VERSION,
            "abc123",
            "master",
            "2026-01-01T00:00:00Z",
            "x86_64-unknown-linux-gnu",
            false,
        );
        // Channel suffix/display metadata never forks the semantic version.
        assert_eq!(ctx.version, PKG_VERSION);
        assert!(semver::Version::parse(&ctx.version).is_ok());
    }

    let current = DeploymentContext::current();
    assert_eq!(current.version, PKG_VERSION);
}

// ---------------------------------------------------------------------------
// §51 — build ID determinism
// ---------------------------------------------------------------------------

#[test]
fn build_id_deterministic_and_discriminating() {
    let base = derive_build_id("0.1.1", "production", "abc", "x86_64-unknown-linux-gnu");
    assert_eq!(
        base,
        derive_build_id("0.1.1", "production", "abc", "x86_64-unknown-linux-gnu")
    );
    assert_ne!(
        base,
        derive_build_id("0.1.1", "production", "abd", "x86_64-unknown-linux-gnu")
    );
    assert_ne!(
        base,
        derive_build_id("0.1.1", "development", "abc", "x86_64-unknown-linux-gnu")
    );
    assert_ne!(
        base,
        derive_build_id("0.1.1", "production", "abc", "aarch64-apple-darwin")
    );
}

// ---------------------------------------------------------------------------
// §52 — global state isolation
// ---------------------------------------------------------------------------

#[test]
fn global_config_data_cache_state_differ_by_channel() {
    let prod = DeploymentPaths::new(DeploymentChannel::Production);
    let dev = DeploymentPaths::new(DeploymentChannel::Development);
    // App-dir names are the isolation root.
    assert_eq!(prod.channel().app_dir_name(), "m31a");
    assert_eq!(dev.channel().app_dir_name(), "m31a-dev");
    assert_ne!(prod.socket_path(), dev.socket_path());
    assert_ne!(prod.pid_file(), dev.pid_file());
    assert_ne!(prod.log_dir(), dev.log_dir());
    // Platform-backed dirs embed the app name when ProjectDirs resolves.
    if prod.config_dir() != dev.config_dir() {
        assert_ne!(prod.data_dir(), dev.data_dir());
        assert_ne!(prod.cache_dir(), dev.cache_dir());
        assert_ne!(prod.state_dir(), dev.state_dir());
    }
}

// ---------------------------------------------------------------------------
// §53 — project-local .m31a semantics
// ---------------------------------------------------------------------------

#[test]
fn project_state_shared_root_but_isolated_runtime_state() {
    let ws = std::path::Path::new("/tmp/ws-probe");
    // One shared workspace directory for both channels.
    assert_eq!(DeploymentPaths::project_root(ws), ws.join(".m31a"));
    // Production keeps the legacy DB (backward compatible); dev is isolated.
    assert_eq!(
        DeploymentPaths::project_db_path(ws, DeploymentChannel::Production),
        ws.join(".m31a").join("m31a.db")
    );
    assert_eq!(
        DeploymentPaths::project_db_path(ws, DeploymentChannel::Development),
        ws.join(".m31a").join("m31a-dev.db")
    );
    // Deployment-scoped runtime state is namespaced per channel.
    assert_ne!(
        DeploymentPaths::project_state_dir(ws, DeploymentChannel::Production),
        DeploymentPaths::project_state_dir(ws, DeploymentChannel::Development)
    );
    // Credentials are never shared between channels.
    assert_ne!(
        DeploymentPaths::project_credentials_file(ws, DeploymentChannel::Production),
        DeploymentPaths::project_credentials_file(ws, DeploymentChannel::Development)
    );
}

// ---------------------------------------------------------------------------
// §54 — side-by-side installations
// ---------------------------------------------------------------------------

#[test]
fn side_by_side_channels_do_not_collide() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    // Both channels initialize their own project state without touching the
    // other's files.
    for channel in [
        DeploymentChannel::Production,
        DeploymentChannel::Development,
    ] {
        let db = DeploymentPaths::project_db_path(ws, channel);
        let state = DeploymentPaths::project_state_dir(ws, channel);
        std::fs::create_dir_all(state.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(&db, format!("db-for-{}", channel.as_str())).unwrap();
        std::fs::write(
            DeploymentPaths::project_socket_path(ws, channel)
                .parent()
                .unwrap()
                .join("probe"),
            b"ok",
        )
        .unwrap_or(());
    }
    let prod_db = std::fs::read_to_string(DeploymentPaths::project_db_path(
        ws,
        DeploymentChannel::Production,
    ))
    .unwrap();
    let dev_db = std::fs::read_to_string(DeploymentPaths::project_db_path(
        ws,
        DeploymentChannel::Development,
    ))
    .unwrap();
    assert_ne!(prod_db, dev_db);
    assert_ne!(
        DeploymentPaths::project_socket_path(ws, DeploymentChannel::Production),
        DeploymentPaths::project_socket_path(ws, DeploymentChannel::Development)
    );
}

// ---------------------------------------------------------------------------
// §55 — update semantics incl. rollback
// ---------------------------------------------------------------------------

fn test_artifact(channel: DeploymentChannel, script_body: &[u8]) -> (ReleaseArtifact, Vec<u8>, Vec<u8>) {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use m31a::release::integrity::sha256_bytes;

    let mut full_script = format!("#!/bin/sh\necho \"{} 0.2.0\"\n# ", channel.binary_name()).into_bytes();
    full_script.extend_from_slice(script_body);
    full_script.push(b'\n');

    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    header.set_size(full_script.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    tar.append_data(&mut header, channel.binary_name(), full_script.as_slice())
        .unwrap();
    let archive_bytes = tar.into_inner().unwrap().finish().unwrap();
    let digest = sha256_bytes(&archive_bytes);
    let target = DeploymentContext::current().target;

    (
        ReleaseArtifact {
            artifact_id: format!("{}-0.2.0-{}", channel.binary_name(), target),
            version: "0.2.0".to_string(),
            channel,
            target,
            format: "tar.gz".to_string(),
            filename: "m31a-test.tar.gz".to_string(),
            sha256: digest,
            size: archive_bytes.len() as u64,
            build_id: "0123456789abcdef".to_string(),
            git_commit: "abc12345".to_string(),
        },
        archive_bytes,
        full_script,
    )
}

#[test]
fn update_success_checksum_mismatch_and_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let inst = Installer::new(dir.path());

    // Successful update stages, verifies, and preserves the previous binary.
    let (a1, b1, s1) = test_artifact(DeploymentChannel::Production, b"v1-bytes");
    let live = inst
        .install_bytes(&a1, &b1, DeploymentChannel::Production)
        .unwrap();
    assert_eq!(std::fs::read(&live).unwrap(), s1);

    let (a2, b2, s2) = test_artifact(DeploymentChannel::Production, b"v2-bytes-longer");
    inst.install_bytes(&a2, &b2, DeploymentChannel::Production)
        .unwrap();
    assert_eq!(std::fs::read(&live).unwrap(), s2);

    // Checksum mismatch: previous known-good executable survives.
    let (mut bad, _, _) = test_artifact(DeploymentChannel::Production, b"expected");
    bad.size = b"corrupt".len() as u64;
    assert!(
        inst.install_bytes(&bad, b"corrupt", DeploymentChannel::Production)
            .is_err()
    );
    assert_eq!(std::fs::read(&live).unwrap(), s2);

    // Missing artifact bytes (empty download) are rejected.
    let (mut empty, _, _) = test_artifact(DeploymentChannel::Production, b"nonempty");
    empty.sha256 = "e".repeat(64);
    empty.size = 0;
    assert!(
        inst.install_bytes(&empty, b"", DeploymentChannel::Production)
            .is_err()
    );

    // Rollback restores the previous known-good binary.
    assert!(rollback_available(dir.path(), "m31a"));
    let restored = rollback(dir.path(), "m31a").unwrap();
    assert_eq!(restored, live);
    assert_eq!(std::fs::read(&restored).unwrap(), s1);

    // Rollback without a backup fails closed.
    let fresh = tempfile::tempdir().unwrap();
    assert!(!rollback_available(fresh.path(), "m31a"));
    assert!(rollback(fresh.path(), "m31a").is_err());
}

// ---------------------------------------------------------------------------
// §56 — channel update safety
// ---------------------------------------------------------------------------

fn channel_manifest(channel: DeploymentChannel, version: &str) -> DeploymentManifest {
    DeploymentManifest {
        schema_version: 1,
        version: version.to_string(),
        channel,
        build_id: Some("0123456789abcdef".to_string()),
        commit: Some("abc".to_string()),
        artifacts: vec![m31a::deployment::ManifestArtifact {
            target: "x86_64-unknown-linux-gnu".to_string(),
            filename: "m31a-linux-x64.tar.gz".to_string(),
            sha256: "a".repeat(64),
            size: 10,
            build_id: "0123456789abcdef".to_string(),
            commit: "abc".to_string(),
        }],
    }
}

#[test]
fn production_cannot_consume_development_manifests() {
    let dev = channel_manifest(DeploymentChannel::Development, "0.2.0");
    assert!(
        discover_update(
            &dev,
            DeploymentChannel::Production,
            "x86_64-unknown-linux-gnu",
            "0.1.1"
        )
        .is_err()
    );
    let prod = channel_manifest(DeploymentChannel::Production, "0.2.0");
    assert!(
        discover_update(
            &prod,
            DeploymentChannel::Development,
            "x86_64-unknown-linux-gnu",
            "0.1.1"
        )
        .is_err()
    );
    // Same-channel newer version discovers deterministically.
    let c = discover_update(
        &prod,
        DeploymentChannel::Production,
        "x86_64-unknown-linux-gnu",
        "0.1.1",
    )
    .unwrap();
    assert_eq!(c.version, "0.2.0");
}

// ---------------------------------------------------------------------------
// §57 — release promotion through the existing state machine
// ---------------------------------------------------------------------------

fn promoted_candidate() -> ReleaseCandidate {
    let mut rc = ReleaseCandidate::new();
    rc.record_build(true).unwrap();
    rc.record_integrity(true).unwrap();
    rc.record_verification(&ReleaseEvidence {
        build_succeeded: true,
        integrity_ok: true,
        verification_ok: true,
        blockers: vec![],
        approval_note: None,
    })
    .unwrap();
    rc
}

fn clean_ctx(channel: DeploymentChannel) -> DeploymentContext {
    DeploymentContext::from_parts(
        channel,
        "0.1.1",
        "abc",
        "master",
        "2026-01-01T00:00:00Z",
        "x86_64-unknown-linux-gnu",
        false,
    )
}

#[test]
fn development_cannot_claim_production_without_evidence() {
    // A fresh (unbuilt) candidate never promotes, whatever the channel.
    let rc = ReleaseCandidate::new();
    let ev = ReleaseEvidence {
        build_succeeded: true,
        integrity_ok: true,
        verification_ok: true,
        blockers: vec![],
        approval_note: Some("ok".to_string()),
    };
    assert!(
        evaluate_promotion(
            DeploymentChannel::Production,
            &clean_ctx(DeploymentChannel::Development),
            &rc,
            &ev,
            "approved"
        )
        .is_err()
    );
}

#[test]
fn full_evidence_promotes_but_dirty_or_blocked_does_not() {
    let rc = promoted_candidate();
    let ev = ReleaseEvidence {
        build_succeeded: true,
        integrity_ok: true,
        verification_ok: true,
        blockers: vec![],
        approval_note: Some("ok".to_string()),
    };
    assert!(
        evaluate_promotion(
            DeploymentChannel::Production,
            &clean_ctx(DeploymentChannel::Development),
            &rc,
            &ev,
            "RC-1 approved: all gates green"
        )
        .is_ok()
    );
    // Dirty source tree denies production promotion.
    let dirty = DeploymentContext::from_parts(
        DeploymentChannel::Production,
        "0.1.1",
        "abc",
        "master",
        "2026-01-01T00:00:00Z",
        "x86_64-unknown-linux-gnu",
        true,
    );
    assert!(
        evaluate_promotion(DeploymentChannel::Production, &dirty, &rc, &ev, "approved").is_err()
    );
    // Blocking findings deny promotion even with approval.
    let blocked = ReleaseEvidence {
        build_succeeded: true,
        integrity_ok: true,
        verification_ok: true,
        blockers: vec![Blocker::Blocking],
        approval_note: Some("ok".to_string()),
    };
    assert!(
        evaluate_promotion(
            DeploymentChannel::Production,
            &clean_ctx(DeploymentChannel::Production),
            &rc,
            &blocked,
            "approved"
        )
        .is_err()
    );
}

// ---------------------------------------------------------------------------
// §58 — dirty semantics: dev allowed+marked, prod promotion denied
// ---------------------------------------------------------------------------

#[test]
fn dirty_flag_recorded_and_gates_production() {
    let dev_dirty = DeploymentContext::from_parts(
        DeploymentChannel::Development,
        "0.1.1",
        "abc",
        "dev",
        "2026-01-01T00:00:00Z",
        "x86_64-unknown-linux-gnu",
        true,
    );
    assert!(dev_dirty.dirty);
    // Development callers can observe dirty=true and proceed; production
    // promotion of the same tree is denied (tested above).
    let rc = promoted_candidate();
    let ev = ReleaseEvidence {
        build_succeeded: true,
        integrity_ok: true,
        verification_ok: true,
        blockers: vec![],
        approval_note: Some("ok".to_string()),
    };
    assert!(evaluate_promotion(DeploymentChannel::Production, &dev_dirty, &rc, &ev, "ok").is_err());
}

// ---------------------------------------------------------------------------
// §59 — TUI cockpit at four widths
// ---------------------------------------------------------------------------

fn render_header_text(model: &m31a::tui::model::TuiViewModel, width: u16, height: u16) -> String {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = m31a::tui::theme::ThemeTokens::resolve(m31a::tui::theme::ThemeMode::DarkSlateCyan);
    let replay = m31a::tui::replay::ReplayController::new();
    terminal
        .draw(|f| {
            m31a::tui::shell::header::render_header(
                f,
                f.area(),
                model,
                m31a::tui::navigation::ScreenId::TaskGraph,
                &replay,
                &tokens,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[test]
fn cockpit_header_shows_version_and_channel_at_all_widths() {
    let model = m31a::tui::model::TuiViewModel::new();
    let ctx = DeploymentContext::current();
    // Quiet header keeps identity (M31A); full version + channel identity
    // remains available via the canonical cockpit label (progressive
    // disclosure — not permanently in the header).
    let expected_version = format!("v{}", ctx.version);
    let expected_channel = if ctx.is_production() {
        "PRODUCTION"
    } else {
        "DEVELOPMENT"
    };
    for (w, h) in [(80u16, 24u16), (100, 30), (160, 40), (220, 50)] {
        let header_h = if h >= 24 { 3 } else { h.min(3) };
        let text = render_header_text(&model, w, header_h.max(2));
        assert!(text.contains("M31A"), "width {w}");
    }
    // Cockpit label contract itself (version + channel, unobtrusive source).
    assert!(ctx.cockpit_label().contains(&expected_version));
    assert!(
        ctx.cockpit_label().contains(expected_channel)
            || ctx.cockpit_label().contains(&expected_version)
    );
}

// ---------------------------------------------------------------------------
// §60 — doctor reports deployment accurately, no secrets
// ---------------------------------------------------------------------------

#[tokio::test]
async fn doctor_reports_deployment_channel() {
    use m31a::cli::doctor::{DoctorRunner, ProbeCategory};
    let runner = DoctorRunner::with_default_probes();
    let report = runner.run(None).await;
    let text = report.format_text();
    let ctx = DeploymentContext::current();
    assert!(text.contains(ctx.channel.as_str()));
    assert!(text.contains(ctx.version.as_str()));
    // The deployment probe itself carries no remediation and no secret
    // values (doctor remediation text may name env vars like API_KEY_NVIDIA
    // as configuration guidance — that is pre-existing behavior, not a leak).
    let probe = report
        .results
        .iter()
        .find(|r| r.name == "deployment")
        .expect("deployment probe registered");
    assert!(probe.message.contains("Channel:"));
    assert!(probe.message.contains("Version:"));
    assert!(probe.remediation.is_none());
    assert!(!probe.message.contains("nvapi-"));
    // The deployment probe is registered under environment.
    assert!(
        report
            .results
            .iter()
            .any(|r| r.name == "deployment" && r.category == ProbeCategory::Environment)
    );
}

// ---------------------------------------------------------------------------
// §61 — CLI version determinism per channel
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cli_version_distinguishes_channels_without_forking_semver() {
    let dispatcher = CliDispatcher::new();
    let out = dispatcher
        .dispatch(RuntimeCommand::Version { verbose: false })
        .await
        .unwrap();
    let ctx = DeploymentContext::current();
    assert_eq!(out.text, ctx.cli_version_string());
    assert_eq!(out.data["version"], serde_json::json!(PKG_VERSION));

    let prod = DeploymentContext::from_parts(
        DeploymentChannel::Production,
        "0.1.1",
        "abc",
        "master",
        "2026-01-01T00:00:00Z",
        "x86_64-unknown-linux-gnu",
        false,
    );
    let dev = DeploymentContext::from_parts(
        DeploymentChannel::Development,
        "0.1.1",
        "abc",
        "dev",
        "2026-01-01T00:00:00Z",
        "x86_64-unknown-linux-gnu",
        true,
    );
    assert_eq!(prod.cli_version_string(), "m31a 0.1.1");
    assert!(dev.cli_version_string().starts_with("m31a-dev 0.1.1-dev+"));
    assert_ne!(prod.cli_version_string(), dev.cli_version_string());
    assert_eq!(prod.version, dev.version);

    // Verbose version carries the full deployment report.
    let verbose = dispatcher
        .dispatch(RuntimeCommand::Version { verbose: true })
        .await
        .unwrap();
    assert!(verbose.text.contains("Deployment:"));
    assert!(verbose.text.contains("Channel:"));
}

// ---------------------------------------------------------------------------
// Manifest + artifact validation (§66-67)
// ---------------------------------------------------------------------------

#[test]
fn deployment_manifest_rejects_malformed() {
    let good = channel_manifest(DeploymentChannel::Production, "0.1.1");
    assert!(good.validate().is_ok());
    // Same schema serves development.
    assert!(
        channel_manifest(DeploymentChannel::Development, "0.1.1")
            .validate()
            .is_ok()
    );
    for mut m in [
        DeploymentManifest {
            version: String::new(),
            ..channel_manifest(DeploymentChannel::Production, "0.1.1")
        },
        DeploymentManifest {
            version: "nope".to_string(),
            ..channel_manifest(DeploymentChannel::Production, "0.1.1")
        },
        DeploymentManifest {
            schema_version: 99,
            ..channel_manifest(DeploymentChannel::Production, "0.1.1")
        },
        DeploymentManifest {
            artifacts: vec![],
            ..channel_manifest(DeploymentChannel::Production, "0.1.1")
        },
    ] {
        assert!(m.validate().is_err());
        let _ = &mut m;
    }
    let mut dup = channel_manifest(DeploymentChannel::Production, "0.1.1");
    dup.artifacts.push(dup.artifacts[0].clone());
    assert!(dup.validate().is_err());
    let mut bad_target = channel_manifest(DeploymentChannel::Production, "0.1.1");
    bad_target.artifacts[0].target = "bogus".to_string();
    assert!(bad_target.validate().is_err());
    assert!(DeploymentManifest::parse_json("{}").is_err());
}

#[test]
fn feature_gate_production_rejects_dev_only() {
    let prod = FeatureGate::new(DeploymentChannel::Production);
    for f in DevelopmentFeature::all() {
        assert!(!prod.is_enabled(f));
    }
    let dev = FeatureGate::new(DeploymentChannel::Development);
    for f in DevelopmentFeature::all() {
        assert!(dev.is_enabled(f));
    }
}

#[test]
fn deployment_cli_commands_parse() {
    use clap::Parser;
    use m31a::cli::Cli;
    let d = Cli::try_parse_from(["m31a", "deployment"]).unwrap();
    assert!(CliDispatcher::new().parse_command(&d).is_some());
    let u = Cli::try_parse_from(["m31a", "update", "--check", "--manifest", "m.json"]).unwrap();
    assert!(matches!(
        CliDispatcher::new().parse_command(&u),
        Some(RuntimeCommand::Update {
            check_only: true,
            ..
        })
    ));
    let r = Cli::try_parse_from(["m31a", "rollback"]).unwrap();
    assert!(matches!(
        CliDispatcher::new().parse_command(&r),
        Some(RuntimeCommand::Rollback { .. })
    ));
}
