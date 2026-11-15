//! Core AST symbol representation, relationships, language identities, and source ranges.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AstSymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Class,
    Interface,
    Trait,
    Impl,
    TypeAlias,
    Constant,
    Module,
    Property,
    Macro,
    Variable,
    Field,
    Constructor,
}

impl AstSymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::TypeAlias => "type",
            Self::Constant => "const",
            Self::Module => "module",
            Self::Property => "property",
            Self::Macro => "macro",
            Self::Variable => "variable",
            Self::Field => "field",
            Self::Constructor => "constructor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LanguageId {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    C,
    Cpp,
    Java,
    CSharp,
    #[default]
    Unknown,
}

impl LanguageId {
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "rs" => Self::Rust,
            "ts" | "tsx" => Self::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "py" | "pyi" => Self::Python,
            "go" => Self::Go,
            "c" | "h" => Self::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Self::Cpp,
            "java" => Self::Java,
            "cs" => Self::CSharp,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SymbolVisibility {
    #[default]
    Public,
    Private,
    Internal,
    Protected,
}

impl SymbolVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Internal => "internal",
            Self::Protected => "protected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    Defines,
    Imports,
    Exports,
    ReExports,
    Calls,
    References,
    Implements,
    Extends,
    UsesType,
    DependsOn,
    Tests,
    Contains,
    PartOfPackage,
    PartOfModule,
}

impl RelationshipKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Defines => "defines",
            Self::Imports => "imports",
            Self::Exports => "exports",
            Self::ReExports => "re_exports",
            Self::Calls => "calls",
            Self::References => "references",
            Self::Implements => "implements",
            Self::Extends => "extends",
            Self::UsesType => "uses_type",
            Self::DependsOn => "depends_on",
            Self::Tests => "tests",
            Self::Contains => "contains",
            Self::PartOfPackage => "part_of_package",
            Self::PartOfModule => "part_of_module",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Exact,
    AstDerived,
    CompilerAssisted,
    Heuristic,
    #[default]
    Unresolved,
}

