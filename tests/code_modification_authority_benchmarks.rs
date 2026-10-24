//! Phase 23 — Autonomous Code Modification & Change Authority Integration Tests
//!
//! Benchmark Suite covering the 10 mandated scenarios (Section 42):
//! 1. Single-file bug fix
//! 2. Multi-file feature
//! 3. API change
//! 4. Database-backed feature
//! 5. Configuration change
//! 6. Test-driven bug fix
//! 7. Cross-subsystem refactor
//! 8. Security-sensitive change
//! 9. Stale-patch conflict
//! 10. Unexpected dependency discovered during implementation

use m31a::capability::providers::local_fs::LocalFileSystemProvider;
use m31a::capability::traits::FileSystemService;
use m31a::change::authority::{ChangeAuthority, ChangeAuthorityError};
use m31a::change::provenance::ChangeProvenanceStore;
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, FilePrecondition,
    ImplementationHypothesis,
};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::state_machine::change_set::ChangeSetState;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

// =========================================================================
// Benchmark 1: Single-file bug fix
// =========================================================================
#[tokio::test]
async fn test_benchmark_1_single_file_bug_fix() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let initial_content = b"pub fn add(a: u32, b: u32) -> u32 {\n    a - b\n}\n";
    fs.write_file(Path::new("src/math.rs"), initial_content)
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Fix subtraction typo in add()",
        "Minus operator used instead of plus",
        "Replace '-' with '+'",
        "add(2, 3) returns 5 instead of underflowing/subtracting",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec!["src/math.rs".to_string()]);
    let mutation = FileMutationProposal::new(
        "src/math.rs",
        FileMutationOp::Substring {
            old_content: "a - b".to_string(),
            new_content: "a + b".to_string(),
        },
        "Fix arithmetic operator",
    );

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![mutation],
    );

    let authority = ChangeAuthority::new().with_pool(pool.clone());
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);
    assert_eq!(outcome.files_modified, vec!["src/math.rs"]);

    let updated_bytes = fs
        .read_file(Path::new("src/math.rs"), None, None)
        .await
        .unwrap();
    let updated_str = String::from_utf8(updated_bytes).unwrap();
    assert!(updated_str.contains("a + b"));

    // Verify durable provenance recorded in SQLite
    let provenance_store = ChangeProvenanceStore::new();
    let record = provenance_store
        .get_by_proposal(proposal.id, Some(&pool))
        .await
        .unwrap();
    assert!(record.is_some());
    assert_eq!(record.unwrap().affected_files, vec!["src/math.rs"]);
}

