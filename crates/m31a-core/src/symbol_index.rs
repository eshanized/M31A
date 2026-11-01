//! AST and symbol intelligence engine for semantic codebase navigation.
//!
//! Provides structural understanding of codebases without blowing up LLM context windows:
//! - Outline generation (extracts definitions, signatures, and line ranges)
//! - Symbol lookup (`find_definition`)
//! - Reference scanning (`find_references`)
//! - Multi-language support: Rust, Python, TypeScript/JavaScript, Go.

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

    /// Recursively scans and indexes supported source files in the workspace.
    pub fn build_from_workspace(root: &Path, max_files: usize) -> Self {
        let mut index = Self::new();
        let mut files_visited = 0;
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
                        let symbols = parse_symbols_from_source(&path, &content);
                        for sym in symbols {
                            index
                                .symbols_by_name
                                .entry(sym.name.clone())
                                .or_default()
                                .push(sym);
                        }
                        files_visited += 1;
                        if files_visited >= max_files {
                            index.files_indexed = files_visited;
                            return index;
                        }
                    }
                }
            }
        }

        index.files_indexed = files_visited;
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

    /// Scans the workspace for textual references to a symbol name.
    pub fn find_references(
        root: &Path,
        symbol_name: &str,
        max_results: usize,
    ) -> Vec<SymbolReference> {
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

/// Parses top-level and method definitions from source content using regex/heuristics.
pub fn parse_symbols_from_source(path: &Path, content: &str) -> Vec<SymbolItem> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "rs" => parse_rust_symbols(path, content),
        "py" => parse_python_symbols(path, content),
        "ts" | "js" | "tsx" | "jsx" => parse_typescript_symbols(path, content),
        "go" => parse_go_symbols(path, content),
        _ => Vec::new(),
    }
}

fn parse_rust_symbols(path: &Path, content: &str) -> Vec<SymbolItem> {
    let mut symbols = Vec::new();
    let mut current_doc = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;

        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            current_doc.push(trimmed.trim_start_matches('/').trim());
            continue;
        }

        let doc = if current_doc.is_empty() {
            None
        } else {
            let joined = current_doc.join("\n");
            current_doc.clear();
            Some(joined)
        };

        let is_pub = trimmed.starts_with("pub ");
        let visibility = if is_pub {
            SymbolVisibility::Public
        } else {
            SymbolVisibility::Private
        };

        // Functions
        if let Some(rest) = trimmed
            .strip_prefix("pub fn ")
            .or_else(|| trimmed.strip_prefix("fn "))
            && let Some((name, _)) = rest.split_once('(')
        {
            let sym_name = name.split_whitespace().last().unwrap_or(name.trim());
            symbols.push(SymbolItem {
                name: sym_name.to_string(),
                kind: SymbolKind::Function,
                file_path: path.to_path_buf(),
                line: line_num,
                signature: trimmed
                    .split('{')
                    .next()
                    .unwrap_or(trimmed)
                    .trim()
                    .to_string(),
                visibility,
                doc_comment: doc,
            });
            continue;
        }

        // Structs
        if let Some(rest) = trimmed
            .strip_prefix("pub struct ")
            .or_else(|| trimmed.strip_prefix("struct "))
        {
            let name = rest
                .split(&['<', '{', ';', '('][..])
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: SymbolKind::Struct,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility,
                    doc_comment: doc,
                });
                continue;
            }
        }

        // Enums
        if let Some(rest) = trimmed
            .strip_prefix("pub enum ")
            .or_else(|| trimmed.strip_prefix("enum "))
        {
            let name = rest.split(&['<', '{'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: SymbolKind::Enum,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility,
                    doc_comment: doc,
                });
                continue;
            }
        }

        // Traits
        if let Some(rest) = trimmed
            .strip_prefix("pub trait ")
            .or_else(|| trimmed.strip_prefix("trait "))
        {
            let name = rest.split(&['<', '{', ':'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: SymbolKind::Trait,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility,
                    doc_comment: doc,
                });
            }
        }
    }
    symbols
}

