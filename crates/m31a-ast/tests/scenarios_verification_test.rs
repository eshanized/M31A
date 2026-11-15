//! Scenarios A through F Verification Test Suite for M31A Initiative 1
//! (Repository Intelligence Engine).

use m31a_ast::*;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// Scenario A: Pure Rust Workspace (M31A Workspace self-audit)
#[test]
fn test_scenario_a_pure_rust_workspace() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // 1. Workspace discovery finds all member crates
    let inventory = WorkspaceDiscovery::discover_workspace(&repo_root);
    assert!(
        inventory.packages.len() >= 15,
        "Expected at least 15 packages in M31A workspace, found {}",
        inventory.packages.len()
    );

    let pkg_names: Vec<&str> = inventory.packages.iter().map(|p| p.name.as_str()).collect();
    assert!(pkg_names.contains(&"m31a-ast"));
    assert!(pkg_names.contains(&"m31a-core"));
    assert!(pkg_names.contains(&"m31a-storage"));
    assert!(pkg_names.contains(&"m31a-tools"));

    // 2. Package graph construction & cycle detection
    let pkg_graph = PackageGraph::build(&inventory);
    let pkg_cycles = pkg_graph.detect_cycles();
    assert!(
        pkg_cycles.is_empty(),
        "Detected circular package dependencies in M31A: {:?}",
        pkg_cycles
    );

    // 3. Traversal and symbol extraction across crates
    let options = TraversalOptions {
        max_files: 5000,
        ..Default::default()
    };
    let index = AstCodebaseIndex::build_with_options(&repo_root, options);
    assert_eq!(index.status(), IndexStatus::Complete);
    assert!(
        index.files_indexed() > 10,
        "Expected > 10 indexed files, got {}",
        index.files_indexed()
    );
    assert!(
        index.total_symbols() > 50,
        "Expected > 50 symbols, got {}",
        index.total_symbols()
    );

    // 4. Cross-file/cross-crate caller discovery
    let callers = index.find_references_to("parse_file");
    // At least one caller or reference exists in workspace
    assert!(
        !callers.is_empty() || !index.find_symbols_by_name("parse_file").is_empty(),
        "Expected symbol resolution for parse_file"
    );

    // 5. Impact analysis for modifying a core AST type
    let target_file = repo_root.join("crates/m31a-ast/src/types.rs");
    if target_file.exists() {
        let impact = SemanticImpactAnalyzer::analyze_impact(
            &index,
            &[target_file],
            &["AstSymbol".to_string()],
        );
        assert!(!impact.target_files.is_empty());
        assert!(impact.blast_radius_score >= 0.0);
    }
}

/// Scenario B: TypeScript / JavaScript Project
#[test]
fn test_scenario_b_typescript_javascript_project() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();

    // 1. Package boundaries and tsconfig
    let pkg_json = r#"{
  "name": "enterprise-ts-app",
  "version": "2.4.0",
  "main": "dist/index.js"
}"#;
    fs::write(ws.join("package.json"), pkg_json).unwrap();

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "CommonJS",
    "strict": true
  }
}"#;
    fs::write(ws.join("tsconfig.json"), tsconfig).unwrap();

    // 2. Source code with classes, interfaces, types, functions, exports
    fs::create_dir_all(ws.join("src")).unwrap();
    let user_ts = r#"export interface User {
    id: string;
    email: string;
    role: string;
}

export type UserId = string;

export class UserService {
    private users: Map<string, User> = new Map();

    public async findUserById(id: UserId): Promise<User | null> {
        return this.users.get(id) || null;
    }

    public registerUser(user: User): void {
        this.users.set(user.id, user);
    }
}

export function createDefaultUser(): User {
    return { id: "u-1", email: "admin@corp.io", role: "admin" };
}
"#;
    fs::write(ws.join("src/user.ts"), user_ts).unwrap();

    let service_ts = r#"import { UserService, createDefaultUser } from "./user";

export class OrderService {
    private userService = new UserService();

    public processOrder(orderId: string): void {
        const u = createDefaultUser();
        this.userService.registerUser(u);
    }
}
"#;
    fs::write(ws.join("src/service.ts"), service_ts).unwrap();

    // 3. Test file
    fs::create_dir_all(ws.join("tests")).unwrap();
    let test_ts = r#"import { UserService, createDefaultUser } from "../src/user";

