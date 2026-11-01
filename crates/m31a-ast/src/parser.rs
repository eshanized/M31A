//! Multi-language AST parsing and symbol extraction.

use crate::types::{AstSymbol, AstSymbolKind, SourceRange};
use std::path::Path;

pub struct AstParser;

impl AstParser {
    pub fn parse_file(path: &Path, content: &str) -> Vec<AstSymbol> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "rs" => parse_rust(path, content),
            "ts" | "tsx" | "js" | "jsx" => parse_typescript(path, content),
            "py" => parse_python(path, content),
            "go" => parse_go(path, content),
            "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "java" | "cs" => parse_c_like(path, content),
            _ => parse_generic(path, content),
        }
    }
}

// ---------------------------------------------------------------------------
// Rust Parser
// ---------------------------------------------------------------------------

use syn::spanned::Spanned;

fn extract_syn_doc(attrs: &[syn::Attribute]) -> Option<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(meta) = &attr.meta
            && let syn::Expr::Lit(expr_lit) = &meta.value
            && let syn::Lit::Str(lit_str) = &expr_lit.lit
        {
            let text = lit_str.value();
            docs.push(text.trim().to_string());
        }
    }
    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

fn syn_item_start_line(vis: &syn::Visibility, fallback_span: proc_macro2::Span) -> usize {
    match vis {
        syn::Visibility::Public(p) => p.span.start().line,
        syn::Visibility::Restricted(r) => r.pub_token.span.start().line,
        syn::Visibility::Inherited => fallback_span.start().line,
    }
}