impl ResolutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::AstDerived => "ast_derived",
            Self::CompilerAssisted => "compiler_assisted",
            Self::Heuristic => "heuristic",
            Self::Unresolved => "unresolved",
        }
    }

    pub fn is_confident(&self) -> bool {
        matches!(
            self,
            Self::Exact | Self::AstDerived | Self::CompilerAssisted
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl SourceRange {
    pub fn new(start_line: usize, end_line: usize, start_col: usize, end_col: usize) -> Self {
        Self {
            start_line,
            end_line,
            start_col,
            end_col,
        }
    }

    pub fn lines(&self) -> usize {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            1
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstSymbol {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub qualified_name: String,
    pub kind: AstSymbolKind,
    #[serde(default)]
    pub language: LanguageId,
    pub file_path: PathBuf,
    pub range: SourceRange,
    pub signature: String,
    pub doc_comment: Option<String>,
    pub parent_scope: Option<String>,
    #[serde(default)]
    pub package_id: Option<String>,
    #[serde(default)]
    pub module_path: Option<String>,
    #[serde(default)]
    pub visibility: SymbolVisibility,
    pub is_public: bool,
    pub children: Vec<AstSymbol>,
    #[serde(default)]
    pub content_hash: Option<String>,
}

impl AstSymbol {
    pub fn new(
        name: impl Into<String>,
        kind: AstSymbolKind,
        file_path: PathBuf,
        range: SourceRange,
        signature: impl Into<String>,
    ) -> Self {
        let name_str = name.into();
        let sig_str = signature.into();
        let lang = LanguageId::from_path(&file_path);
        let id = Self::generate_id(&file_path, None, &name_str, kind, range.start_line);
        Self {
            id,
            qualified_name: name_str.clone(),
            name: name_str,
            kind,
            language: lang,
            file_path,
            range,
            signature: sig_str,
            doc_comment: None,
            parent_scope: None,
            package_id: None,
            module_path: None,
            visibility: SymbolVisibility::Public,
            is_public: true,
            children: Vec::new(),
            content_hash: None,
        }
    }

    pub fn with_parent_scope(mut self, parent: impl Into<String>) -> Self {
        let p = parent.into();
        self.qualified_name = format!("{}::{}", p, self.name);
        self.parent_scope = Some(p.clone());
        self.id = Self::generate_id(
            &self.file_path,
            Some(&p),
            &self.name,
            self.kind,
            self.range.start_line,
        );
        self
    }

    pub fn with_package_and_module(
        mut self,
        package_id: Option<String>,
        module_path: Option<String>,
    ) -> Self {
        self.package_id = package_id;
        self.module_path = module_path;
        self
    }

    pub fn with_visibility(mut self, visibility: SymbolVisibility) -> Self {
        self.visibility = visibility;
        self.is_public = matches!(visibility, SymbolVisibility::Public);
        self
    }

    pub fn generate_id(
        file_path: &Path,
        parent_scope: Option<&str>,
        name: &str,
        kind: AstSymbolKind,
        line: usize,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(file_path.to_string_lossy().as_bytes());
        hasher.update(b":");
        if let Some(scope) = parent_scope {
            hasher.update(scope.as_bytes());
            hasher.update(b":");
        }
        hasher.update(name.as_bytes());
        hasher.update(b":");
        hasher.update(kind.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(line.to_string().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let short_hash = &hash[..16];

        let scope_prefix = parent_scope.map(|s| format!("{}::", s)).unwrap_or_default();
        format!("sym::{}{}_{}", scope_prefix, name, short_hash)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolRelationship {
    pub id: String,
    pub source_id: String,
    pub target_id: Option<String>,
    pub source_symbol: String,
    pub target_symbol: String,
    pub kind: RelationshipKind,
    pub file_path: PathBuf,
    pub line: usize,
    pub range: Option<SourceRange>,
    pub status: ResolutionStatus,
    pub confidence: f64,
    pub provenance: String,
    pub diagnostic: Option<String>,
}

impl SymbolRelationship {
    pub fn new(
        source_id: impl Into<String>,
        source_symbol: impl Into<String>,
        target_symbol: impl Into<String>,
        kind: RelationshipKind,
        file_path: PathBuf,
        line: usize,
        status: ResolutionStatus,
    ) -> Self {
        let src_id = source_id.into();
        let src_sym = source_symbol.into();
        let tgt_sym = target_symbol.into();
        let id = format!(
            "rel::{}_{}_{}_{}_{}",
            src_sym,
            kind.as_str(),
            tgt_sym,
            file_path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            line
        );
        let confidence = match status {
            ResolutionStatus::Exact => 1.0,
            ResolutionStatus::AstDerived => 0.85,
            ResolutionStatus::CompilerAssisted => 0.95,
            ResolutionStatus::Heuristic => 0.45,
            ResolutionStatus::Unresolved => 0.1,
        };

        Self {
            id,
            source_id: src_id,
            target_id: None,
            source_symbol: src_sym,
            target_symbol: tgt_sym,
            kind,
            file_path,
            line,
            range: None,
            status,
            confidence,
            provenance: "ast_parser".to_string(),
            diagnostic: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticChunk {
    pub symbol_name: String,
    pub kind: AstSymbolKind,
    pub range: SourceRange,
    pub code: String,
    pub token_estimate: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CallReference {
    pub caller_name: String,
    pub callee_name: String,
    pub file_path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportItem {
    pub source_file: PathBuf,
    pub imported_symbol: String,
    pub alias: Option<String>,
    pub from_module: String,
    pub line: usize,
    pub is_glob: bool,
    pub is_type_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportItem {
    pub source_file: PathBuf,
    pub symbol_name: String,
    pub exported_as: String,
    pub line: usize,
    pub is_default: bool,
    pub reexported_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CallSite {
    pub caller_name: String,
    pub callee_name: String,
    pub callee_scope: Option<String>,
    pub file_path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub is_method_call: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserDiagnostic {
    pub file_path: PathBuf,
    pub language: LanguageId,
    pub parser_id: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub partial_usable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    Complete,
    Partial,
    Interrupted,
    #[default]
    Failed,
}

impl IndexStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LanguageCapabilities {
    pub syntax_parsing: bool,
    pub definition_extraction: bool,
    pub import_export_extraction: bool,
    pub reference_extraction: bool,
    pub call_resolution: bool,
    pub type_resolution: bool,
    pub test_discovery: bool,
}
