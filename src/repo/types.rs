use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Level of empirical verification supporting a repository fact (REP-04, D-09).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FactClass {
    /// Formally verified by AST parser (e.g. syn 2.0) or build manifest (VerifiedRepositoryFact).
    VerifiedFact,
    /// Inferred via deterministic regex or token scanner (InferredFact).
    InferredFact,
    /// Proposed by LLM reasoning; requires empirical verification before authoritative use.
    Hypothesis,
    /// Assumed based on conventions, heuristics, or defaults.
    Assumption,
    /// Unknown or unclassified provenance.
    Unknown,
}

impl FactClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VerifiedFact => "verified_fact",
            Self::InferredFact => "inferred_fact",
            Self::Hypothesis => "hypothesis",
            Self::Assumption => "assumption",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "verified_fact" | "verified" => Some(Self::VerifiedFact),
            "inferred_fact" | "inferred" => Some(Self::InferredFact),
            "hypothesis" => Some(Self::Hypothesis),
            "assumption" => Some(Self::Assumption),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Whether this fact has been formally verified by a deterministic compiler/parser.
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::VerifiedFact)
    }
}

impl fmt::Display for FactClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Category of code or repository symbol.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Class,
    Interface,
    TypeAlias,
    Variable,
    Constant,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Module => "module",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::TypeAlias => "type_alias",
            Self::Variable => "variable",
            Self::Constant => "constant",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "function" | "fn" => Some(Self::Function),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "trait" => Some(Self::Trait),
            "impl" => Some(Self::Impl),
            "module" | "mod" => Some(Self::Module),
            "class" => Some(Self::Class),
            "interface" => Some(Self::Interface),
            "type_alias" | "type" => Some(Self::TypeAlias),
            "variable" | "var" => Some(Self::Variable),
            "constant" | "const" => Some(Self::Constant),
            _ => None,
        }
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Normalized canonical symbol discovered in the repository (REP-01, REP-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositorySymbol {
    /// Stable identifier, e.g. "path/to/file.rs::fn::my_function"
    pub id: String,
    /// Unqualified symbol name, e.g. "my_function"
    pub name: String,
    /// Fully qualified path/name, e.g. "crate::module::my_function"
    pub qualified_name: String,
    /// Kind of symbol
    pub kind: SymbolKind,
    /// Relative repository file path
    pub file_path: String,
    /// 1-indexed start line
    pub start_line: usize,
    /// 1-indexed end line (inclusive)
    pub end_line: usize,
    /// Cleaned declaration/signature
    pub signature: String,
    /// Fact classification (VerifiedFact, InferredFact, etc.)
    pub fact_class: FactClass,
    /// Optional documentation comment
    pub doc_comment: Option<String>,
    /// Whether this symbol represents a test function, suite, or test helper
    #[serde(default)]
    pub is_test: bool,
    /// Detailed provenance tracking the analysis method and source coordinates
    #[serde(default)]
    pub provenance: Option<EvidenceProvenance>,
}

impl RepositorySymbol {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        qualified_name: impl Into<String>,
        kind: SymbolKind,
        file_path: impl Into<String>,
        start_line: usize,
        end_line: usize,
        signature: impl Into<String>,
        fact_class: FactClass,
        doc_comment: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            qualified_name: qualified_name.into(),
            kind,
            file_path: file_path.into(),
            start_line,
            end_line: end_line.max(start_line),
            signature: signature.into(),
            fact_class,
            doc_comment,
            is_test: false,
            provenance: None,
        }
    }

    /// Mark this symbol as a test or non-test symbol.
    pub fn with_test(mut self, is_test: bool) -> Self {
        self.is_test = is_test;
        self
    }

    /// Attach empirical evidence provenance.
    pub fn with_provenance(mut self, provenance: EvidenceProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }
}

/// Multi-layer file classification (REP-01, Level 1 File System).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FileClassification {
    /// Authoritative production source code
    Source,
    /// Test suite or test fixture file
    Test,
    /// Project or runtime configuration file
    Configuration,
    /// Build manifest, build script, or target specification
    BuildTarget,
    /// Project documentation, specification, or manual
    Documentation,
    /// Machine-generated code or generated artifacts
    Generated,
    /// Static media, fonts, or non-code assets
    Asset,
    /// Compiled binary, executable, or shared object
    Binary,
    /// Ignored or excluded path (e.g. build output, caches)
    Ignored,
}