describe("UserService", () => {
    it("finds registered user", async () => {
        const service = new UserService();
        const u = createDefaultUser();
        service.registerUser(u);
        const found = await service.findUserById(u.id);
        expect(found).not.toBeNull();
    });
});
"#;
    fs::write(ws.join("tests/user.test.ts"), test_ts).unwrap();

    // Discovery & Indexing
    let inventory = WorkspaceDiscovery::discover_workspace(ws);
    assert_eq!(inventory.packages.len(), 1);
    assert_eq!(inventory.packages[0].name, "enterprise-ts-app");

    let index = AstCodebaseIndex::build_with_options(ws, TraversalOptions::default());
    assert_eq!(index.status(), IndexStatus::Complete);

    // Check extracted symbols
    let user_syms = index.find_symbols_by_name("UserService");
    assert!(!user_syms.is_empty(), "Expected UserService class");
    assert_eq!(user_syms[0].kind, AstSymbolKind::Class);

    let iface_syms = index.find_symbols_by_name("User");
    assert!(!iface_syms.is_empty(), "Expected User interface");

    let fn_syms = index.find_symbols_by_name("createDefaultUser");
    assert!(!fn_syms.is_empty(), "Expected createDefaultUser function");

    // Check test association
    let tests = index.find_related_tests("createDefaultUser", &ws.join("src/user.ts"));
    assert!(
        !tests.is_empty(),
        "Expected tests to be found for createDefaultUser"
    );
}

/// Scenario C: Python Package
#[test]
fn test_scenario_c_python_package() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();

    // 1. pyproject.toml
    let pyproject = r#"[project]
name = "math_analytics"
version = "1.0.0"
dependencies = ["numpy", "scipy"]
"#;
    fs::write(ws.join("pyproject.toml"), pyproject).unwrap();

    // 2. Implementation modules
    fs::create_dir_all(ws.join("src/math_analytics")).unwrap();
    let engine_py = r#"from typing import List, Optional

class AnalyticsEngine:
    """Core analytics engine."""

    def __init__(self, precision: int = 4):
        self.precision = precision

    def compute_variance(self, data: List[float]) -> float:
        if not data:
            return 0.0
        mean = self.compute_mean(data)
        return sum((x - mean) ** 2 for x in data) / len(data)

    def compute_mean(self, data: List[float]) -> float:
        return sum(data) / len(data)

def normalize_dataset(raw_data: List[float]) -> List[float]:
    mean = sum(raw_data) / len(raw_data)
    return [x - mean for x in raw_data]
"#;
    fs::write(ws.join("src/math_analytics/engine.py"), engine_py).unwrap();

    // 3. Pytest tests
    fs::create_dir_all(ws.join("tests")).unwrap();
    let test_py = r#"import pytest
from math_analytics.engine import AnalyticsEngine, normalize_dataset

def test_variance_calculation():
    engine = AnalyticsEngine()
    data = [1.0, 2.0, 3.0, 4.0, 5.0]
    var = engine.compute_variance(data)
    assert var > 0.0

def test_normalize():
    norm = normalize_dataset([10.0, 20.0, 30.0])
    assert len(norm) == 3
"#;
    fs::write(ws.join("tests/test_engine.py"), test_py).unwrap();

    // Discovery & Indexing
    let inventory = WorkspaceDiscovery::discover_workspace(ws);
    assert_eq!(inventory.packages.len(), 1);
    assert_eq!(inventory.packages[0].name, "math_analytics");

    let index = AstCodebaseIndex::build_with_options(ws, TraversalOptions::default());
    assert_eq!(index.status(), IndexStatus::Complete);

    // Symbol verification
    let engine_syms = index.find_symbols_by_name("AnalyticsEngine");
    assert!(!engine_syms.is_empty());
    assert_eq!(engine_syms[0].kind, AstSymbolKind::Class);

    let method_syms = index.find_symbols_by_name("compute_variance");
    assert!(!method_syms.is_empty());
    assert_eq!(method_syms[0].kind, AstSymbolKind::Method);

    // Related test discovery
    let tests = index.find_related_tests(
        "normalize_dataset",
        &ws.join("src/math_analytics/engine.py"),
    );
    assert!(
        !tests.is_empty(),
        "Expected related test for normalize_dataset"
    );
}