fn parse_rust_syn(
    path: &Path,
    content: &str,
    lines: &[&str],
) -> Result<Vec<AstSymbol>, syn::Error> {
    let file = syn::parse_file(content)?;
    let mut symbols = Vec::new();

    fn process_items(
        items: &[syn::Item],
        path: &Path,
        lines: &[&str],
        parent_scope: Option<&str>,
        symbols: &mut Vec<AstSymbol>,
    ) {
        for item in items {
            match item {
                syn::Item::Fn(f) => {
                    let start_line = syn_item_start_line(&f.vis, f.sig.fn_token.span);
                    let end_line = f.span().end().line;
                    let is_public = !matches!(f.vis, syn::Visibility::Inherited);
                    let mut sym = AstSymbol::new(
                        f.sig.ident.to_string(),
                        if parent_scope.is_some() {
                            AstSymbolKind::Method
                        } else {
                            AstSymbolKind::Function
                        },
                        path.to_path_buf(),
                        SourceRange::new(
                            start_line,
                            end_line,
                            0,
                            lines
                                .get(start_line.saturating_sub(1))
                                .map(|l| l.len())
                                .unwrap_or(0),
                        ),
                        extract_signature(lines, start_line.saturating_sub(1)),
                    );
                    sym.is_public = is_public;
                    sym.doc_comment = extract_syn_doc(&f.attrs);
                    sym.parent_scope = parent_scope.map(|s| s.to_string());
                    symbols.push(sym);
                }
                syn::Item::Struct(s) => {
                    let start_line = syn_item_start_line(&s.vis, s.struct_token.span);
                    let end_line = s.span().end().line;
                    let is_public = !matches!(s.vis, syn::Visibility::Inherited);
                    let mut sym = AstSymbol::new(
                        s.ident.to_string(),
                        AstSymbolKind::Struct,
                        path.to_path_buf(),
                        SourceRange::new(
                            start_line,
                            end_line,
                            0,
                            lines
                                .get(start_line.saturating_sub(1))
                                .map(|l| l.len())
                                .unwrap_or(0),
                        ),
                        extract_signature(lines, start_line.saturating_sub(1)),
                    );
                    sym.is_public = is_public;
                    sym.doc_comment = extract_syn_doc(&s.attrs);
                    symbols.push(sym);
                }
                syn::Item::Enum(e) => {
                    let start_line = syn_item_start_line(&e.vis, e.enum_token.span);
                    let end_line = e.span().end().line;
                    let is_public = !matches!(e.vis, syn::Visibility::Inherited);
                    let mut sym = AstSymbol::new(
                        e.ident.to_string(),
                        AstSymbolKind::Enum,
                        path.to_path_buf(),
                        SourceRange::new(
                            start_line,
                            end_line,
                            0,
                            lines
                                .get(start_line.saturating_sub(1))
                                .map(|l| l.len())
                                .unwrap_or(0),
                        ),
                        extract_signature(lines, start_line.saturating_sub(1)),
                    );
                    sym.is_public = is_public;
                    sym.doc_comment = extract_syn_doc(&e.attrs);
                    symbols.push(sym);
                }
                syn::Item::Trait(t) => {
                    let start_line = syn_item_start_line(&t.vis, t.trait_token.span);
                    let end_line = t.span().end().line;
                    let is_public = !matches!(t.vis, syn::Visibility::Inherited);
                    let mut sym = AstSymbol::new(
                        t.ident.to_string(),
                        AstSymbolKind::Trait,
                        path.to_path_buf(),
                        SourceRange::new(
                            start_line,
                            end_line,
                            0,
                            lines
                                .get(start_line.saturating_sub(1))
                                .map(|l| l.len())
                                .unwrap_or(0),
                        ),
                        extract_signature(lines, start_line.saturating_sub(1)),
                    );
                    sym.is_public = is_public;
                    sym.doc_comment = extract_syn_doc(&t.attrs);
                    symbols.push(sym);
                }
                syn::Item::Impl(imp) => {
                    let target_name = match &*imp.self_ty {
                        syn::Type::Path(p) => p
                            .path
                            .segments
                            .last()
                            .map(|s| s.ident.to_string())
                            .unwrap_or_else(|| "Self".to_string()),
                        _ => "Self".to_string(),
                    };
                    for impl_item in &imp.items {
                        if let syn::ImplItem::Fn(f) = impl_item {
                            let start_line = syn_item_start_line(&f.vis, f.sig.fn_token.span);
                            let end_line = f.span().end().line;
                            let is_public = !matches!(f.vis, syn::Visibility::Inherited);
                            let mut sym = AstSymbol::new(
                                f.sig.ident.to_string(),
                                AstSymbolKind::Method,
                                path.to_path_buf(),
                                SourceRange::new(
                                    start_line,
                                    end_line,
                                    0,
                                    lines
                                        .get(start_line.saturating_sub(1))
                                        .map(|l| l.len())
                                        .unwrap_or(0),
                                ),
                                extract_signature(lines, start_line.saturating_sub(1)),
                            );
                            sym.is_public = is_public;
                            sym.doc_comment = extract_syn_doc(&f.attrs);
                            sym.parent_scope = Some(target_name.clone());
                            symbols.push(sym);
                        }
                    }
                }
                syn::Item::Type(t) => {
                    let start_line = syn_item_start_line(&t.vis, t.type_token.span);
                    let end_line = t.span().end().line;
                    let is_public = !matches!(t.vis, syn::Visibility::Inherited);
                    let mut sym = AstSymbol::new(
                        t.ident.to_string(),
                        AstSymbolKind::TypeAlias,
                        path.to_path_buf(),
                        SourceRange::new(
                            start_line,
                            end_line,
                            0,
                            lines
                                .get(start_line.saturating_sub(1))
                                .map(|l| l.len())
                                .unwrap_or(0),
                        ),
                        extract_signature(lines, start_line.saturating_sub(1)),
                    );
                    sym.is_public = is_public;
                    sym.doc_comment = extract_syn_doc(&t.attrs);
                    symbols.push(sym);
                }
                syn::Item::Mod(m) => {
                    if let Some((_, ref inner_items)) = m.content {
                        process_items(inner_items, path, lines, parent_scope, symbols);
                    }
                }
                _ => {}
            }
        }
    }

    process_items(&file.items, path, lines, None, &mut symbols);
    Ok(symbols)
}

fn parse_rust(path: &Path, content: &str) -> Vec<AstSymbol> {
    let lines: Vec<&str> = content.lines().collect();
    if let Ok(syms) = parse_rust_syn(path, content, &lines) {
        return syms;
    }
    parse_rust_scanner(path, content, &lines)
}

