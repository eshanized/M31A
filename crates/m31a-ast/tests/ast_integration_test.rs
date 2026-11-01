//! Integration tests for AST parsing, chunking, and drift detection.
//!
//! Verifies syntax tree extraction, cross-reference indexing, and schema divergence analysis.

use m31a_ast::{AstCodebaseIndex, AstParser, AstSymbolKind};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_c_like_language_ast_parsing() {
    let tmp = tempdir().unwrap();
    let cpp_file = tmp.path().join("service.cpp");
    let cpp_code = r#"
class DatabaseConnection {
public:
    void executeQuery(const std::string& query) {
        // execute
    }
};

enum class ConnectionStatus {
    Connected,
    Disconnected
};
"#;
    let symbols = AstParser::parse_file(&cpp_file, cpp_code);
    assert_eq!(symbols.len(), 3);
    assert_eq!(symbols[0].name, "DatabaseConnection");
    assert_eq!(symbols[0].kind, AstSymbolKind::Class);
    assert_eq!(symbols[1].name, "executeQuery");
    assert_eq!(symbols[1].kind, AstSymbolKind::Method);
    assert_eq!(
        symbols[1].parent_scope.as_deref(),
        Some("DatabaseConnection")
    );
    assert_eq!(symbols[2].name, "ConnectionStatus");
    assert_eq!(symbols[2].kind, AstSymbolKind::Enum);
}

#[test]
fn test_ast_codebase_index_build_and_find_definition() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let rust_file = root.join("auth.rs");
    let rust_code = r#"
pub struct UserSession {
    pub token: String,
}

impl UserSession {
    pub fn validate(&self) -> bool {
        !self.token.is_empty()
    }
}
"#;
    fs::write(&rust_file, rust_code).unwrap();

    let ts_file = root.join("api.ts");
    let ts_code = r#"
export class ApiClient {
    async request(url: string): Promise<any> {
        return fetch(url);
    }
}
"#;
    fs::write(&ts_file, ts_code).unwrap();

    let index = AstCodebaseIndex::build(root, 100);
    assert_eq!(index.files_indexed(), 2);
    assert!(index.total_symbols() >= 3);

    // Find UserSession
    let defs = index.find_definition("UserSession");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "UserSession");
    assert_eq!(defs[0].kind, AstSymbolKind::Struct);

    // Find validate
    let val_defs = index.find_definition("validate");
    assert_eq!(val_defs.len(), 1);
    assert_eq!(val_defs[0].name, "validate");
    assert_eq!(val_defs[0].kind, AstSymbolKind::Method);

    // Find ApiClient
    let client_defs = index.find_definition("ApiClient");
    assert_eq!(client_defs.len(), 1);
    assert_eq!(client_defs[0].name, "ApiClient");
}

#[test]
fn test_extract_symbol_body_exact() {
    let tmp = tempdir().unwrap();
    let file = tmp.path().join("model.rs");
    let code = r#"// Header comment
pub struct Config {
    pub host: String,
    pub port: u16,
}

pub fn run() {}
"#;
    fs::write(&file, code).unwrap();

    let index = AstCodebaseIndex::build(tmp.path(), 10);
    let body = index.extract_symbol_body(&file, "Config").unwrap();
    assert!(body.is_some());
    let body_str = body.unwrap();
    assert!(body_str.starts_with("pub struct Config"));
    assert!(body_str.contains("pub host: String"));
}

#[test]
fn test_semantic_chunking_file() {
    let tmp = tempdir().unwrap();
    let file = tmp.path().join("server.rs");
    let code = r#"
pub struct Server {
    port: u16,
}

impl Server {
    pub fn listen(&self) {
        println!("listening");
    }
}
"#;
    fs::write(&file, code).unwrap();

    let index = AstCodebaseIndex::build(tmp.path(), 10);
    let chunks = index.chunk_file(&file).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().any(|c| c.symbol_name == "Server"));
}

#[test]
fn test_find_callers_cross_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // File A defines
    let file_a = root.join("crypto.rs");
    fs::write(
        &file_a,
        "pub fn compute_hash(data: &str) -> String { data.to_string() }\n",
    )
    .unwrap();

    // File B calls
    let file_b = root.join("auth.rs");
    let file_b_code = r#"
pub fn authenticate(password: &str) {
    let hash = compute_hash(password);
}
"#;
    fs::write(&file_b, file_b_code).unwrap();

    let callers = AstCodebaseIndex::find_callers(root, "compute_hash", 10);
    assert!(!callers.is_empty());
    assert!(callers.iter().any(|c| c.file_path == file_b));
}

#[test]
fn test_parse_python_and_go_symbols() {
    let py_code = r#"
class PaymentProcessor:
    def process(self, amount):
        return True

def standalone_helper():
    pass
"#;
    let py_syms = AstParser::parse_file(&std::path::PathBuf::from("pay.py"), py_code);
    assert!(
        py_syms
            .iter()
            .any(|s| s.name == "PaymentProcessor" && s.kind == AstSymbolKind::Class)
    );
    assert!(
        py_syms
            .iter()
            .any(|s| s.name == "process" && s.kind == AstSymbolKind::Method)
    );
    assert!(
        py_syms
            .iter()
            .any(|s| s.name == "standalone_helper" && s.kind == AstSymbolKind::Function)
    );

    let go_code = r#"
package main

type Service struct {
    Name string
}

func StartService() {
}
"#;
    let go_syms = AstParser::parse_file(&std::path::PathBuf::from("main.go"), go_code);
    assert!(
        go_syms
            .iter()
            .any(|s| s.name == "Service" && s.kind == AstSymbolKind::Struct)
    );
    assert!(
        go_syms
            .iter()
            .any(|s| s.name == "StartService" && s.kind == AstSymbolKind::Function)
    );
}
