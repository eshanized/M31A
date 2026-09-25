use crate::repo::types::{FactClass, RepositorySymbol, SymbolKind};
use regex::Regex;
use std::sync::LazyLock;

static DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^([ \t]*)def\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\((.*?)\)(?:\s*->\s*([^:]+))?:"#)
        .expect("valid def regex")
});

static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^([ \t]*)class\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*\((.*?)\))?:"#)
        .expect("valid class regex")
});

static FROM_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*from\s+([a-zA-Z0-9_.]+)\s+import\s+([a-zA-Z0-9_,\s*]+)"#)
        .expect("valid from import regex")
});

static DIRECT_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*import\s+([a-zA-Z0-9_.,\s]+)"#).expect("valid direct import regex")
});

pub struct PythonExtractor;

impl PythonExtractor {
    pub fn extract(file_path: &str, content: &str) -> Vec<RepositorySymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut class_stack: Vec<(usize, String)> = Vec::new(); // (indent, class_name)

        let mut idx = 0;
        while idx < lines.len() {
            let line = lines[idx];
            let line_num = idx + 1;

            // Trim trailing comment if any for clean matching
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                idx += 1;
                continue;
            }

            // Calculate leading indentation in spaces
            let indent = line
                .chars()
                .take_while(|c| c.is_whitespace())
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum::<usize>();

            // Pop classes from stack if indentation has dedented
            while let Some((c_indent, _)) = class_stack.last() {
                if indent <= *c_indent {
                    class_stack.pop();
                } else {
                    break;
                }
            }

            // 1. Class definition
            if let Some(caps) = CLASS_RE.captures(line) {
                let class_name = caps[2].to_string();
                let inheritance = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("");
                let sig = if inheritance.is_empty() {
                    format!("class {class_name}")
                } else {
                    format!("class {class_name}({inheritance})")
                };

                let qualified_name = if let Some((_, parent_class)) = class_stack.last() {
                    format!("{file_path}::{parent_class}::{class_name}")
                } else {
                    format!("{file_path}::{class_name}")
                };
                let id = format!("{qualified_name}#class");

                let (doc, end_line) = Self::extract_doc_and_extent(&lines, idx, indent);

                symbols.push(RepositorySymbol::new(
                    id,
                    class_name.clone(),
                    qualified_name,
                    SymbolKind::Class,
                    file_path,
                    line_num,
                    end_line,
                    sig,
                    FactClass::InferredFact,
                    doc,
                ));

                class_stack.push((indent, class_name));
                idx += 1;
                continue;
            }

            // 2. Function / Method definition
            if let Some(caps) = DEF_RE.captures(line) {
                let fn_name = caps[2].to_string();
                let params = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("");
                let ret = caps.get(4).map(|m| m.as_str().trim()).unwrap_or("");
                let sig = if ret.is_empty() {
                    format!("def {fn_name}({params})")
                } else {
                    format!("def {fn_name}({params}) -> {ret}")
                };

                let qualified_name = if let Some((_, parent_class)) = class_stack.last() {
                    format!("{file_path}::{parent_class}::{fn_name}")
                } else {
                    format!("{file_path}::{fn_name}")
                };
                let id = format!("{qualified_name}#fn");

                let (doc, end_line) = Self::extract_doc_and_extent(&lines, idx, indent);

                symbols.push(RepositorySymbol::new(
                    id,
                    fn_name,
                    qualified_name,
                    SymbolKind::Function,
                    file_path,
                    line_num,
                    end_line,
                    sig,
                    FactClass::InferredFact,
                    doc,
                ));

                idx += 1;
                continue;
            }

            // 3. Imports
            if let Some(caps) = FROM_IMPORT_RE.captures(line) {
                let module = caps[1].to_string();
                let imported = caps[2].trim().to_string();
                let sig = format!("from {module} import {imported}");
                let id = format!("{file_path}::import::{module}#{line_num}");

                symbols.push(RepositorySymbol::new(
                    id,
                    module.clone(),
                    format!("{file_path}::{module}"),
                    SymbolKind::Module,
                    file_path,
                    line_num,
                    line_num,
                    sig,
                    FactClass::InferredFact,
                    None,
                ));

                idx += 1;
                continue;
            }

