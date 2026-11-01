//! `m31a-ast`: Codebase AST, syntax tree intelligence, and semantic chunking engine.

pub mod architecture;
pub mod chunker;
pub mod clones;
pub mod data_flow;
pub mod doc_sync;
pub mod drift;
pub mod impact;
pub mod index;
pub mod parser;
pub mod semver;
pub mod syntax_validator;
pub mod types;

pub use architecture::{
    ArchitecturalComplexityAnalyzer, ArchitectureAuditReport, ComplexityRisk, ComplexitySmell,
    ModuleCoupling,
};
pub use chunker::AstChunker;
pub use clones::{CloneAuditReport, CloneGroup, CloneInstance, CodeCloneDetector, DeadSymbol};
pub use data_flow::{DataFlowAnalyzer, DataFlowReport, FlowSinkKind, FlowSourceKind, VariableFlow};
pub use doc_sync::{
    DocDriftItem, DocDriftKind, DocSyncAuditor, DocSyncReport, MissingDocItem,
    ProseDocSynchronizer, ProseSyncReport, StaleDocItem,
};
pub use drift::{CodebaseDriftDetector, DriftCategory, DriftCounts, DriftElement, DriftResult};
pub use impact::{
    BlastRadiusReport, DependentConsumer, ImpactRiskLevel, SemanticImpactAnalyzer, SymbolImpact,
};
pub use index::{AstCodebaseIndex, CallGraphNode};
pub use parser::AstParser;
pub use semver::{
    BreakingChange, DependencyAuditReport, DependencyIssue, DependencySeverity,
    ManifestDependencyAuditor, SemverAuditReport, SemverDriftDetector, SemverImpact,
};
pub use syntax_validator::{SyntaxValidationError, SyntaxValidator};
pub use types::{AstSymbol, AstSymbolKind, CallReference, SemanticChunk, SourceRange};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_rust_symbols_and_ranges() {
        let code = r#"
/// A high-performance agent executor.
pub struct AgentExecutor {
    pub max_turns: usize,
}

impl AgentExecutor {
    /// Runs a single turn.
    pub async fn run_turn(&self, session_id: &str) -> Result<()> {
        println!("Running turn {}", session_id);
        Ok(())
    }

    fn helper(&self) -> bool {
        true
    }
}

pub enum ExecutionState {
    Active,
    Completed,
}
"#;
        let syms = AstParser::parse_file(&PathBuf::from("executor.rs"), code);
        assert_eq!(syms.len(), 4);

        // Struct
        assert_eq!(syms[0].name, "AgentExecutor");
        assert_eq!(syms[0].kind, AstSymbolKind::Struct);
        assert_eq!(syms[0].range.start_line, 3);
        assert_eq!(syms[0].range.end_line, 5);
        assert!(
            syms[0]
                .doc_comment
                .as_ref()
                .unwrap()
                .contains("high-performance")
        );

        // Method run_turn
        assert_eq!(syms[1].name, "run_turn");
        assert_eq!(syms[1].kind, AstSymbolKind::Method);
        assert_eq!(syms[1].parent_scope.as_deref(), Some("AgentExecutor"));
        assert_eq!(syms[1].range.start_line, 9);
        assert_eq!(syms[1].range.end_line, 12);

        // Helper method
        assert_eq!(syms[2].name, "helper");
        assert_eq!(syms[2].kind, AstSymbolKind::Method);
        assert_eq!(syms[2].parent_scope.as_deref(), Some("AgentExecutor"));

        // Enum
        assert_eq!(syms[3].name, "ExecutionState");
        assert_eq!(syms[3].kind, AstSymbolKind::Enum);
    }

    #[test]
    fn test_parse_typescript_symbols() {
        let code = r#"
export interface UserConfig {
    apiKey: string;
}

export class AgentService {
    constructor(private config: UserConfig) {}

    public async executeTask(prompt: string): Promise<string> {
        return "done";
    }
}

export const createService = (config: UserConfig): AgentService => {
    return new AgentService(config);
};
"#;
        let syms = AstParser::parse_file(&PathBuf::from("service.ts"), code);
        assert_eq!(syms.len(), 5);

        assert_eq!(syms[0].name, "UserConfig");
        assert_eq!(syms[0].kind, AstSymbolKind::Interface);

        assert_eq!(syms[1].name, "AgentService");
        assert_eq!(syms[1].kind, AstSymbolKind::Class);

        assert_eq!(syms[2].name, "constructor");
        assert_eq!(syms[2].kind, AstSymbolKind::Method);
        assert_eq!(syms[2].parent_scope.as_deref(), Some("AgentService"));

        assert_eq!(syms[3].name, "executeTask");
        assert_eq!(syms[3].kind, AstSymbolKind::Method);
        assert_eq!(syms[3].parent_scope.as_deref(), Some("AgentService"));

        assert_eq!(syms[4].name, "createService");
        assert_eq!(syms[4].kind, AstSymbolKind::Function);
    }

    #[test]
    fn test_parse_python_symbols() {
        let code = r#"
class DatabaseManager:
    def __init__(self, dsn: str):
        self.dsn = dsn

    async def connect(self):
        print("connected")

def standalone_helper(x: int) -> int:
    return x * 2
"#;
        let syms = AstParser::parse_file(&PathBuf::from("db.py"), code);
        assert_eq!(syms.len(), 4);

        assert_eq!(syms[0].name, "DatabaseManager");
        assert_eq!(syms[0].kind, AstSymbolKind::Class);

        assert_eq!(syms[1].name, "__init__");
        assert_eq!(syms[1].kind, AstSymbolKind::Method);
        assert_eq!(syms[1].parent_scope.as_deref(), Some("DatabaseManager"));

        assert_eq!(syms[2].name, "connect");
        assert_eq!(syms[2].kind, AstSymbolKind::Method);
        assert_eq!(syms[2].parent_scope.as_deref(), Some("DatabaseManager"));

        assert_eq!(syms[3].name, "standalone_helper");
        assert_eq!(syms[3].kind, AstSymbolKind::Function);
    }

    #[test]
    fn test_parse_go_symbols() {
        let code = r#"
package server

type Router struct {
    routes []string
}

func (r *Router) Register(path string) {
    r.routes = append(r.routes, path)
}

func NewRouter() *Router {
    return &Router{}
}
"#;
        let syms = AstParser::parse_file(&PathBuf::from("server.go"), code);
        assert_eq!(syms.len(), 3);

        assert_eq!(syms[0].name, "Router");
        assert_eq!(syms[0].kind, AstSymbolKind::Struct);

        assert_eq!(syms[1].name, "Register");
        assert_eq!(syms[1].kind, AstSymbolKind::Method);

        assert_eq!(syms[2].name, "NewRouter");
        assert_eq!(syms[2].kind, AstSymbolKind::Function);
    }

    #[test]
    fn test_chunker_and_body_extraction() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("math.rs");
        let content = r#"// Header comment
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
"#;
        std::fs::write(&file_path, content).unwrap();

        let syms = AstParser::parse_file(&file_path, content);
        assert_eq!(syms.len(), 2);

        let chunks = AstChunker::chunk_file(&file_path, content, &syms);
        assert!(!chunks.is_empty());

        let index = AstCodebaseIndex::build(temp.path(), 10);
        let body = index
            .extract_symbol_body(&file_path, "add")
            .unwrap()
            .unwrap();
        assert!(body.contains("pub fn add"));
        assert!(body.contains("a + b"));
    }

    #[test]
    fn test_parse_robust_to_braces_in_strings_and_comments() {
        let rs_code = r#"
pub fn format_message(name: &str) -> String {
    // A comment with unmatched { and } braces
    /* Block comment with { nested { braces } */
    let template = "Hello {name}, your code is {100%}";
    format!("Result: {}", template)
}

pub fn second_function() -> bool {
    true
}
"#;
        let syms = AstParser::parse_file(&PathBuf::from("test.rs"), rs_code);
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].name, "format_message");
        assert_eq!(syms[1].name, "second_function");

        let ts_code = r#"
export function renderTemplate(param: string): string {
    // Comment with { and }
    const str = "String with {braces} and escaped \"{\" quotes";
    const template = `Template with ${param} and {extra}`;
    return str + template;
}

export function followingFunction(): number {
    return 42;
}
"#;
        let ts_syms = AstParser::parse_file(&PathBuf::from("template.ts"), ts_code);
        assert_eq!(ts_syms.len(), 2);
        assert_eq!(ts_syms[0].name, "renderTemplate");
        assert_eq!(ts_syms[1].name, "followingFunction");
    }
}