fn parse_rust_scanner(path: &Path, _content: &str, lines: &[&str]) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let mut current_doc = Vec::new();
    let mut current_impl_target: Option<String> = None;
    let mut impl_brace_depth: usize = 0;

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let raw_line = lines[line_idx];
        let trimmed = raw_line.trim();
        let line_num = line_idx + 1;

        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            current_doc.push(trimmed.trim_start_matches('/').trim());
            line_idx += 1;
            continue;
        }

        let doc = if current_doc.is_empty() {
            None
        } else {
            let joined = current_doc.join("\n");
            current_doc.clear();
            Some(joined)
        };

        // Check if inside or leaving an impl block
        if current_impl_target.is_some() {
            let opens = raw_line.chars().filter(|&c| c == '{').count();
            let closes = raw_line.chars().filter(|&c| c == '}').count();
            impl_brace_depth += opens;
            if impl_brace_depth >= closes {
                impl_brace_depth -= closes;
            } else {
                impl_brace_depth = 0;
            }
            if impl_brace_depth == 0 {
                current_impl_target = None;
            }
        }

        let is_pub = trimmed.starts_with("pub ") || trimmed.starts_with("pub(crate) ");

        // 1. Impl block: `impl Foo` or `impl<T> Bar for Foo`
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            let rest = trimmed.strip_prefix("impl").unwrap_or("").trim();
            let target_name = rest
                .split('{')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .last()
                .unwrap_or("")
                .trim_matches(&['<', '>', ';'][..]);

            if !target_name.is_empty() {
                current_impl_target = Some(target_name.to_string());
                impl_brace_depth = raw_line.chars().filter(|&c| c == '{').count();
            }
            line_idx += 1;
            continue;
        }

        // 2. Struct
        if trimmed.contains("struct ")
            && let Some(rest) =
                strip_keywords(trimmed, &["pub ", "pub(crate) ", "pub(super) ", "struct "])
        {
            let name = rest
                .split(&['<', '{', ';', '('][..])
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(lines, line_idx).unwrap_or(line_num);
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::Struct,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(lines, line_idx),
                );
                sym.is_public = is_pub;
                sym.doc_comment = doc;
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 3. Enum
        if trimmed.contains("enum ")
            && let Some(rest) =
                strip_keywords(trimmed, &["pub ", "pub(crate) ", "pub(super) ", "enum "])
        {
            let name = rest.split(&['<', '{', ';'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(lines, line_idx).unwrap_or(line_num);
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::Enum,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(lines, line_idx),
                );
                sym.is_public = is_pub;
                sym.doc_comment = doc;
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 4. Trait
        if trimmed.contains("trait ")
            && let Some(rest) = strip_keywords(trimmed, &["pub ", "pub(crate) ", "trait "])
        {
            let name = rest
                .split(&['<', '{', ':', ';'][..])
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(lines, line_idx).unwrap_or(line_num);
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::Trait,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(lines, line_idx),
                );
                sym.is_public = is_pub;
                sym.doc_comment = doc;
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 5. Function or Method
        if trimmed.contains("fn ")
            && let Some(rest) = strip_fn_prefix(trimmed)
            && let Some((name_part, _)) = rest.split_once('(')
        {
            let name = name_part
                .split_whitespace()
                .last()
                .unwrap_or(name_part.trim());
            if !name.is_empty() {
                let end_line = find_closing_brace(lines, line_idx).unwrap_or(line_num);
                let kind = if current_impl_target.is_some() {
                    AstSymbolKind::Method
                } else {
                    AstSymbolKind::Function
                };
                let mut sym = AstSymbol::new(
                    name,
                    kind,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(lines, line_idx),
                );
                sym.is_public = is_pub;
                sym.doc_comment = doc;
                sym.parent_scope = current_impl_target.clone();
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 6. Type alias
        if (trimmed.starts_with("type ") || trimmed.starts_with("pub type "))
            && let Some(rest) = strip_keywords(trimmed, &["pub ", "pub(crate) ", "type "])
        {
            let name = rest.split(&['<', '=', ';'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::TypeAlias,
                    path.to_path_buf(),
                    SourceRange::new(line_num, line_num, 0, raw_line.len()),
                    trimmed.to_string(),
                );
                sym.is_public = is_pub;
                sym.doc_comment = doc;
                symbols.push(sym);
            }
        }

        line_idx += 1;
    }

    symbols
}

// ---------------------------------------------------------------------------
// TypeScript / JavaScript Parser
// ---------------------------------------------------------------------------

fn parse_typescript(path: &Path, content: &str) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_class: Option<String> = None;
    let mut class_brace_depth = 0;

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let raw_line = lines[line_idx];
        let trimmed = raw_line.trim();
        let line_num = line_idx + 1;

        if current_class.is_some() {
            let opens = raw_line.chars().filter(|&c| c == '{').count();
            let closes = raw_line.chars().filter(|&c| c == '}').count();
            class_brace_depth += opens;
            if class_brace_depth >= closes {
                class_brace_depth -= closes;
            } else {
                class_brace_depth = 0;
            }
            if class_brace_depth == 0 {
                current_class = None;
            }
        }

        let is_export = trimmed.starts_with("export ");

        // 1. Class
        if trimmed.contains("class ")
            && let Some(rest) =
                strip_keywords(trimmed, &["export ", "default ", "abstract ", "class "])
        {
            let name = rest.split(&['<', '{', ' '][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                current_class = Some(name.to_string());
                class_brace_depth = raw_line.chars().filter(|&c| c == '{').count();

                symbols.push(AstSymbol::new(
                    name,
                    AstSymbolKind::Class,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                ));
                line_idx += 1;
                continue;
            }
        }

        // 2. Interface
        if trimmed.contains("interface ")
            && let Some(rest) = strip_keywords(trimmed, &["export ", "interface "])
        {
            let name = rest.split(&['<', '{', ' '][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                symbols.push(AstSymbol::new(
                    name,
                    AstSymbolKind::Interface,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                ));
                line_idx = end_line;
                continue;
            }
        }

        // 3. Type alias
        if trimmed.contains("type ")
            && trimmed.contains('=')
            && let Some(rest) = strip_keywords(trimmed, &["export ", "type "])
        {
            let name = rest.split(&['<', '=', ' '][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(AstSymbol::new(
                    name,
                    AstSymbolKind::TypeAlias,
                    path.to_path_buf(),
                    SourceRange::new(line_num, line_num, 0, raw_line.len()),
                    trimmed.to_string(),
                ));
            }
        }

        // 4. Function
        if trimmed.contains("function ")
            && let Some(rest) = strip_keywords(
                trimmed,
                &["export ", "default ", "async ", "function ", "*"],
            )
            && let Some((name_part, _)) = rest.split_once('(')
        {
            let name = name_part.trim().split('<').next().unwrap_or("").trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::Function,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                );
                sym.is_public = is_export;
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 5. Arrow function: `const foo = (...) =>`
        if (trimmed.starts_with("const ") || trimmed.starts_with("export const "))
            && trimmed.contains("=>")
            && let Some(rest) = strip_keywords(trimmed, &["export ", "const "])
            && let Some((name_part, _)) = rest.split_once('=')
        {
            let name = name_part.trim().split(':').next().unwrap_or("").trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::Function,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                );
                sym.is_public = is_export;
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 6. Method inside class
        if current_class.is_some()
            && trimmed.contains('(')
            && (trimmed.ends_with('{') || trimmed.contains(") {"))
        {
            let mut clean = trimmed;
            let modifiers = [
                "public ",
                "private ",
                "protected ",
                "static ",
                "async ",
                "override ",
                "readonly ",
            ];
            let mut changed = true;
            while changed {
                changed = false;
                for &m in &modifiers {
                    if let Some(rest) = clean.strip_prefix(m) {
                        clean = rest.trim_start();
                        changed = true;
                    }
                }
            }

            if let Some((name_part, _)) = clean.split_once('(') {
                let name = name_part.trim();
                if !name.is_empty()
                    && !name.contains(' ')
                    && name != "if"
                    && name != "for"
                    && name != "switch"
                {
                    let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Method,
                        path.to_path_buf(),
                        SourceRange::new(line_num, end_line, 0, raw_line.len()),
                        extract_signature(&lines, line_idx),
                    );
                    sym.parent_scope = current_class.clone();
                    symbols.push(sym);
                    line_idx = end_line;
                    continue;
                }
            }
        }

        line_idx += 1;
    }

    symbols
}

// ---------------------------------------------------------------------------
// Python Parser
// ---------------------------------------------------------------------------

fn parse_python(path: &Path, content: &str) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_class: Option<(String, usize)> = None; // (name, indent)

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let raw_line = lines[line_idx];
        let trimmed = raw_line.trim();
        let indent = raw_line.len() - raw_line.trim_start().len();
        let line_num = line_idx + 1;

        if trimmed.is_empty() || trimmed.starts_with('#') {
            line_idx += 1;
            continue;
        }

        // Exit class scope if indent dropped back
        if let Some((_, class_indent)) = current_class
            && indent <= class_indent
        {
            current_class = None;
        }

        // 1. Class
        if trimmed.starts_with("class ") {
            let rest = trimmed.strip_prefix("class ").unwrap_or("").trim();
            let name = rest.split(&['(', ':'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                let end_line = find_python_block_end(&lines, line_idx, indent);
                current_class = Some((name.to_string(), indent));

                symbols.push(AstSymbol::new(
                    name,
                    AstSymbolKind::Class,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, indent, raw_line.len()),
                    raw_line.trim_end().to_string(),
                ));
                line_idx += 1;
                continue;
            }
        }

        // 2. Function / Method: `def` or `async def`
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            let rest = trimmed
                .strip_prefix("async def ")
                .or_else(|| trimmed.strip_prefix("def "))
                .unwrap_or("")
                .trim();

            if let Some((name_part, _)) = rest.split_once('(') {
                let name = name_part.trim();
                if !name.is_empty() {
                    let end_line = find_python_block_end(&lines, line_idx, indent);
                    let (kind, parent) = if let Some((ref c_name, _)) = current_class {
                        (AstSymbolKind::Method, Some(c_name.clone()))
                    } else {
                        (AstSymbolKind::Function, None)
                    };

                    let mut sym = AstSymbol::new(
                        name,
                        kind,
                        path.to_path_buf(),
                        SourceRange::new(line_num, end_line, indent, raw_line.len()),
                        extract_python_signature(&lines, line_idx),
                    );
                    sym.parent_scope = parent;
                    sym.is_public = !name.starts_with('_') || name.starts_with("__init__");
                    symbols.push(sym);
                    line_idx = end_line;
                    continue;
                }
            }
        }

        line_idx += 1;
    }

    symbols
}