            if let Some(caps) = DIRECT_IMPORT_RE.captures(line) {
                let modules = caps[1].trim().to_string();
                let sig = format!("import {modules}");
                let id = format!("{file_path}::import::{modules}#{line_num}");

                symbols.push(RepositorySymbol::new(
                    id,
                    modules.clone(),
                    format!("{file_path}::{modules}"),
                    SymbolKind::Module,
                    file_path,
                    line_num,
                    line_num,
                    sig,
                    FactClass::InferredFact,
                    None,
                ));

                idx += 1;
                continue;
            }

            idx += 1;
        }

        symbols
    }

    fn extract_doc_and_extent(
        lines: &[&str],
        start_idx: usize,
        base_indent: usize,
    ) -> (Option<String>, usize) {
        let mut doc = None;
        let mut end_line = start_idx + 1;

        // Check next line for docstring
        if start_idx + 1 < lines.len() {
            let next_line = lines[start_idx + 1].trim();
            if next_line.starts_with("\"\"\"") || next_line.starts_with("'''") {
                let quote = if next_line.starts_with("\"\"\"") {
                    "\"\"\""
                } else {
                    "'''"
                };
                let rest = &next_line[3..];
                if let Some(end_pos) = rest.find(quote) {
                    // Single-line docstring
                    doc = Some(rest[..end_pos].trim().to_string());
                } else {
                    // Multi-line docstring
                    let mut doc_lines = vec![rest.to_string()];
                    let mut cur = start_idx + 2;
                    while cur < lines.len() {
                        let l = lines[cur];
                        if let Some(pos) = l.find(quote) {
                            doc_lines.push(l[..pos].trim().to_string());
                            break;
                        } else {
                            doc_lines.push(l.trim().to_string());
                        }
                        cur += 1;
                    }
                    doc = Some(doc_lines.join("\n"));
                }
            }
        }

        // Estimate extent: find where indentation returns to <= base_indent
        for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let cur_indent = line
                .chars()
                .take_while(|c| c.is_whitespace())
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum::<usize>();
            if cur_indent <= base_indent {
                end_line = i;
                break;
            }
            end_line = i + 1;
        }

        (doc, end_line.max(start_idx + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_extractor() {
        let code = r#"
import os, sys
from typing import List, Optional

class Engine(BaseEngine):
    """The core engine."""

    def __init__(self, name: str):
        self.name = name

    def start(self) -> bool:
        return True

def standalone_fn(x: int, y: int) -> int:
    """Add two numbers."""
    return x + y
"#;

        let symbols = PythonExtractor::extract("app/engine.py", code);
        assert!(!symbols.is_empty());

        // Verify FactClass is InferredFact for all
        for s in &symbols {
            assert_eq!(s.fact_class, FactClass::InferredFact);
        }

        // Verify Class
        let cls = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Class)
            .unwrap();
        assert_eq!(cls.name, "Engine");
        assert_eq!(cls.qualified_name, "app/engine.py::Engine");
        assert_eq!(cls.doc_comment.as_deref(), Some("The core engine."));

        // Verify Methods
        let init = symbols.iter().find(|s| s.name == "__init__").unwrap();
        assert_eq!(init.qualified_name, "app/engine.py::Engine::__init__");
        assert_eq!(init.kind, SymbolKind::Function);

        let start = symbols.iter().find(|s| s.name == "start").unwrap();
        assert_eq!(start.qualified_name, "app/engine.py::Engine::start");

        // Verify Standalone Function
        let stand = symbols.iter().find(|s| s.name == "standalone_fn").unwrap();
        assert_eq!(stand.qualified_name, "app/engine.py::standalone_fn");
        assert_eq!(stand.doc_comment.as_deref(), Some("Add two numbers."));

        // Verify Imports
        let imps: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Module)
            .collect();
        assert_eq!(imps.len(), 2);
    }
}