/// Scenario D: Go Module
#[test]
fn test_scenario_d_go_module() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();

    // 1. go.mod
    let gomod = r#"module github.com/example/streamgateway

go 1.22
"#;
    fs::write(ws.join("go.mod"), gomod).unwrap();

    // 2. Server package
    fs::create_dir_all(ws.join("server")).unwrap();
    let server_go = r#"package server

import (
	"fmt"
	"sync"
)

type Router interface {
	Route(path string) bool
}

type GatewayServer struct {
	port int
	mu   sync.RWMutex
}

func NewGatewayServer(port int) *GatewayServer {
	return &GatewayServer{port: port}
}

func (s *GatewayServer) Start() error {
	fmt.Printf("Starting on %d\n", s.port)
	return nil
}
"#;
    fs::write(ws.join("server/server.go"), server_go).unwrap();

    // 3. Unit test
    let server_test_go = r#"package server

import (
	"testing"
)

func TestGatewayServerStart(t *testing.T) {
	srv := NewGatewayServer(8080)
	if err := srv.Start(); err != nil {
		t.Fatalf("failed to start: %v", err)
	}
}
"#;
    fs::write(ws.join("server/server_test.go"), server_test_go).unwrap();

    // Discovery & Indexing
    let inventory = WorkspaceDiscovery::discover_workspace(ws);
    assert_eq!(inventory.packages.len(), 1);
    assert_eq!(
        inventory.packages[0].name,
        "github.com/example/streamgateway"
    );

    let index = AstCodebaseIndex::build_with_options(ws, TraversalOptions::default());
    assert_eq!(index.status(), IndexStatus::Complete);

    // Symbols
    let srv_syms = index.find_symbols_by_name("GatewayServer");
    assert!(!srv_syms.is_empty());
    assert_eq!(srv_syms[0].kind, AstSymbolKind::Struct);

    let iface_syms = index.find_symbols_by_name("Router");
    assert!(!iface_syms.is_empty());
    assert_eq!(iface_syms[0].kind, AstSymbolKind::Interface);

    let method_syms = index.find_symbols_by_name("Start");
    assert!(!method_syms.is_empty());
    assert_eq!(method_syms[0].kind, AstSymbolKind::Method);

    // Tests
    let tests = index.find_related_tests("NewGatewayServer", &ws.join("server/server.go"));
    assert!(
        !tests.is_empty(),
        "Expected related test for NewGatewayServer"
    );
}

/// Scenario E: Incremental Persistence and Recovery
#[test]
fn test_scenario_e_incremental_persistence_and_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();

    let cargo_toml = r#"[package]
name = "incremental-demo"
version = "0.1.0"
edition = "2024"
"#;
    fs::write(ws.join("Cargo.toml"), cargo_toml).unwrap();
    fs::create_dir_all(ws.join("src")).unwrap();

    let file_a = ws.join("src/a.rs");
    let file_b = ws.join("src/b.rs");

    fs::write(&file_a, "pub fn helper_one() -> i32 { 101 }\n").unwrap();
    fs::write(
        &file_b,
        "use crate::a::helper_one;\npub fn helper_two() -> i32 { helper_one() + 1 }\n",
    )
    .unwrap();

    // 1. Initial build & timing
    let start_initial = Instant::now();
    let mut index = AstCodebaseIndex::build_with_options(ws, TraversalOptions::default());
    let initial_duration = start_initial.elapsed();

    assert_eq!(index.files_indexed(), 2);
    assert_eq!(index.total_symbols(), 2);

    let hash_a_before = index.file_hashes().get(&file_a).cloned().unwrap();

    // 2. Modify only file_a
    let start_incremental = Instant::now();
    let updated_a = "pub fn helper_one() -> i32 { 999 }\npub fn extra_func() {}\n";
    fs::write(&file_a, updated_a).unwrap();

    index.update_file(&file_a, updated_a);
    let incremental_duration = start_incremental.elapsed();

    // 3. Confirm only file_a was updated and new symbol is present
    let hash_a_after = index.file_hashes().get(&file_a).cloned().unwrap();
    assert_ne!(
        hash_a_before, hash_a_after,
        "Hash of modified file must change"
    );

    let extra_syms = index.find_symbols_by_name("extra_func");
    assert_eq!(extra_syms.len(), 1, "New symbol must be indexed");
    assert_eq!(index.total_symbols(), 3);

    // 4. File removal
    let removed = index.remove_file(&file_b);
    assert!(removed, "File removal must succeed");
    assert_eq!(index.total_symbols(), 2);
    assert!(index.find_symbols_by_name("helper_two").is_empty());

    println!(
        "Scenario E Timings: Initial build={:?}, Incremental update={:?}",
        initial_duration, incremental_duration
    );
}