// ---------------------------------------------------------------------------
// Go Parser
// ---------------------------------------------------------------------------

fn parse_go(path: &Path, content: &str) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let raw_line = lines[line_idx];
        let trimmed = raw_line.trim();
        let line_num = line_idx + 1;

        // 1. Struct / Interface type
        if trimmed.starts_with("type ") {
            let rest = trimmed.strip_prefix("type ").unwrap_or("").trim();
            let mut parts = rest.split_whitespace();
            if let Some(name) = parts.next()
                && let Some(type_kind) = parts.next()
            {
                let (kind, end_line) = match type_kind {
                    "struct" => (
                        AstSymbolKind::Struct,
                        find_closing_brace(&lines, line_idx).unwrap_or(line_num),
                    ),
                    "interface" => (
                        AstSymbolKind::Interface,
                        find_closing_brace(&lines, line_idx).unwrap_or(line_num),
                    ),
                    _ => (AstSymbolKind::TypeAlias, line_num),
                };

                let mut sym = AstSymbol::new(
                    name,
                    kind,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                );
                sym.is_public = name
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        // 2. Func or Receiver method
        if trimmed.starts_with("func ") {
            let rest = trimmed.strip_prefix("func ").unwrap_or("").trim();
            let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);

            if rest.starts_with('(') {
                // Method with receiver: `func (s *Server) Start() error`
                if let Some((_, after_recv)) = rest.split_once(')')
                    && let Some((name_part, _)) = after_recv.trim().split_once('(')
                {
                    let name = name_part.trim();
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Method,
                        path.to_path_buf(),
                        SourceRange::new(line_num, end_line, 0, raw_line.len()),
                        extract_signature(&lines, line_idx),
                    );
                    sym.is_public = name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);
                    symbols.push(sym);
                    line_idx = end_line;
                    continue;
                }
            } else if let Some((name_part, _)) = rest.split_once('(') {
                let name = name_part.trim();
                let mut sym = AstSymbol::new(
                    name,
                    AstSymbolKind::Function,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                );
                sym.is_public = name
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                symbols.push(sym);
                line_idx = end_line;
                continue;
            }
        }

        line_idx += 1;
    }

    symbols
}

