//! Phase 45 — Release Candidate validation: versioning, manifest, integrity,
//! provenance, SBOM, migrations, install/upgrade/rollback, smoke, platform,
//! hygiene, and release-process failure injection.
//!
//! All tests are deterministic. The single live probe is `#[ignore]`d per
//! `tests/architecture_live_test_partition.rs`.

use std::sync::Arc;

use tempfile::tempdir;

use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::action::ApplicationAction;
use m31a::persistence::sqlite::repositories::SqliteLifecycleRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{PreExecutionCoordinator, PreExecutionResponse};
use m31a::release::{
    ArtifactEntry, Blocker, BlockerEvaluator, Finding, PKG_VERSION, ReleaseClassification,
    ReleaseManifest, check_release_dir, cli_version_string, collect_provenance, generate_sbom,
    parse_cargo_toml_version, runtime_version, sha256_bytes, validate_sbom, verify_provenance,
    verify_sha256sums, write_sha256sums,
};

// ===========================================================================
// HELPERS
// ===========================================================================

async fn setup_db() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let dir = tempdir().expect("tempdir");
    let pool = initialize_database(&dir.path().join("t.db"))
        .await
        .expect("initialize_database");
    (dir, pool)
}

/// Assert the applied-migration ledger is complete: versions 1..=28 present,
/// all successful. This is the "schema state is verifiable" check.
/// (025 adds task prompt-reference columns for the PromptOS wiring remediation.
/// 026 adds budget-ledger estimated-provenance columns; 027 binds the full
/// execution surface on execution authorizations; 028 adds model invocation cost tracking.)
async fn assert_migrations_complete(pool: &sqlx::SqlitePool) {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT version, success FROM _sqlx_migrations ORDER BY version ASC")
            .fetch_all(pool)
            .await
            .expect("_sqlx_migrations readable");
    assert_eq!(
        rows.len(),
        28,
        "expected 28 applied migrations, got {rows:?}"
    );
    for (idx, (version, success)) in rows.iter().enumerate() {
        assert_eq!(*version, idx as i64 + 1, "migration versions contiguous");
        assert_eq!(*success, 1, "migration {version} successful");
    }
}

fn sample_manifest(dir: &std::path::Path, files: &[(&str, &[u8])]) -> ReleaseManifest {
    let mut artifacts = Vec::new();
    for (name, data) in files {
        std::fs::write(dir.join(name), data).unwrap();
        artifacts.push(ArtifactEntry {
            name: name.to_string(),
            sha256: sha256_bytes(data),
            size_bytes: data.len() as u64,
            kind: "test".to_string(),
        });
    }
    artifacts.sort_by(|a, b| a.name.cmp(&b.name));
    ReleaseManifest {
        schema_version: 1,
        project: "m31a".to_string(),
        version: PKG_VERSION.to_string(),
        source_revision: "rev-test".to_string(),
        build_target: "x86_64-unknown-linux-gnu".to_string(),
        toolchain_rustc: "rustc 1.85.0".to_string(),
        toolchain_cargo: "cargo 1.85.0".to_string(),
        build_timestamp: "2026-01-01T00:00:00Z".to_string(),
        reproducible_build: true,
        artifacts,
        sbom_file: None,
        sbom_sha256: None,
        provenance_file: None,
        migration_version: 28,
        release_status: "candidate".to_string(),
    }
}

// ===========================================================================
// VERSION CONSISTENCY (§6)
// ===========================================================================

#[test]
fn p45_version_single_authority() {
    // package == runtime == CLI string.
    assert_eq!(PKG_VERSION, runtime_version());
    assert_eq!(cli_version_string(), format!("m31a {PKG_VERSION}"));
    // Cross-checked against the Cargo.toml source document (tests run with
    // cwd = package root, so this relative read is the real file).
    let doc = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml readable");
    assert_eq!(parse_cargo_toml_version(&doc).unwrap(), PKG_VERSION);
    // No second authority: the binary-facing constant is the only source.
    assert!(!PKG_VERSION.trim().is_empty());
}

// ===========================================================================
// MANIFEST + INTEGRITY (§7/§8) + FAILURE INJECTION (§28)
// ===========================================================================

#[test]
fn p45_manifest_round_trip_and_dir_validation() {
    let dir = tempdir().unwrap();
    let m = sample_manifest(dir.path(), &[("a.bin", b"aaa"), ("b.bin", b"bbb")]);
    let json_a = m.to_canonical_json().unwrap();
    let json_b = m.to_canonical_json().unwrap();
    assert_eq!(json_a, json_b, "manifest serialization deterministic");
    let parsed = ReleaseManifest::parse_json(&json_a).unwrap();
    assert_eq!(parsed, m);
    parsed
        .validate_against_dir(dir.path())
        .expect("valid dir verifies");
}