impl FileClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Test => "test",
            Self::Configuration => "configuration",
            Self::BuildTarget => "build_target",
            Self::Documentation => "documentation",
            Self::Generated => "generated",
            Self::Asset => "asset",
            Self::Binary => "binary",
            Self::Ignored => "ignored",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "source" | "src" => Some(Self::Source),
            "test" | "tests" => Some(Self::Test),
            "configuration" | "config" => Some(Self::Configuration),
            "build_target" | "build" => Some(Self::BuildTarget),
            "documentation" | "doc" | "docs" => Some(Self::Documentation),
            "generated" | "gen" => Some(Self::Generated),
            "asset" | "assets" => Some(Self::Asset),
            "binary" | "bin" => Some(Self::Binary),
            "ignored" => Some(Self::Ignored),
            _ => None,
        }
    }

    pub fn is_source_or_test(&self) -> bool {
        matches!(self, Self::Source | Self::Test)
    }
}

impl fmt::Display for FileClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Category of executable or library entry point.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EntryPointKind {
    /// Binary executable entry (e.g. `fn main()` or cargo `[[bin]]`)
    BinaryMain,
    /// Root crate or package library export (e.g. `src/lib.rs`)
    LibraryRoot,
    /// Integration test harness entry (e.g. `tests/*.rs`)
    TestEntry,
    /// CLI subcommand dispatcher
    CliCommand,
    /// Executable script
    Script,
}

impl EntryPointKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BinaryMain => "binary_main",
            Self::LibraryRoot => "library_root",
            Self::TestEntry => "test_entry",
            Self::CliCommand => "cli_command",
            Self::Script => "script",
        }
    }
}

impl fmt::Display for EntryPointKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Verified or inferred entry point into repository logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntryPoint {
    pub id: String,
    pub kind: EntryPointKind,
    pub file_path: String,
    pub symbol_name: Option<String>,
    pub line_number: usize,
    pub description: String,
}

impl EntryPoint {
    pub fn new(
        id: impl Into<String>,
        kind: EntryPointKind,
        file_path: impl Into<String>,
        symbol_name: Option<String>,
        line_number: usize,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            file_path: file_path.into(),
            symbol_name,
            line_number,
            description: description.into(),
        }
    }
}

/// Method used to discover and analyze a repository entity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisMethod {
    /// Formally parsed syntax tree via language compiler/AST
    AstVisitor,
    /// Deterministic regular expression / token scanner
    RegexScan,
    /// Package or build system manifest inspection
    ManifestInspection,
    /// Standard filesystem path or directory convention
    PathConvention,
    /// Heuristic inference from naming or structure
    HeuristicInference,
}

impl AnalysisMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AstVisitor => "ast_visitor",
            Self::RegexScan => "regex_scan",
            Self::ManifestInspection => "manifest_inspection",
            Self::PathConvention => "path_convention",
            Self::HeuristicInference => "heuristic_inference",
        }
    }
}

impl fmt::Display for AnalysisMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Empirical provenance and traceability evidence for repository intelligence findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceProvenance {
    pub method: AnalysisMethod,
    pub source_file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub fact_class: FactClass,
}

impl EvidenceProvenance {
    pub fn new(
        method: AnalysisMethod,
        source_file: impl Into<String>,
        start_line: usize,
        end_line: usize,
        fact_class: FactClass,
    ) -> Self {
        Self {
            method,
            source_file: source_file.into(),
            start_line,
            end_line: end_line.max(start_line),
            fact_class,
        }
    }

    pub fn verified_ast(source_file: impl Into<String>, start: usize, end: usize) -> Self {
        Self::new(
            AnalysisMethod::AstVisitor,
            source_file,
            start,
            end,
            FactClass::VerifiedFact,
        )
    }

    pub fn inferred_regex(source_file: impl Into<String>, start: usize, end: usize) -> Self {
        Self::new(
            AnalysisMethod::RegexScan,
            source_file,
            start,
            end,
            FactClass::InferredFact,
        )
    }

    pub fn manifest(source_file: impl Into<String>, line: usize) -> Self {
        Self::new(
            AnalysisMethod::ManifestInspection,
            source_file,
            line,
            line,
            FactClass::VerifiedFact,
        )
    }
}

/// Staleness and synchronization state of the repository index.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    /// In-memory graph is strictly synchronized with disk hashes
    Current,
    /// Some files have changed on disk but majority remain valid
    PartiallyStale,
    /// Discrepancy threshold exceeded; complete re-indexing required
    Stale,
    /// Repository has not yet been scanned
    Empty,
}

impl IndexStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::PartiallyStale => "partially_stale",
            Self::Stale => "stale",
            Self::Empty => "empty",
        }
    }

    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Current | Self::PartiallyStale)
    }
}