// ---------------------------------------------------------------------------
// C / C++ / Java / C# Parser
// ---------------------------------------------------------------------------

fn parse_c_like(path: &Path, content: &str) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_class: Option<String> = None;
    let mut class_brace_depth = 0;

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let raw_line = lines[line_idx];
        let trimmed = raw_line.trim();
        let line_num = line_idx + 1;

        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            line_idx += 1;
            continue;
        }

        if current_class.is_some() {
            let opens = raw_line.chars().filter(|&c| c == '{').count();
            let closes = raw_line.chars().filter(|&c| c == '}').count();
            class_brace_depth += opens;
            if class_brace_depth >= closes {
                class_brace_depth -= closes;
            } else {
                class_brace_depth = 0;
            }
            if class_brace_depth == 0 {
                current_class = None;
            }
        }

        let is_public = trimmed.starts_with("public ") || trimmed.starts_with("export ");

        // 1. Enum: `enum Color`, `enum class Status` (check before class so `enum class` is an enum)
        if trimmed.contains("enum ")
            && !trimmed.contains(";")
            && let Some((_, after_enum)) = trimmed.split_once("enum ")
        {
            let clean = after_enum
                .strip_prefix("class ")
                .or_else(|| after_enum.strip_prefix("struct "))
                .unwrap_or(after_enum)
                .trim();
            let name = clean
                .split(&[':', '{', ' '][..])
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                symbols.push(AstSymbol::new(
                    name,
                    AstSymbolKind::Enum,
                    path.to_path_buf(),
                    SourceRange::new(line_num, end_line, 0, raw_line.len()),
                    extract_signature(&lines, line_idx),
                ));
                line_idx = end_line;
                continue;
            }
        }

        // 2. Class or Struct: `class Foo`, `struct Bar`
        if (trimmed.contains("class ")
            || trimmed.contains("struct ")
            || trimmed.contains("interface "))
            && !trimmed.contains(";")
        {
            let (kind_str, kind) = if trimmed.contains("class ") {
                ("class ", AstSymbolKind::Class)
            } else if trimmed.contains("interface ") {
                ("interface ", AstSymbolKind::Interface)
            } else {
                ("struct ", AstSymbolKind::Struct)
            };

            if let Some((_, after_kw)) = trimmed.split_once(kind_str) {
                let name = after_kw
                    .split(&[':', '<', '{', ' ', '('][..])
                    .next()
                    .unwrap_or("")
                    .trim();

                if !name.is_empty() && !name.contains(';') {
                    let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                    current_class = Some(name.to_string());
                    class_brace_depth = raw_line.chars().filter(|&c| c == '{').count();

                    symbols.push(AstSymbol::new(
                        name,
                        kind,
                        path.to_path_buf(),
                        SourceRange::new(line_num, end_line, 0, raw_line.len()),
                        extract_signature(&lines, line_idx),
                    ));
                    line_idx += 1;
                    continue;
                }
            }
        }

        // 3. Function / Method
        if trimmed.contains('(')
            && (trimmed.contains(") {") || trimmed.ends_with('{') || trimmed.ends_with(')'))
            && !trimmed.starts_with("if ")
            && !trimmed.starts_with("if(")
            && !trimmed.starts_with("for ")
            && !trimmed.starts_with("for(")
            && !trimmed.starts_with("while ")
            && !trimmed.starts_with("while(")
            && !trimmed.starts_with("switch ")
            && !trimmed.starts_with("catch ")
            && let Some((before_paren, _)) = trimmed.split_once('(')
        {
            let tokens: Vec<&str> = before_paren.split_whitespace().collect();
            if let Some(&name_candidate) = tokens.last() {
                let clean_name = name_candidate
                    .trim_start_matches('*')
                    .trim_start_matches('&');
                let clean_name = clean_name.split("::").last().unwrap_or(clean_name);

                if !clean_name.is_empty()
                    && clean_name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    let end_line = find_closing_brace(&lines, line_idx).unwrap_or(line_num);
                    let kind = if current_class.is_some() {
                        AstSymbolKind::Method
                    } else {
                        AstSymbolKind::Function
                    };

                    let mut sym = AstSymbol::new(
                        clean_name,
                        kind,
                        path.to_path_buf(),
                        SourceRange::new(line_num, end_line, 0, raw_line.len()),
                        extract_signature(&lines, line_idx),
                    );
                    sym.is_public = is_public;
                    sym.parent_scope = current_class.clone();
                    symbols.push(sym);
                    line_idx = end_line;
                    continue;
                }
            }
        }

        line_idx += 1;
    }

    symbols
}

