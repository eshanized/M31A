use crate::repo::types::{FactClass, RepositorySymbol, SymbolKind};
use regex::Regex;
use std::sync::LazyLock;

static FN_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*([a-zA-Z0-9_$]+)\s*(?:<[^>]*>)?\s*\((.*?)\)(?:\s*:\s*([^;{]+))?"#)
        .expect("valid fn decl regex")
});

static ARROW_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*(?:export\s+)?(?:const|let|var)\s+([a-zA-Z0-9_$]+)\s*(?::\s*[^=]+)?\s*=\s*(?:async\s*)?(?:\((.*?)\)|[a-zA-Z0-9_$]+)\s*(?::\s*[^=]+)?\s*=>"#)
        .expect("valid arrow fn regex")
});

static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*(?:export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([a-zA-Z0-9_$]+)(?:\s+extends\s+[a-zA-Z0-9_$.]+)?(?:\s+implements\s+[a-zA-Z0-9_$,\s]+)?"#)
        .expect("valid class regex")
});

static INTERFACE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*(?:export\s+)?interface\s+([a-zA-Z0-9_$]+)(?:<[^>]*>)?(?:\s+extends\s+[a-zA-Z0-9_$,\s]+)?"#)
        .expect("valid interface regex")
});

static TYPE_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*(?:export\s+)?type\s+([a-zA-Z0-9_$]+)(?:<[^>]*>)?\s*="#)
        .expect("valid type alias regex")
});

static IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[ \t]*import\s+(?:(?:.+?)\s+from\s+)?['"]([^'"]+)['"]"#)
        .expect("valid import regex")
});

pub struct TypeScriptExtractor;

impl TypeScriptExtractor {
    pub fn extract(file_path: &str, content: &str) -> Vec<RepositorySymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut idx = 0;
        let mut active_doc = None;
        let mut pending_doc_lines = Vec::new();
        let mut in_block_comment = false;