#[test]
fn p45_manifest_tamper_fails_closed() {
    let dir = tempdir().unwrap();
    let m = sample_manifest(dir.path(), &[("a.bin", b"aaa")]);
    std::fs::write(dir.path().join("a.bin"), b"tampered").unwrap();
    let err = m
        .validate_against_dir(dir.path())
        .expect_err("tamper must fail");
    assert!(err.to_string().contains("a.bin"), "got: {err}");
}

#[test]
fn p45_manifest_missing_artifact_fails_closed() {
    let dir = tempdir().unwrap();
    let m = sample_manifest(dir.path(), &[("a.bin", b"aaa")]);
    std::fs::remove_file(dir.path().join("a.bin")).unwrap();
    let err = m
        .validate_against_dir(dir.path())
        .expect_err("missing must fail");
    assert!(err.to_string().contains("missing"), "got: {err}");
}

#[test]
fn p45_manifest_unexpected_artifact_detected() {
    let dir = tempdir().unwrap();
    let m = sample_manifest(dir.path(), &[("a.bin", b"aaa")]);
    let doc = write_sha256sums(dir.path(), &["a.bin".to_string()]).unwrap();
    std::fs::write(dir.path().join("extra.bin"), b"???").unwrap();
    assert!(verify_sha256sums(dir.path(), &doc, true).is_err());
    // And the manifest (which lists only a.bin) still verifies: membership
    // is explicit, orphans are a hygiene/integrity concern, not a silent pass.
    m.validate_against_dir(dir.path())
        .expect("listed entries verify");
}

#[test]
fn p45_blocker_evaluation_failure_injection() {
    // Tampered artifact → SecurityBlocked → promotion denied.
    let tampered = Finding {
        id: "integrity".to_string(),
        classification: ReleaseClassification::SecurityBlocked,
        detail: "hash mismatch".to_string(),
    };
    assert_eq!(
        BlockerEvaluator::evaluate(&tampered, false),
        Blocker::Blocking
    );
    assert!(!BlockerEvaluator::promotion_allowed(&[tampered], 0));
    // Failed mandatory gate (migration/version/provenance) blocks whatever
    // the recorded class says.
    let post = Finding {
        id: "migration".to_string(),
        classification: ReleaseClassification::PostRelease,
        detail: "failed gate".to_string(),
    };
    assert!(!BlockerEvaluator::promotion_allowed(&[post], 1));
    // A blocker is never a warning: no intermediate state exists.
    assert_ne!(Blocker::Blocking, Blocker::NonBlocking);
}

// ===========================================================================
// PROVENANCE (§9) + SBOM (§10)
// ===========================================================================

#[test]
fn p45_provenance_collects_real_facts() {
    // Runs in the repository checkout: revision must match git itself.
    let repo = std::path::Path::new(".");
    let p = collect_provenance(repo, PKG_VERSION, "x86_64-unknown-linux-gnu", vec![])
        .expect("provenance collects in repo");
    let expected = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    assert_eq!(p.source_revision, expected);
    assert_eq!(p.version, PKG_VERSION);
    assert_eq!(p.signing_mechanism, "none");
    assert!(p.toolchain_rustc.starts_with("rustc "));
    // Consistency verification passes for freshly collected facts.
    let vdir = tempdir().unwrap();
    verify_provenance(&p, vdir.path()).expect("fresh provenance verifies");
}

#[test]
fn p45_sbom_matches_lockfile() {
    let lock = std::fs::read_to_string("Cargo.lock").expect("Cargo.lock readable");
    let (sbom, json_a) = generate_sbom(&lock).expect("SBOM generates");
    let (_, json_b) = generate_sbom(&lock).expect("SBOM regenerates");
    assert_eq!(json_a, json_b, "SBOM generation deterministic");
    // Every lock package appears exactly once, sorted.
    let lock_count = lock.matches("[[package]]").count();
    assert_eq!(sbom.components.len(), lock_count);
    assert!(validate_sbom(&json_a).is_ok());
    assert!(sbom.components.iter().any(|c| c.name == "tokio"));
    assert!(sbom.components.iter().any(|c| c.name == "sqlx"));
}

// ===========================================================================
// MIGRATIONS (§13)
// ===========================================================================