// ---------------------------------------------------------------------------
// Generic fallback
// ---------------------------------------------------------------------------

fn parse_generic(_path: &Path, _content: &str) -> Vec<AstSymbol> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn strip_keywords<'a>(mut s: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    for &p in prefixes {
        s = s.trim_start_matches(p).trim();
    }
    if !s.is_empty() { Some(s) } else { None }
}

fn strip_fn_prefix(line: &str) -> Option<&str> {
    let mut s = line;
    for prefix in &[
        "pub ",
        "pub(crate) ",
        "pub(super) ",
        "async ",
        "const ",
        "unsafe ",
        "extern \"C\" ",
    ] {
        s = s.trim_start_matches(prefix).trim();
    }
    s.strip_prefix("fn ")
}

fn find_closing_brace(lines: &[&str], start_idx: usize) -> Option<usize> {
    let mut depth: isize = 0;
    let mut found_open = false;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut in_block_comment = false;
    let mut escaped = false;

    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            let next_ch = chars.get(i + 1).copied();

            if in_block_comment {
                if ch == '*' && next_ch == Some('/') {
                    in_block_comment = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }

            if in_single_quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '\'' {
                    in_single_quote = false;
                }
                i += 1;
                continue;
            }

            if in_double_quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_double_quote = false;
                }
                i += 1;
                continue;
            }

            if in_backtick {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '`' {
                    in_backtick = false;
                }
                i += 1;
                continue;
            }

            // Not inside literal or comment:
            if ch == '/' && next_ch == Some('/') {
                break; // Line comment ends processing for this line
            }
            if ch == '/' && next_ch == Some('*') {
                in_block_comment = true;
                i += 2;
                continue;
            }

            if ch == '\'' {
                in_single_quote = true;
                escaped = false;
            } else if ch == '"' {
                in_double_quote = true;
                escaped = false;
            } else if ch == '`' {
                in_backtick = true;
                escaped = false;
            } else if ch == '{' {
                depth += 1;
                found_open = true;
            } else if ch == '}' {
                depth -= 1;
                if found_open && depth <= 0 {
                    return Some(idx + 1);
                }
            }
            i += 1;
        }
    }

    None
}

fn find_python_block_end(lines: &[&str], start_idx: usize, base_indent: usize) -> usize {
    let mut last_code_line = start_idx + 1;

    for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        if indent <= base_indent {
            return last_code_line;
        }
        last_code_line = idx + 1;
    }

    last_code_line
}

fn extract_signature(lines: &[&str], start_idx: usize) -> String {
    let mut sig = String::new();
    for line in lines.iter().skip(start_idx) {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.find('{') {
            sig.push_str(&trimmed[..idx]);
            break;
        }
        sig.push_str(trimmed);
        sig.push(' ');
        if sig.len() > 300 || trimmed.ends_with(';') {
            break;
        }
    }
    sig.trim().to_string()
}

fn extract_python_signature(lines: &[&str], start_idx: usize) -> String {
    let mut sig = String::new();
    for line in lines.iter().skip(start_idx) {
        let trimmed = line.trim();
        sig.push_str(trimmed);
        if trimmed.ends_with(':') {
            break;
        }
        sig.push(' ');
        if sig.len() > 300 {
            break;
        }
    }
    sig.trim().trim_end_matches(':').trim().to_string()
}