fn parse_python_symbols(path: &Path, content: &str) -> Vec<SymbolItem> {
    let mut symbols = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;

        if let Some(rest) = trimmed.strip_prefix("def ") {
            if let Some((name, _)) = rest.split_once('(') {
                symbols.push(SymbolItem {
                    name: name.trim().to_string(),
                    kind: SymbolKind::Function,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed.to_string(),
                    visibility: if name.trim().starts_with('_') {
                        SymbolVisibility::Private
                    } else {
                        SymbolVisibility::Public
                    },
                    doc_comment: None,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("class ") {
            let name = rest.split(&['(', ':'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed.to_string(),
                    visibility: SymbolVisibility::Public,
                    doc_comment: None,
                });
            }
        }
    }
    symbols
}

fn parse_typescript_symbols(path: &Path, content: &str) -> Vec<SymbolItem> {
    let mut symbols = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;
        let is_export = trimmed.starts_with("export ");
        let normalized = if is_export {
            trimmed.strip_prefix("export ").unwrap_or(trimmed).trim()
        } else {
            trimmed
        };

        if let Some(rest) = normalized.strip_prefix("function ") {
            if let Some((name, _)) = rest.split_once('(') {
                symbols.push(SymbolItem {
                    name: name.trim().to_string(),
                    kind: SymbolKind::Function,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility: if is_export {
                        SymbolVisibility::Public
                    } else {
                        SymbolVisibility::Private
                    },
                    doc_comment: None,
                });
            }
        } else if let Some(rest) = normalized.strip_prefix("class ") {
            let name = rest.split(&[' ', '<', '{'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility: if is_export {
                        SymbolVisibility::Public
                    } else {
                        SymbolVisibility::Private
                    },
                    doc_comment: None,
                });
            }
        } else if let Some(rest) = normalized.strip_prefix("interface ") {
            let name = rest.split(&[' ', '<', '{'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility: if is_export {
                        SymbolVisibility::Public
                    } else {
                        SymbolVisibility::Private
                    },
                    doc_comment: None,
                });
            }
        }
    }
    symbols
}

fn parse_go_symbols(path: &Path, content: &str) -> Vec<SymbolItem> {
    let mut symbols = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;

        if let Some(rest) = trimmed.strip_prefix("func ") {
            // Function or Method
            let is_method = rest.starts_with('(');
            let name_part = if is_method {
                rest.split_once(')')
                    .map(|(_, after)| after.trim())
                    .unwrap_or("")
            } else {
                rest
            };
            if let Some((name, _)) = name_part.split_once('(') {
                let name = name.trim();
                let is_exported = name.chars().next().is_some_and(|c| c.is_uppercase());
                symbols.push(SymbolItem {
                    name: name.to_string(),
                    kind: if is_method {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    },
                    file_path: path.to_path_buf(),
                    line: line_num,
                    signature: trimmed
                        .split('{')
                        .next()
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                    visibility: if is_exported {
                        SymbolVisibility::Public
                    } else {
                        SymbolVisibility::Private
                    },
                    doc_comment: None,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("type ")
            && let Some((name, kind_str)) = rest.split_once(' ')
        {
            let kind_str = kind_str.trim();
            let kind = if kind_str.starts_with("struct") {
                SymbolKind::Struct
            } else if kind_str.starts_with("interface") {
                SymbolKind::Interface
            } else {
                SymbolKind::TypeAlias
            };
            let is_exported = name.chars().next().is_some_and(|c| c.is_uppercase());
            symbols.push(SymbolItem {
                name: name.to_string(),
                kind,
                file_path: path.to_path_buf(),
                line: line_num,
                signature: trimmed
                    .split('{')
                    .next()
                    .unwrap_or(trimmed)
                    .trim()
                    .to_string(),
                visibility: if is_exported {
                    SymbolVisibility::Public
                } else {
                    SymbolVisibility::Private
                },
                doc_comment: None,
            });
        }
    }
    symbols
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
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "UserPayload");
        assert_eq!(symbols[0].kind, SymbolKind::Interface);
        assert_eq!(symbols[1].name, "UserService");
        assert_eq!(symbols[1].kind, SymbolKind::Class);
        assert_eq!(symbols[2].name, "fetchUser");
        assert_eq!(symbols[2].kind, SymbolKind::Function);
    }
}