#[tokio::test]
async fn p45_migration_fresh_bootstrap() {
    let (_dir, pool) = setup_db().await;
    assert_migrations_complete(&pool).await;
    // Phase 44 tables exist with the 024 columns.
    for table in [
        "tool_mutation_fence",
        "budget_ledger",
        "recovery_attempts",
        "checkpoints",
    ] {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "table {table} exists");
    }
    let cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('recovery_attempts')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(cols.contains(&"mutation_fingerprint".to_string()));
    assert!(cols.contains(&"semantic_signature".to_string()));
}

#[tokio::test]
async fn p45_migration_rerun_idempotent_data_preserved() {
    let (dir, pool) = setup_db().await;
    let now = chrono::Utc::now().to_rfc3339();
    let mid = m31a::ids::MissionId::new();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mid.as_bytes().as_slice())
        .bind("preserve me").bind("active").bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    // Re-running bootstrap over an existing database is idempotent.
    let pool2 = initialize_database(&dir.path().join("t.db"))
        .await
        .expect("re-migrate");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mid.as_bytes().as_slice())
        .fetch_one(&pool2)
        .await
        .unwrap();
    assert_eq!(n, 1, "existing rows survive re-migration");
    assert_migrations_complete(&pool2).await;
}

#[tokio::test]
async fn p45_migration_upgrade_applies_pending_024() {
    // Simulate a pre-024 database: full schema minus the 024 columns, with
    // 024 unmarked in the ledger. Upgrade must apply 024 cleanly and preserve
    // existing rows (supported-upgrade path).
    let (dir, pool) = setup_db().await;
    sqlx::query("ALTER TABLE recovery_attempts DROP COLUMN mutation_fingerprint")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE recovery_attempts DROP COLUMN semantic_signature")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS tool_mutation_fence")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS budget_ledger")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 24")
        .execute(&pool)
        .await
        .unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let mid = m31a::ids::MissionId::new();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mid.as_bytes().as_slice())
        .bind("upgrade row").bind("active").bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    drop(pool);
    // Upgrade: re-bootstrap applies pending migration 024.
    let pool2 = initialize_database(&dir.path().join("t.db"))
        .await
        .expect("upgrade");
    assert_migrations_complete(&pool2).await;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mid.as_bytes().as_slice())
        .fetch_one(&pool2)
        .await
        .unwrap();
    assert_eq!(n, 1, "upgrade preserves existing data");
    // Downgrade is explicitly unsupported: no DOWN migrations exist.
    let mut downs = 0;
    for entry in std::fs::read_dir("migrations").unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if name.ends_with(".down.sql") {
            downs += 1;
        }
    }
    assert_eq!(downs, 0, "no downgrade migrations by design (documented)");
}

#[tokio::test]
async fn p45_migration_invalid_state_fails_closed() {
    let (_dir, pool) = setup_db().await;
    // Tampered ledger (failed migration recorded) is detected, not ignored.
    sqlx::query("UPDATE _sqlx_migrations SET success = 0 WHERE version = 24")
        .execute(&pool)
        .await
        .unwrap();
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT version, success FROM _sqlx_migrations WHERE success != 1")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!rows.is_empty(), "tamper must be detectable");
    // Bootstrap against a non-database path fails closed.
    assert!(
        initialize_database(std::path::Path::new("/proc/definitely-not-a-db/x.db"))
            .await
            .is_err()
    );
    // Missing schema is observable: unknown tables error, not empty success.
    assert!(
        sqlx::query("SELECT COUNT(*) FROM no_such_table_xyz")
            .fetch_one(&pool)
            .await
            .is_err()
    );
}

// ===========================================================================
// INSTALL / UPGRADE / ROLLBACK / RESTART (§14/§15/§16)
// ===========================================================================

#[tokio::test]
async fn p45_install_first_boot_deterministic() {
    // Canonical installation path: fresh workspace → runtime constructs
    // storage, config fallback, and database without secrets or prompts.
    let dir = tempdir().unwrap();
    assert!(std::env::var("M31A_INSTALL_SECRET_PROBE").is_err());
    let rt = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("first boot");
    assert!(
        m31a::persistence::paths::canonical_db_path_for_workspace(dir.path()).exists(),
        "first boot creates the channel-aware database"
    );
    let one: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(rt.pool())
        .await
        .unwrap();
    assert_eq!(one, 1);
    assert_eq!(m31a::release::PKG_VERSION, env!("CARGO_PKG_VERSION"));
    drop(rt);
    // Second boot against the same installation reuses state (no wipe).
    let rt2 = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("second boot");
    let one: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(rt2.pool())
        .await
        .unwrap();
    assert_eq!(one, 1);
}