/// Scenario F: Task-Aware Context Retrieval
#[test]
fn test_scenario_f_task_aware_retrieval() {
    let temp = tempfile::tempdir().unwrap();
    let ws = temp.path();

    fs::create_dir_all(ws.join("src/auth")).unwrap();
    fs::create_dir_all(ws.join("src/billing")).unwrap();
    fs::create_dir_all(ws.join("tests")).unwrap();

    let auth_code = r#"pub struct AuthManager;
impl AuthManager {
    pub fn verify_jwt(token: &str) -> bool {
        !token.is_empty()
    }
    pub fn revoke_session(session_id: &str) {}
}
"#;
    fs::write(ws.join("src/auth/mod.rs"), auth_code).unwrap();

    let billing_code = r#"pub struct PaymentProcessor;
impl PaymentProcessor {
    pub fn charge_credit_card(amount_cents: u64) -> Result<(), String> {
        if amount_cents == 0 {
            return Err("Zero amount invalid".to_string());
        }
        Ok(())
    }
}
"#;
    fs::write(ws.join("src/billing/mod.rs"), billing_code).unwrap();

    let test_code = r#"use auth::AuthManager;
#[test]
fn test_jwt_verification() {
    assert!(AuthManager::verify_jwt("valid_token"));
}
"#;
    fs::write(ws.join("tests/test_auth.rs"), test_code).unwrap();

    let index = AstCodebaseIndex::build_with_options(ws, TraversalOptions::default());

    // Task 1: Feature modification (JWT Auth)
    let query_feature = RetrievalQuery {
        task_description: Some("Add expiry validation to JWT token verification".to_string()),
        symbol_name: Some("verify_jwt".to_string()),
        token_budget: Some(2000),
        ..Default::default()
    };
    let packet_feature = index.retrieve_context(&query_feature);
    assert!(packet_feature.total_estimated_tokens <= 2000);
    assert!(
        packet_feature
            .symbols
            .iter()
            .any(|s| s.symbol.name == "verify_jwt")
    );
    assert!(!packet_feature.formatted_markdown.is_empty());

    // Task 2: Bug report with stack trace
    let query_bug = RetrievalQuery {
        error_message: Some(
            "Error: Zero amount invalid in PaymentProcessor charge_credit_card".to_string(),
        ),
        stack_trace: Some("at billing::PaymentProcessor::charge_credit_card(line 5)".to_string()),
        token_budget: Some(1500),
        ..Default::default()
    };
    let packet_bug = index.retrieve_context(&query_bug);
    assert!(packet_bug.total_estimated_tokens <= 1500);
    assert!(
        packet_bug
            .symbols
            .iter()
            .any(|s| s.symbol.name == "charge_credit_card" || s.symbol.name == "PaymentProcessor")
    );

    // Task 3: Cross-module refactoring
    let query_refactor = RetrievalQuery {
        task_description: Some("Refactor AuthManager session revocation".to_string()),
        symbol_name: Some("revoke_session".to_string()),
        token_budget: Some(1000),
        ..Default::default()
    };
    let packet_refactor = index.retrieve_context(&query_refactor);
    assert!(packet_refactor.total_estimated_tokens <= 1000);
    assert!(
        packet_refactor
            .symbols
            .iter()
            .any(|s| s.symbol.name == "revoke_session")
    );
}