// =========================================================================
// Benchmark 2: Multi-file feature
// =========================================================================
#[tokio::test]
async fn test_benchmark_2_multi_file_feature() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(
        Path::new("src/user.rs"),
        b"pub struct User {\n    pub id: u64,\n}\n",
    )
    .await
    .unwrap();
    fs.write_file(
        Path::new("src/auth.rs"),
        b"pub fn can_login(user: &crate::user::User) -> bool {\n    true\n}\n",
    )
    .await
    .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Add is_active check to User and Auth",
        "Inactive users currently permitted to log in",
        "Add is_active field to User, check in can_login",
        "Inactive users cannot log in",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec!["src/user.rs".to_string(), "src/auth.rs".to_string()]);
    let mutations = vec![
        FileMutationProposal::new(
            "src/user.rs",
            FileMutationOp::Substring {
                old_content: "pub id: u64,\n".to_string(),
                new_content: "pub id: u64,\n    pub is_active: bool,\n".to_string(),
            },
            "Add is_active field to User",
        ),
        FileMutationProposal::new(
            "src/auth.rs",
            FileMutationOp::Substring {
                old_content: "true\n".to_string(),
                new_content: "user.is_active\n".to_string(),
            },
            "Check user.is_active in can_login",
        ),
    ];

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        mutations,
    );

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);
    assert_eq!(outcome.files_modified.len(), 2);

    let user_code = String::from_utf8(
        fs.read_file(Path::new("src/user.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    let auth_code = String::from_utf8(
        fs.read_file(Path::new("src/auth.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();

    assert!(user_code.contains("pub is_active: bool,"));
    assert!(auth_code.contains("user.is_active"));
}

// =========================================================================
// Benchmark 3: API change
// =========================================================================
#[tokio::test]
async fn test_benchmark_3_api_change() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(
        Path::new("src/api.rs"),
        b"pub fn fetch(url: &str) -> String {\n    format!(\"get: {}\", url)\n}\n",
    )
    .await
    .unwrap();
    fs.write_file(
        Path::new("src/client.rs"),
        b"pub fn run() {\n    let _ = crate::api::fetch(\"http://localhost\");\n}\n",
    )
    .await
    .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Add timeout parameter to fetch API",
        "Requests hang without explicit timeout",
        "Add timeout_ms parameter and propagate from client",
        "Requests time out as configured",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec!["src/api.rs".to_string(), "src/client.rs".to_string()]);
    let mutations = vec![
        FileMutationProposal::new(
            "src/api.rs",
            FileMutationOp::Substring {
                old_content: "pub fn fetch(url: &str) -> String".to_string(),
                new_content: "pub fn fetch(url: &str, timeout_ms: u64) -> String".to_string(),
            },
            "Add timeout parameter",
        ),
        FileMutationProposal::new(
            "src/client.rs",
            FileMutationOp::Substring {
                old_content: "crate::api::fetch(\"http://localhost\");".to_string(),
                new_content: "crate::api::fetch(\"http://localhost\", 5000);".to_string(),
            },
            "Pass timeout parameter in client",
        ),
    ];

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        mutations,
    );

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);

    let api_code = String::from_utf8(
        fs.read_file(Path::new("src/api.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    let client_code = String::from_utf8(
        fs.read_file(Path::new("src/client.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();

    assert!(api_code.contains("timeout_ms: u64"));
    assert!(client_code.contains("5000"));
}

// =========================================================================
// Benchmark 4: Database-backed feature
// =========================================================================
#[tokio::test]
async fn test_benchmark_4_database_backed_feature() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let migration_path = "migrations/999_add_tags_table.sql";
    let model_path = "src/models/tag.rs";

    let hypothesis = ImplementationHypothesis::new(
        "Add tags schema and model",
        "Need tagging support for entities",
        "Add SQL migration table and Rust struct",
        "Tags table exists and Tag struct serializable",
        "sqlx migrate run && cargo test",
    );

    let surface = ChangeSurface::new(vec![migration_path.to_string(), model_path.to_string()]);
    let mutations = vec![
        FileMutationProposal::new(
            migration_path,
            FileMutationOp::CreateNew {
                content: "CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT NOT NULL);\n"
                    .to_string(),
            },
            "Create tags SQL migration",
        ),
        FileMutationProposal::new(
            model_path,
            FileMutationOp::CreateNew {
                content: "pub struct Tag {\n    pub id: String,\n    pub name: String,\n}\n"
                    .to_string(),
            },
            "Create Tag struct",
        ),
    ];

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        mutations,
    );

    let authority = ChangeAuthority::new().with_pool(pool.clone());
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);

    // Verify file content created on disk
    let sql = String::from_utf8(
        fs.read_file(Path::new(migration_path), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        sql,
        "CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT NOT NULL);\n"
    );
    sqlx::raw_sql("CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT NOT NULL);\n")
        .execute(&pool)
        .await
        .unwrap();

    // Verify table exists
    let row: (i64,) = sqlx::query_as("SELECT count(*) FROM tags")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, 0);
}

// =========================================================================
// Benchmark 5: Configuration change
// =========================================================================
#[tokio::test]
async fn test_benchmark_5_configuration_change() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(
        Path::new("config/app.toml"),
        b"port = 8080\nmax_connections = 100\n",
    )
    .await
    .unwrap();
    fs.write_file(
        Path::new("src/config.rs"),
        b"pub fn default_port() -> u16 {\n    8080\n}\n",
    )
    .await
    .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Update default port to 9090",
        "Port conflict on 8080",
        "Update TOML and default function",
        "Port 9090 used by default",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec![
        "config/app.toml".to_string(),
        "src/config.rs".to_string(),
    ]);
    let mutations = vec![
        FileMutationProposal::new(
            "config/app.toml",
            FileMutationOp::Substring {
                old_content: "port = 8080".to_string(),
                new_content: "port = 9090".to_string(),
            },
            "Update port in config TOML",
        ),
        FileMutationProposal::new(
            "src/config.rs",
            FileMutationOp::Substring {
                old_content: "8080".to_string(),
                new_content: "9090".to_string(),
            },
            "Update port in config.rs",
        ),
    ];

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        mutations,
    );

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);

    let toml = String::from_utf8(
        fs.read_file(Path::new("config/app.toml"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    let code = String::from_utf8(
        fs.read_file(Path::new("src/config.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();

    assert!(toml.contains("port = 9090"));
    assert!(code.contains("9090"));
}

// =========================================================================
// Benchmark 6: Test-driven bug fix
// =========================================================================
#[tokio::test]
async fn test_benchmark_6_test_driven_bug_fix() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(
        Path::new("src/calc.rs"),
        b"pub fn multiply(a: i32, b: i32) -> i32 {\n    0\n}\n",
    )
    .await
    .unwrap();
    fs.write_file(
        Path::new("tests/calc_test.rs"),
        b"#[test]\nfn test_multiply() {\n    assert_eq!(calc::multiply(3, 4), 12);\n}\n",
    )
    .await
    .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Implement multiply to pass test",
        "multiply returns hardcoded 0",
        "Return a * b",
        "test_multiply passes with 12",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec!["src/calc.rs".to_string()]);
    let mutation = FileMutationProposal::new(
        "src/calc.rs",
        FileMutationOp::Substring {
            old_content: "0\n".to_string(),
            new_content: "a * b\n".to_string(),
        },
        "Implement multiplication logic",
    );

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![mutation],
    );

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);

    let calc_code = String::from_utf8(
        fs.read_file(Path::new("src/calc.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(calc_code.contains("a * b"));
}

// =========================================================================
// Benchmark 7: Cross-subsystem refactor
// =========================================================================
#[tokio::test]
async fn test_benchmark_7_cross_subsystem_refactor() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(
        Path::new("src/security/token.rs"),
        b"pub struct SecurityToken(pub String);\n",
    )
    .await
    .unwrap();
    fs.write_file(
        Path::new("src/capability/session.rs"),
        b"use crate::security::token::SecurityToken;\npub fn validate(tok: &SecurityToken) -> bool {\n    !tok.0.is_empty()\n}\n",
    )
    .await
    .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Refactor SecurityToken to use strong typing with expiry",
        "SecurityToken lacks expiration metadata",
        "Add expires_at to SecurityToken and check in session validation",
        "Expired tokens rejected",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec![
        "src/security/token.rs".to_string(),
        "src/capability/session.rs".to_string(),
    ]);

    let mutations = vec![
        FileMutationProposal::new(
            "src/security/token.rs",
            FileMutationOp::Substring {
                old_content: "pub struct SecurityToken(pub String);".to_string(),
                new_content: "pub struct SecurityToken { pub secret: String, pub expires_at: u64 }"
                    .to_string(),
            },
            "Add expires_at to SecurityToken",
        ),
        FileMutationProposal::new(
            "src/capability/session.rs",
            FileMutationOp::Substring {
                old_content: "!tok.0.is_empty()".to_string(),
                new_content: "!tok.secret.is_empty() && tok.expires_at > 0".to_string(),
            },
            "Validate token expiry in session",
        ),
    ];

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        mutations,
    );

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);

    let token_code = String::from_utf8(
        fs.read_file(Path::new("src/security/token.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    let session_code = String::from_utf8(
        fs.read_file(Path::new("src/capability/session.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();

    assert!(token_code.contains("pub expires_at: u64"));
    assert!(session_code.contains("tok.expires_at > 0"));
}

// =========================================================================
// Benchmark 8: Security-sensitive change
// =========================================================================
#[tokio::test]
async fn test_benchmark_8_security_sensitive_change() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original_bytes = b"pub fn is_authorized(role: &str) -> bool {\n    role == \"admin\"\n}\n";
    fs.write_file(Path::new("src/security/policy.rs"), original_bytes)
        .await
        .unwrap();

    let expected_hash = sha256_hex(original_bytes);

    let hypothesis = ImplementationHypothesis::new(
        "Allow auditor role read-only access",
        "Auditors currently locked out",
        "Accept 'admin' or 'auditor'",
        "Auditor has access",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec!["src/security/policy.rs".to_string()]);
    let mut mutation = FileMutationProposal::new(
        "src/security/policy.rs",
        FileMutationOp::Substring {
            old_content: "role == \"admin\"".to_string(),
            new_content: "role == \"admin\" || role == \"auditor\"".to_string(),
        },
        "Expand authorized roles",
    );
    mutation.base_hash = Some(expected_hash);

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![mutation],
    );

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(outcome.state, ChangeSetState::Accepted);

    let code = String::from_utf8(
        fs.read_file(Path::new("src/security/policy.rs"), None, None)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(code.contains("role == \"auditor\""));
}

// =========================================================================
// Benchmark 9: Stale-patch conflict
// =========================================================================
#[tokio::test]
async fn test_benchmark_9_stale_patch_conflict() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original_bytes = b"pub fn worker() -> &'static str { \"v1\" }\n";
    fs.write_file(Path::new("src/worker.rs"), original_bytes)
        .await
        .unwrap();

    // Model assumes base hash is H_OLD
    let stale_hash = "deadbeef00000000000000000000000000000000000000000000000000000000".to_string();

    let hypothesis = ImplementationHypothesis::new(
        "Upgrade worker to v2",
        "Worker is on v1",
        "Return v2",
        "Worker returns v2",
        "cargo test",
    );

    let surface = ChangeSurface::new(vec!["src/worker.rs".to_string()]);
    let mut proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "src/worker.rs",
            FileMutationOp::Substring {
                old_content: "\"v1\"".to_string(),
                new_content: "\"v2\"".to_string(),
            },
            "Upgrade version",
        )],
    );
    // Explicit precondition with stale hash
    proposal
        .preconditions
        .push(FilePrecondition::exists_with_hash(
            "src/worker.rs",
            stale_hash,
        ));

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Conflicted)
    );

    // Workspace MUST be untouched
    let current_bytes = fs
        .read_file(Path::new("src/worker.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(current_bytes, original_bytes);
}

// =========================================================================
// Benchmark 10: Unexpected dependency discovered during implementation
// =========================================================================
#[tokio::test]
async fn test_benchmark_10_unexpected_dependency_discovered() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(Path::new("src/core.rs"), b"pub fn core() {}\n")
        .await
        .unwrap();
    fs.write_file(Path::new("src/plugin.rs"), b"pub fn plugin() {}\n")
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new(
        "Update core and secretly also plugin",
        "Plugin is affected but model omitted it from surface",
        "Modify core and plugin",
        "Both modified",
        "cargo test",
    );

    // Surface ONLY declares src/core.rs
    let surface = ChangeSurface::new(vec!["src/core.rs".to_string()]);

    // Proposal attempts to modify src/core.rs AND undeclared src/plugin.rs
    let mutations = vec![
        FileMutationProposal::new(
            "src/core.rs",
            FileMutationOp::Substring {
                old_content: "pub fn core() {}".to_string(),
                new_content: "pub fn core_v2() {}".to_string(),
            },
            "Update core",
        ),
        FileMutationProposal::new(
            "src/plugin.rs",
            FileMutationOp::Substring {
                old_content: "pub fn plugin() {}".to_string(),
                new_content: "pub fn plugin_v2() {}".to_string(),
            },
            "Update plugin (undeclared)",
        ),
    ];

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        mutations,
    );

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    // Reconciler detects scope creep: undeclared file!
    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Rejected)
    );

    // Neither file was modified on disk!
    let core_bytes = fs
        .read_file(Path::new("src/core.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(core_bytes, b"pub fn core() {}\n");
    let plugin_bytes = fs
        .read_file(Path::new("src/plugin.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(plugin_bytes, b"pub fn plugin() {}\n");
}