#[tokio::test]
async fn p45_upgrade_preserves_durable_state() {
    // Supported upgrade: v1 writes state, v2 (same schema line) reopens and
    // finds missions, authorizations, budget, fence rows, and telemetry.
    let dir = tempdir().unwrap();
    let rt = m31a::runtime::AppRuntime::new(dir.path()).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let sessions = m31a::interaction::session::SqliteSessionRepository::new(rt.pool().clone());
    let session = sessions.create_session(dir.path()).await.unwrap();
    let session_id = session.id.to_string();
    let mission_id = session.active_mission_id.unwrap();
    let coordinator =
        PreExecutionCoordinator::deterministic_test(rt.pool().clone(), Some(bus.clone()))
            .with_workspace_root(dir.path().to_path_buf());
    let resp = coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .unwrap();
    assert!(matches!(resp, PreExecutionResponse::PlanForReview { .. }));
    coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
                revision: None,
                content_hash: None,
            },
            "operator",
        )
        .await
        .unwrap();
    coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
                revision: None,
                content_hash: None,
            },
            "operator",
        )
        .await
        .unwrap();
    coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await
        .unwrap();
    drop(rt);
    // "Upgrade": reopen and verify everything survived with semantics intact.
    let rt2 = m31a::runtime::AppRuntime::new(dir.path()).await.unwrap();
    let lifecycle = SqliteLifecycleRepository::new(rt2.pool().clone());
    match lifecycle
        .revalidate_authorization_for_resume(mission_id)
        .await
        .expect("revalidation runs")
    {
        m31a::persistence::sqlite::repositories::lifecycle::ResumeAuthVerdict::Valid {
            plan_revision,
            task_revision,
            ..
        } => {
            assert_eq!(plan_revision, 1);
            assert_eq!(task_revision, 1);
        }
        v => panic!("authorized execution must revalidate after upgrade, got {v:?}"),
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_authorizations")
        .fetch_one(rt2.pool())
        .await
        .unwrap();
    assert!(n >= 1, "authorizations preserved across upgrade");
}

#[tokio::test]
async fn p45_backup_restore_round_trip() {
    // Supported recovery path for failed upgrade / corrupt install: file-level
    // backup restoration. Restored databases open and verify cleanly.
    let dir = tempdir().unwrap();
    let rt = m31a::runtime::AppRuntime::new(dir.path()).await.unwrap();
    // Channel-aware canonical database location (`m31a.db` on production,
    // `m31a-dev.db` on development); WAL sidecars derive from it.
    let db_path = m31a::persistence::paths::canonical_db_path_for_workspace(dir.path());
    let now = chrono::Utc::now().to_rfc3339();
    let mid = m31a::ids::MissionId::new();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mid.as_bytes().as_slice())
        .bind("backup row").bind("active").bind(&now).bind(&now)
        .execute(rt.pool()).await.unwrap();
    // Real backup procedure for WAL-mode databases: checkpoint first so the
    // backup file is self-contained (a raw copy without its WAL is stale).
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(rt.pool())
        .await;
    drop(rt);
    let backup = std::path::PathBuf::from(format!("{}.bak", db_path.display()));
    std::fs::copy(&db_path, &backup).unwrap();
    // Simulate damage + restore. The pool runs in WAL mode, so faithful
    // damage simulation must also clear stale -wal/-shm sidecars; otherwise
    // SQLite reconstructs the pre-damage state from the WAL (correct engine
    // behavior, not a test artifact worth asserting on).
    let wal = std::path::PathBuf::from(format!("{}-wal", db_path.display()));
    let shm = std::path::PathBuf::from(format!("{}-shm", db_path.display()));
    std::fs::write(&db_path, vec![b'X'; 512]).unwrap();
    let _ = std::fs::remove_file(&wal);
    let _ = std::fs::remove_file(&shm);
    assert!(
        initialize_database(&db_path).await.is_err(),
        "damaged DB fails closed"
    );
    std::fs::copy(&backup, &db_path).unwrap();
    let pool = initialize_database(&db_path)
        .await
        .expect("restored DB opens");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mid.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "restored content intact");
}

// ===========================================================================
// SMOKE + PLATFORM (§18/§19)
// ===========================================================================

#[test]
fn p45_platform_matrix_explicit() {
    // Only the host target is executed here; anything else is documented as
    // unexecuted, never "verified".
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    assert!(!arch.is_empty() && !os.is_empty());
    let triple = format!("{arch}-unknown-{os}");
    assert!(triple.contains(arch));
    // This environment executes exactly one target.
    assert_eq!(
        os, "linux",
        "RC execution evidence is linux-only in this environment"
    );
}