        while idx < lines.len() {
            let line = lines[idx];
            let line_num = idx + 1;
            let trimmed = line.trim();

            // Comment parsing
            if in_block_comment {
                if let Some(end_idx) = trimmed.find("*/") {
                    let doc_part = trimmed[..end_idx].trim_start_matches('*').trim();
                    if !doc_part.is_empty() {
                        pending_doc_lines.push(doc_part.to_string());
                    }
                    in_block_comment = false;
                    active_doc = Some(pending_doc_lines.join("\n"));
                    pending_doc_lines.clear();
                } else {
                    let doc_part = trimmed.trim_start_matches('*').trim();
                    if !doc_part.is_empty() {
                        pending_doc_lines.push(doc_part.to_string());
                    }
                }
                idx += 1;
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("/**") {
                if let Some(end_idx) = rest.find("*/") {
                    let doc_part = rest[..end_idx].trim_start_matches('*').trim();
                    active_doc = Some(doc_part.to_string());
                } else {
                    in_block_comment = true;
                    pending_doc_lines.clear();
                    let doc_part = rest.trim_start_matches('*').trim();
                    if !doc_part.is_empty() {
                        pending_doc_lines.push(doc_part.to_string());
                    }
                }
                idx += 1;
                continue;
            }

            if trimmed.starts_with("//") {
                // Single line comment
                idx += 1;
                continue;
            }

            // 1. Function declaration
            if let Some(caps) = FN_DECL_RE.captures(line) {
                let name = caps[1].to_string();
                let params = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                let ret = caps
                    .get(3)
                    .map(|m| format!(": {}", m.as_str().trim()))
                    .unwrap_or_default();
                let sig = format!("function {name}({params}){ret}");
                let qualified_name = format!("{file_path}::{name}");
                let id = format!("{qualified_name}#fn");
                let end_line = Self::find_closing_brace(&lines, idx);

                symbols.push(RepositorySymbol::new(
                    id,
                    name,
                    qualified_name,
                    SymbolKind::Function,
                    file_path,
                    line_num,
                    end_line,
                    sig,
                    FactClass::InferredFact,
                    active_doc.take(),
                ));

                idx += 1;
                continue;
            }

            // 2. Arrow function
            if let Some(caps) = ARROW_FN_RE.captures(line) {
                let name = caps[1].to_string();
                let params = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                let sig = format!("const {name} = ({params}) => ...");
                let qualified_name = format!("{file_path}::{name}");
                let id = format!("{qualified_name}#fn");
                let end_line = Self::find_closing_brace(&lines, idx);

                symbols.push(RepositorySymbol::new(
                    id,
                    name,
                    qualified_name,
                    SymbolKind::Function,
                    file_path,
                    line_num,
                    end_line,
                    sig,
                    FactClass::InferredFact,
                    active_doc.take(),
                ));

                idx += 1;
                continue;
            }

            // 3. Class
            if let Some(caps) = CLASS_RE.captures(line) {
                let name = caps[1].to_string();
                let sig = line.trim_end_matches('{').trim().to_string();
                let qualified_name = format!("{file_path}::{name}");
                let id = format!("{qualified_name}#class");
                let end_line = Self::find_closing_brace(&lines, idx);

                symbols.push(RepositorySymbol::new(
                    id,
                    name,
                    qualified_name,
                    SymbolKind::Class,
                    file_path,
                    line_num,
                    end_line,
                    sig,
                    FactClass::InferredFact,
                    active_doc.take(),
                ));

                idx += 1;
                continue;
            }

            // 4. Interface
            if let Some(caps) = INTERFACE_RE.captures(line) {
                let name = caps[1].to_string();
                let sig = line.trim_end_matches('{').trim().to_string();
                let qualified_name = format!("{file_path}::{name}");
                let id = format!("{qualified_name}#interface");
                let end_line = Self::find_closing_brace(&lines, idx);

                symbols.push(RepositorySymbol::new(
                    id,
                    name,
                    qualified_name,
                    SymbolKind::Interface,
                    file_path,
                    line_num,
                    end_line,
                    sig,
                    FactClass::InferredFact,
                    active_doc.take(),
                ));

                idx += 1;
                continue;
            }

            // 5. Type Alias
            if let Some(caps) = TYPE_ALIAS_RE.captures(line) {
                let name = caps[1].to_string();
                let sig = line.trim().to_string();
                let qualified_name = format!("{file_path}::{name}");
                let id = format!("{qualified_name}#type");

                symbols.push(RepositorySymbol::new(
                    id,
                    name,
                    qualified_name,
                    SymbolKind::TypeAlias,
                    file_path,
                    line_num,
                    line_num,
                    sig,
                    FactClass::InferredFact,
                    active_doc.take(),
                ));

                idx += 1;
                continue;
            }

            // 6. Imports
            if let Some(caps) = IMPORT_RE.captures(line) {
                let source = caps[1].to_string();
                let sig = line.trim().to_string();
                let id = format!("{file_path}::import::{source}#{line_num}");

                symbols.push(RepositorySymbol::new(
                    id,
                    source.clone(),
                    format!("{file_path}::{source}"),
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

            // Reset doc if non-comment code wasn't matched
            if !trimmed.is_empty() {
                active_doc = None;
            }

            idx += 1;
        }

        symbols
    }

    fn find_closing_brace(lines: &[&str], start_idx: usize) -> usize {
        let mut depth = 0;
        let mut started = false;

        for (i, line) in lines.iter().enumerate().skip(start_idx) {
            for ch in line.chars() {
                if ch == '{' {
                    depth += 1;
                    started = true;
                } else if ch == '}' && started {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
            }
        }
        start_idx + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_extractor() {
        let ts_code = r#"
import { Router } from 'express';
import axios from "axios";

/**
 * User configuration options.
 */
export interface UserConfig {
    id: string;
    timeout: number;
}

export type Status = 'active' | 'inactive';

/**
 * Controller class handling requests.
 */
export class UserController extends BaseController {
    constructor() {
        super();
    }
}

export async function fetchUser(id: string): Promise<UserConfig> {
    return { id, timeout: 3000 };
}

export const processUser = async (user: UserConfig) => {
    console.log(user);
};
"#;

        let symbols = TypeScriptExtractor::extract("src/controllers/user.ts", ts_code);
        assert!(!symbols.is_empty());

        for s in &symbols {
            assert_eq!(s.fact_class, FactClass::InferredFact);
        }

        // Interface
        let iface = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Interface)
            .unwrap();
        assert_eq!(iface.name, "UserConfig");
        assert_eq!(
            iface.doc_comment.as_deref(),
            Some("User configuration options.")
        );

        // Type Alias
        let type_alias = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::TypeAlias)
            .unwrap();
        assert_eq!(type_alias.name, "Status");

        // Class
        let cls = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Class)
            .unwrap();
        assert_eq!(cls.name, "UserController");
        assert_eq!(
            cls.doc_comment.as_deref(),
            Some("Controller class handling requests.")
        );

        // Function
        let fn_decl = symbols.iter().find(|s| s.name == "fetchUser").unwrap();
        assert_eq!(fn_decl.kind, SymbolKind::Function);

        // Arrow function
        let arrow = symbols.iter().find(|s| s.name == "processUser").unwrap();
        assert_eq!(arrow.kind, SymbolKind::Function);

        // Imports
        let imps: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Module)
            .collect();
        assert_eq!(imps.len(), 2);
    }
}