impl fmt::Display for IndexStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Layered architectural subsystem classification (L0-L9 architecture mapping).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SubsystemKind {
    /// L0: Kernel, error taxonomy, IDs, core invariants
    Kernel,
    /// L1: Security, policy engine, sandboxing
    Security,
    /// L2: Capability providers, tool runtime, process execution
    Capability,
    /// L3: Intelligence, prompt compiler, context, model router
    Intelligence,
    /// L4: Agent runtime, roles, state machines, worker dispatcher
    Agent,
    /// L5: Planning, DAG decomposition, task dependencies
    Planning,
    /// L6: Task execution, job supervisor, artifact storage, persistence
    Execution,
    /// L7: Verification, test runners, recovery, diagnostics
    Verification,
    /// L8: Autonomy controller, mission engine, loop reconciliation
    Autonomy,
    /// L9: CLI commands, TUI presentation, external integrations
    UserInterface,
    /// Unclassified or external component
    Other,
}

impl SubsystemKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Security => "security",
            Self::Capability => "capability",
            Self::Intelligence => "intelligence",
            Self::Agent => "agent",
            Self::Planning => "planning",
            Self::Execution => "execution",
            Self::Verification => "verification",
            Self::Autonomy => "autonomy",
            Self::UserInterface => "user_interface",
            Self::Other => "other",
        }
    }

    pub fn layer_number(&self) -> Option<usize> {
        match self {
            Self::Kernel => Some(0),
            Self::Security => Some(1),
            Self::Capability => Some(2),
            Self::Intelligence => Some(3),
            Self::Agent => Some(4),
            Self::Planning => Some(5),
            Self::Execution => Some(6),
            Self::Verification => Some(7),
            Self::Autonomy => Some(8),
            Self::UserInterface => Some(9),
            Self::Other => None,
        }
    }

    /// Detect subsystem from relative path conventions.
    pub fn detect_from_path(path: &str) -> Self {
        let normalized = path.replace('\\', "/");
        if normalized.starts_with("src/kernel") || normalized.starts_with("src/ids") {
            Self::Kernel
        } else if normalized.starts_with("src/policy") || normalized.starts_with("src/sandbox") {
            Self::Security
        } else if normalized.starts_with("src/capability")
            || normalized.starts_with("src/tools")
            || normalized.starts_with("src/process")
        {
            Self::Capability
        } else if normalized.starts_with("src/context")
            || normalized.starts_with("src/model")
            || normalized.starts_with("src/prompt")
            || normalized.starts_with("src/repo")
        {
            Self::Intelligence
        } else if normalized.starts_with("src/agent") || normalized.starts_with("src/state_machine")
        {
            Self::Agent
        } else if normalized.starts_with("src/planning") || normalized.starts_with("src/dag") {
            Self::Planning
        } else if normalized.starts_with("src/persistence")
            || normalized.starts_with("src/checkpoint")
            || normalized.starts_with("src/state")
            || normalized.starts_with("src/workflow")
        {
            Self::Execution
        } else if normalized.starts_with("src/verification")
            || normalized.starts_with("src/recovery")
        {
            Self::Verification
        } else if normalized.starts_with("src/controller")
            || normalized.starts_with("src/scheduler")
        {
            Self::Autonomy
        } else if normalized.starts_with("src/cli")
            || normalized.starts_with("src/tui")
            || normalized.starts_with("src/interaction")
        {
            Self::UserInterface
        } else {
            Self::Other
        }
    }
}

impl fmt::Display for SubsystemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Granularity or structure of an extracted source slice.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceSliceKind {
    /// Complete contents of the target file.
    FullFile,
    /// Exact body and span of the discovered symbol.
    SymbolBody,
    /// Function/struct/trait signature only without internal implementation body.
    SignatureOnly,
    /// Contiguous line range with surrounding context.
    ContiguousSlice,
}

impl SourceSliceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FullFile => "full_file",
            Self::SymbolBody => "symbol_body",
            Self::SignatureOnly => "signature_only",
            Self::ContiguousSlice => "contiguous_slice",
        }
    }
}

impl fmt::Display for SourceSliceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Extracted source slice with span coordinates, hash, and staleness indicator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceSlice {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub slice_kind: SourceSliceKind,
    pub content_hash: String,
    pub is_stale: bool,
}

impl SourceSlice {
    pub fn new(
        file_path: impl Into<String>,
        start_line: usize,
        end_line: usize,
        content: impl Into<String>,
        slice_kind: SourceSliceKind,
        content_hash: impl Into<String>,
        is_stale: bool,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            start_line,
            end_line,
            content: content.into(),
            slice_kind,
            content_hash: content_hash.into(),
            is_stale,
        }
    }
}