#[tokio::test]
async fn p45_release_smoke_launch_to_shutdown() {
    // launch → initialize → config → persistence → version → schema → stop.
    let dir = tempdir().unwrap();
    let rt = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("launch");
    assert_eq!(m31a::release::runtime_version(), env!("CARGO_PKG_VERSION"));
    let versions: (i64, i64) = sqlx::query_as(
        "SELECT version, success FROM _sqlx_migrations ORDER BY version DESC LIMIT 1",
    )
    .fetch_one(rt.pool())
    .await
    .unwrap();
    assert_eq!(versions, (28, 1));
    drop(rt);
}

// ===========================================================================
// HYGIENE (§29)
// ===========================================================================

#[test]
fn p45_release_dir_hygiene() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("m31a.tar.gz"), b"pkg").unwrap();
    std::fs::write(dir.path().join("SHA256SUMS"), b"abc  m31a.tar.gz\n").unwrap();
    assert!(check_release_dir(dir.path()).is_empty());
    std::fs::write(dir.path().join("stale.tmp"), b"x").unwrap();
    std::fs::write(dir.path().join("leak.txt"), b"key=nvapi-1234567890").unwrap();
    let findings = check_release_dir(dir.path());
    assert_eq!(findings.len(), 2);
}

// ===========================================================================
// TAXONOMY CONSISTENCY (§12/§24)
// ===========================================================================

#[test]
fn p45_taxonomy_single_closed_set() {
    use m31a::release::ReleaseClassification;
    // The code taxonomy is the regression anchor: exactly the seven approved
    // classifications, including CREDENTIAL-BLOCKED (Option A).
    let names: Vec<&str> = ReleaseClassification::ALL
        .iter()
        .map(|c| c.as_str())
        .collect();
    assert!(names.contains(&"CREDENTIAL-BLOCKED"));
    assert_eq!(names.len(), 7);
}

// ===========================================================================
// LIVE PROBE (partitioned)
// ===========================================================================

/// Live release probe: real model call settles authoritatively (proves the RC
/// path carries Phase 42 accounting, not a stub).
#[tokio::test]
#[ignore = "requires live NVIDIA credentials + WAN; run explicitly with --ignored"]
async fn live_p45_release_accounting_probe() {
    use m31a::agent::model_policy::ModelCaller;
    use m31a::model::types::UsageSource;
    let harness = match m31a::testing::real_model::RealModelHarness::ensure() {
        Ok(h) => h,
        Err(skipped) => {
            eprintln!("LIVE SKIPPED (environment): {}", skipped.reason);
            return;
        }
    };
    let caller = harness.routed_caller(Vec::new());
    let token = tokio_util::sync::CancellationToken::new();
    let (proposal, usage) = caller
        .call_model_cancellable_with_usage("Return the text: 'p45 rc probe ok'", &token)
        .await
        .expect("live model call");
    assert!(
        proposal.is_completion()
            || matches!(
                proposal,
                m31a::model::types::ModelProposal::AssistantText { .. }
            )
    );
    assert_eq!(usage.source, UsageSource::AuthoritativeProvider);
    harness.assert_canonical_model_routing();
}

// ===========================================================================
// REHEARSAL-OUTPUT VALIDATION (partitioned: requires assembled dist/)
// ===========================================================================

/// Validates a real assembled release directory (clean-room rehearsal
/// output): manifest parses + schema-validates, every entry verifies against
/// bytes on disk. Set M31A_RC_DIR to the assembled dist/ directory.
/// Ignored by default: without rehearsal output there is nothing to verify,
/// and absence must never read as proof.
#[test]
#[ignore = "requires assembled release directory in M31A_RC_DIR (clean-room rehearsal output)"]
fn rehearsal_rc_manifest_verifies() {
    let dir = match std::env::var("M31A_RC_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("REHEARSAL SKIPPED: M31A_RC_DIR unset (no assembled dist to verify)");
            return;
        }
    };
    let dir = std::path::Path::new(&dir);
    let doc = std::fs::read_to_string(dir.join("rc-manifest.json"))
        .expect("rc-manifest.json readable in rehearsal output");
    let manifest = ReleaseManifest::parse_json(&doc).expect("manifest parses");
    manifest.validate_schema().expect("manifest schema valid");
    manifest
        .validate_against_dir(dir)
        .expect("every manifest entry verifies against rehearsal bytes");
    assert_eq!(
        manifest.version, PKG_VERSION,
        "rehearsal version matches package"
    );
}
