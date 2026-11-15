//! AST and symbol intelligence engine adapter for semantic codebase navigation.
//!
//! Provides structural understanding of codebases by delegating to the authoritative
//! `m31a-ast` parsing and indexing engine while maintaining backwards-compatible types:
//! - Outline generation (`FileOutline`, `generate_file_outline`)
//! - Symbol lookup (`SymbolIndex`, `find_definition`)
//! - Reference scanning (`SymbolReference`, `find_references`)
//! - Multi-language support: Rust, Python, TypeScript/JavaScript, Go, C/C++.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Class,
    Enum,
    Trait,
    Interface,
    TypeAlias,
    Constant,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Class => "class",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Interface => "interface",
            Self::TypeAlias => "type",
            Self::Constant => "constant",
        }
    }
}

impl From<m31a_ast::AstSymbolKind> for SymbolKind {
    fn from(k: m31a_ast::AstSymbolKind) -> Self {
        match k {
            m31a_ast::AstSymbolKind::Function => Self::Function,
            m31a_ast::AstSymbolKind::Method => Self::Method,
            m31a_ast::AstSymbolKind::Struct => Self::Struct,
            m31a_ast::AstSymbolKind::Class => Self::Class,
            m31a_ast::AstSymbolKind::Enum => Self::Enum,
            m31a_ast::AstSymbolKind::Trait => Self::Trait,
            m31a_ast::AstSymbolKind::Interface => Self::Interface,
            m31a_ast::AstSymbolKind::TypeAlias => Self::TypeAlias,
            m31a_ast::AstSymbolKind::Constant => Self::Constant,
            _ => Self::Function,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SymbolVisibility {
    Public,
    Private,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SymbolItem {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: PathBuf,
    pub line: usize,
    pub signature: String,
    pub visibility: SymbolVisibility,
    pub doc_comment: Option<String>,
}

impl From<m31a_ast::AstSymbol> for SymbolItem {
    fn from(s: m31a_ast::AstSymbol) -> Self {
        Self {
            name: s.name,
            kind: SymbolKind::from(s.kind),
            file_path: s.file_path,
            line: s.range.start_line,
            signature: s.signature,
            visibility: if s.is_public {
                SymbolVisibility::Public
            } else {
                SymbolVisibility::Private
            },
            doc_comment: s.doc_comment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SymbolReference {
    pub file_path: PathBuf,
    pub line: usize,
    pub line_content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileOutline {
    pub file_path: PathBuf,
    pub total_lines: usize,
    pub symbols: Vec<SymbolItem>,
}

#[derive(Debug, Default)]
pub struct SymbolIndex {
    symbols_by_name: HashMap<String, Vec<SymbolItem>>,
    files_indexed: usize,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recursively scans and indexes supported source files in the workspace using canonical AST engine.
    pub fn build_from_workspace(root: &Path, max_files: usize) -> Self {
        let ast_index = m31a_ast::AstCodebaseIndex::build(root, max_files);
        let mut index = Self::new();

        for sym in ast_index.all_symbols() {
            let item = SymbolItem::from(sym.clone());
            index
                .symbols_by_name
                .entry(item.name.clone())
                .or_default()
                .push(item);
        }

        index.files_indexed = ast_index.files_indexed();
        index
    }

    pub fn files_indexed(&self) -> usize {
        self.files_indexed
    }

    /// Finds symbol definitions matching the query name.
    pub fn find_definition(&self, symbol_name: &str) -> Vec<SymbolItem> {
        self.symbols_by_name
            .get(symbol_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Generates a concise file outline for a specific source file.
    pub fn generate_file_outline(path: &Path) -> anyhow::Result<FileOutline> {
        let content = fs::read_to_string(path)?;
        let total_lines = content.lines().count();
        let symbols = parse_symbols_from_source(path, &content);
        Ok(FileOutline {
            file_path: path.to_path_buf(),
            total_lines,
            symbols,
        })
    }

    /// Scans the workspace for references or calls to a symbol name.
    pub fn find_references(
        root: &Path,
        symbol_name: &str,
        max_results: usize,
    ) -> Vec<SymbolReference> {
        let callers = m31a_ast::AstCodebaseIndex::find_callers(root, symbol_name, max_results);
        if !callers.is_empty() {
            return callers
                .into_iter()
                .map(|c| SymbolReference {
                    file_path: c.file_path,
                    line: c.line,
                    line_content: format!("call to `{}` by `{}`", c.callee_name, c.caller_name),
                })
                .collect();
        }

        // Textual fallback
        let mut references = Vec::new();
        let mut dirs_to_visit = vec![root.to_path_buf()];

        while let Some(dir) = dirs_to_visit.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let file_name = entry.file_name();
                    let name_str = file_name.to_string_lossy();

                    if name_str.starts_with('.')
                        || matches!(
                            name_str.as_ref(),
                            "target" | "node_modules" | "dist" | "__pycache__"
                        )
                    {
                        continue;
                    }

                    if path.is_dir() {
                        dirs_to_visit.push(path);
                    } else if path.is_file()
                        && is_supported_source_file(&path)
                        && let Ok(content) = fs::read_to_string(&path)
                    {
                        for (line_idx, line) in content.lines().enumerate() {
                            if line.contains(symbol_name) {
                                references.push(SymbolReference {
                                    file_path: path.to_path_buf(),
                                    line: line_idx + 1,
                                    line_content: line.trim().to_string(),
                                });
                                if references.len() >= max_results {
                                    return references;
                                }
                            }
                        }
                    }
                }
            }
        }
        references
    }
}

fn is_supported_source_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py" | "ts" | "js" | "tsx" | "jsx" | "go" | "c" | "cpp" | "h" | "hpp"
    )
}

/// Parses top-level and method definitions from source content using the authoritative AST engine.
pub fn parse_symbols_from_source(path: &Path, content: &str) -> Vec<SymbolItem> {
    let syms = m31a_ast::AstParser::parse_file(path, content);
    syms.into_iter().map(SymbolItem::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_symbols() {
        let src = r#"
/// Documentation for Person
pub struct Person {
    pub name: String,
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Greeter {
    fn greet(&self) -> String;
}

pub fn create_person(name: &str) -> Person {
    Person { name: name.to_string() }
}
"#;
        let symbols = parse_symbols_from_source(Path::new("src/person.rs"), src);
        assert_eq!(symbols.len(), 5);
        assert_eq!(symbols[0].name, "Person");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(
            symbols[0].doc_comment.as_deref(),
            Some("Documentation for Person")
        );
        assert_eq!(symbols[1].name, "Status");
        assert_eq!(symbols[1].kind, SymbolKind::Enum);
        assert_eq!(symbols[2].name, "Greeter");
        assert_eq!(symbols[2].kind, SymbolKind::Trait);
        assert_eq!(symbols[3].name, "greet");
        assert_eq!(symbols[4].name, "create_person");
        assert_eq!(symbols[4].kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_python_symbols() {
        let src = r#"
class DataProcessor:
    def process(self, data):
        pass

def run_job(timeout: int = 30):
    pass
"#;
        let symbols = parse_symbols_from_source(Path::new("job.py"), src);
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "DataProcessor");
        assert_eq!(symbols[0].kind, SymbolKind::Class);
        assert_eq!(symbols[1].name, "process");
        assert_eq!(symbols[2].name, "run_job");
    }

    #[test]
    fn test_parse_typescript_symbols() {
        let src = r#"
export interface UserPayload {
    id: string;
}

export class UserService {
    save() {}
}

export function fetchUser(id: string): UserPayload {
    return { id };
}
"#;
        let symbols = parse_symbols_from_source(Path::new("service.ts"), src);
        // Includes interface, class, method, function
        assert!(symbols.len() >= 3);
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "UserPayload" && s.kind == SymbolKind::Interface)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "fetchUser" && s.kind == SymbolKind::Function)
        );
    }
}