/// Synchronization and drift status of a specific file against the index.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FileStaleness {
    /// Disk content hash exactly matches indexed repository graph hash.
    Current,
    /// File on disk has been modified and differs from indexed graph hash.
    Modified,
    /// File was recorded in repository graph but is missing on disk.
    MissingFromDisk,
    /// File exists on disk but is absent from repository graph.
    MissingFromGraph,
    /// Staleness cannot be evaluated (e.g. no workspace root configured).
    Unknown,
}

impl FileStaleness {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Modified => "modified",
            Self::MissingFromDisk => "missing_from_disk",
            Self::MissingFromGraph => "missing_from_graph",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_current(&self) -> bool {
        matches!(self, Self::Current)
    }
}

impl fmt::Display for FileStaleness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fact_class_serde_and_display() {
        let fact = FactClass::VerifiedFact;
        assert_eq!(fact.as_str(), "verified_fact");
        assert_eq!(fact.to_string(), "verified_fact");
        assert!(fact.is_verified());
        assert_eq!(
            FactClass::from_str_name("verified_fact"),
            Some(FactClass::VerifiedFact)
        );
        assert_eq!(
            FactClass::from_str_name("inferred"),
            Some(FactClass::InferredFact)
        );

        let json = serde_json::to_string(&fact).unwrap();
        assert_eq!(json, "\"verified_fact\"");
        let deserialized: FactClass = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, FactClass::VerifiedFact);
    }

    #[test]
    fn test_symbol_kind_serde_and_display() {
        let kind = SymbolKind::Function;
        assert_eq!(kind.as_str(), "function");
        assert_eq!(kind.to_string(), "function");
        assert_eq!(SymbolKind::from_str_name("fn"), Some(SymbolKind::Function));

        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"function\"");
        let deserialized: SymbolKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SymbolKind::Function);
    }

    #[test]
    fn test_repository_symbol_creation() {
        let sym = RepositorySymbol::new(
            "src/lib.rs::fn::run",
            "run",
            "m31a::run",
            SymbolKind::Function,
            "src/lib.rs",
            10,
            25,
            "pub fn run() -> Result<(), Error>",
            FactClass::VerifiedFact,
            Some("Runs the kernel.".to_string()),
        );
        assert_eq!(sym.name, "run");
        assert_eq!(sym.start_line, 10);
        assert_eq!(sym.end_line, 25);
        assert!(sym.fact_class.is_verified());
        assert!(!sym.is_test);

        let test_sym = sym.with_test(true);
        assert!(test_sym.is_test);
    }

    #[test]
    fn test_file_classification_and_subsystems() {
        assert_eq!(FileClassification::Source.as_str(), "source");
        assert_eq!(
            FileClassification::from_str_name("test"),
            Some(FileClassification::Test)
        );
        assert!(FileClassification::Source.is_source_or_test());
        assert!(FileClassification::Test.is_source_or_test());
        assert!(!FileClassification::Documentation.is_source_or_test());

        assert_eq!(
            SubsystemKind::detect_from_path("src/kernel/error.rs"),
            SubsystemKind::Kernel
        );
        assert_eq!(
            SubsystemKind::detect_from_path("src/policy/gate.rs"),
            SubsystemKind::Security
        );
        assert_eq!(
            SubsystemKind::detect_from_path("src/repo/graph.rs"),
            SubsystemKind::Intelligence
        );
        assert_eq!(
            SubsystemKind::detect_from_path("src/planning/constraints.rs"),
            SubsystemKind::Planning
        );
        assert_eq!(
            SubsystemKind::detect_from_path("src/cli/main.rs"),
            SubsystemKind::UserInterface
        );
    }

    #[test]
    fn test_provenance_and_entry_point() {
        let prov = EvidenceProvenance::verified_ast("src/lib.rs", 10, 20);
        assert_eq!(prov.method, AnalysisMethod::AstVisitor);
        assert_eq!(prov.fact_class, FactClass::VerifiedFact);

        let ep = EntryPoint::new(
            "ep::main",
            EntryPointKind::BinaryMain,
            "src/main.rs",
            Some("main".to_string()),
            1,
            "Application entry point",
        );
        assert_eq!(ep.kind, EntryPointKind::BinaryMain);
        assert_eq!(ep.symbol_name.as_deref(), Some("main"));

        assert_eq!(IndexStatus::Current.as_str(), "current");
        assert!(IndexStatus::Current.is_usable());
        assert!(IndexStatus::PartiallyStale.is_usable());
        assert!(!IndexStatus::Stale.is_usable());
        assert!(!IndexStatus::Empty.is_usable());
    }
}
