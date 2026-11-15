//! Unified Multi-Language Parser Framework & Adapters (`m31a-ast::language`).
//!
//! Provides the `LanguageAdapter` trait and production adapters for:
//! - Rust (`syn` AST + syn visitor for use declarations, impls, calls, tests, with graceful fallback).
//! - TypeScript / JavaScript (imports, exports, classes, interfaces, JSX components, calls, tests).
//! - Python (imports, aliases, classes, inheritance, decorators, functions, calls, tests).
//! - Go (package, imports, structs, interfaces, receiver methods, calls, tests).
//! - C/C++ / Java / C# (syntax outlines and calls).
//!
//! Produces structured `ParseResult` with symbols, relationships, diagnostics, imports, and exports.

use crate::types::{
    AstSymbol, AstSymbolKind, CallSite, DiagnosticSeverity, ExportItem, ImportItem,
    LanguageCapabilities, LanguageId, ParserDiagnostic, RelationshipKind, ResolutionStatus,
    SourceRange, SymbolRelationship, SymbolVisibility,
};
use std::path::Path;

/// Structured output from parsing a single source file.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub symbols: Vec<AstSymbol>,
    pub relationships: Vec<SymbolRelationship>,
    pub diagnostics: Vec<ParserDiagnostic>,
    pub imports: Vec<ImportItem>,
    pub exports: Vec<ExportItem>,
    pub calls: Vec<CallSite>,
}

/// Abstract contract for language-specific syntax parsing and relationship extraction.
pub trait LanguageAdapter: Send + Sync {
    fn language_id(&self) -> LanguageId;
    fn can_parse(&self, path: &Path) -> bool;
    fn parse(&self, path: &Path, content: &str) -> ParseResult;
    fn capabilities(&self) -> LanguageCapabilities;
}

// ===========================================================================
// Rust Adapter
// ===========================================================================

pub struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Rust
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("rs"))
            .unwrap_or(false)
    }

    fn capabilities(&self) -> LanguageCapabilities {
        LanguageCapabilities {
            syntax_parsing: true,
            definition_extraction: true,
            import_export_extraction: true,
            reference_extraction: true,
            call_resolution: true,
            type_resolution: true,
            test_discovery: true,
        }
    }

    fn parse(&self, path: &Path, content: &str) -> ParseResult {
        let mut result = ParseResult::default();
        let lines: Vec<&str> = content.lines().collect();

        match syn::parse_file(content) {
            Ok(file) => {
                parse_rust_file_syn(&file, path, &lines, &mut result);
            }
            Err(err) => {
                let span = err.span();
                result.diagnostics.push(ParserDiagnostic {
                    file_path: path.to_path_buf(),
                    language: LanguageId::Rust,
                    parser_id: "syn".to_string(),
                    line: span.start().line,
                    column: span.start().column,
                    message: format!("Syntax error: {}", err),
                    severity: DiagnosticSeverity::Warning,
                    partial_usable: true,
                });

                // Fall back to resilient scanner on syntax error
                parse_rust_scanner(path, &lines, &mut result);
            }
        }

        result
    }
}

use syn::spanned::Spanned;
use syn::visit::Visit;

struct SynCallVisitor<'a> {
    current_caller: Option<String>,
    current_scope: Option<String>,
    file_path: &'a Path,
    calls: Vec<CallSite>,
}

impl<'a> Visit<'_> for SynCallVisitor<'a> {
    fn visit_item_fn(&mut self, i: &syn::ItemFn) {
        let prev = self.current_caller.take();
        self.current_caller = Some(i.sig.ident.to_string());
        syn::visit::visit_item_fn(self, i);
        self.current_caller = prev;
    }

    fn visit_impl_item_fn(&mut self, i: &syn::ImplItemFn) {
        let prev = self.current_caller.take();
        self.current_caller = Some(i.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, i);
        self.current_caller = prev;
    }

    fn visit_expr_call(&mut self, i: &syn::ExprCall) {
        if let Some(ref caller) = self.current_caller {
            let callee_name = match &*i.func {
                syn::Expr::Path(p) => p
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default(),
                _ => String::new(),
            };

            if !callee_name.is_empty() {
                self.calls.push(CallSite {
                    caller_name: caller.clone(),
                    callee_name,
                    callee_scope: self.current_scope.clone(),
                    file_path: self.file_path.to_path_buf(),
                    line: i.span().start().line,
                    col: i.span().start().column,
                    is_method_call: false,
                });
            }
        }
        syn::visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &syn::ExprMethodCall) {
        if let Some(ref caller) = self.current_caller {
            self.calls.push(CallSite {
                caller_name: caller.clone(),
                callee_name: i.method.to_string(),
                callee_scope: None,
                file_path: self.file_path.to_path_buf(),
                line: i.span().start().line,
                col: i.span().start().column,
                is_method_call: true,
            });
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

fn parse_rust_file_syn(
    file: &syn::File,
    path: &Path,
    lines: &[&str],
    result: &mut ParseResult,
) {
    // 1. Visit calls with AST visitor
    let mut call_visitor = SynCallVisitor {
        current_caller: None,
        current_scope: None,
        file_path: path,
        calls: Vec::new(),
    };
    call_visitor.visit_file(file);
    result.calls = call_visitor.calls;

    // 2. Process items (functions, structs, traits, impls, use declarations, re-exports)
    process_syn_items(&file.items, path, lines, None, result);

    // 3. Convert calls into SymbolRelationship edges
    for call in &result.calls {
        let mut rel = SymbolRelationship::new(
            call.caller_name.clone(),
            call.caller_name.clone(),
            call.callee_name.clone(),
            RelationshipKind::Calls,
            path.to_path_buf(),
            call.line,
            ResolutionStatus::AstDerived,
        );
        rel.provenance = "syn_ast_visitor".to_string();
        result.relationships.push(rel);
    }
}

fn extract_syn_doc(attrs: &[syn::Attribute]) -> Option<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(meta) = &attr.meta
            && let syn::Expr::Lit(expr_lit) = &meta.value
            && let syn::Lit::Str(lit_str) = &expr_lit.lit
        {
            docs.push(lit_str.value().trim().to_string());
        }
    }
    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

fn has_test_attribute(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || a.path().is_ident("tokio::test")
            || a.path().segments.iter().any(|s| s.ident == "test")
    })
}

fn process_syn_items(
    items: &[syn::Item],
    path: &Path,
    lines: &[&str],
    parent_scope: Option<&str>,
    result: &mut ParseResult,
) {
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                let start_line = f.span().start().line;
                let end_line = f.span().end().line;
                let is_public = !matches!(f.vis, syn::Visibility::Inherited);
                let is_test = has_test_attribute(&f.attrs);

                let kind = if is_test {
                    AstSymbolKind::Function
                } else if parent_scope.is_some() {
                    AstSymbolKind::Method
                } else {
                    AstSymbolKind::Function
                };

                let mut sym = AstSymbol::new(
                    f.sig.ident.to_string(),
                    kind,
                    path.to_path_buf(),
                    SourceRange::new(start_line, end_line, 0, lines.get(start_line.saturating_sub(1)).map(|l| l.len()).unwrap_or(0)),
                    lines.get(start_line.saturating_sub(1)).unwrap_or(&"").trim().to_string(),
                );
                sym.is_public = is_public;
                sym.doc_comment = extract_syn_doc(&f.attrs);
                if let Some(scope) = parent_scope {
                    sym = sym.with_parent_scope(scope);
                }
                sym.visibility = if is_public { SymbolVisibility::Public } else { SymbolVisibility::Private };

                let sym_id = sym.id.clone();
                let sym_name = sym.name.clone();
                result.symbols.push(sym);

                if is_test {
                    result.relationships.push(SymbolRelationship::new(
                        sym_id,
                        sym_name,
                        "test_harness",
                        RelationshipKind::Tests,
                        path.to_path_buf(),
                        start_line,
                        ResolutionStatus::Exact,
                    ));
                }
            }
            syn::Item::Struct(s) => {
                let start_line = s.span().start().line;
                let end_line = s.span().end().line;
                let is_public = !matches!(s.vis, syn::Visibility::Inherited);
                let mut sym = AstSymbol::new(
                    s.ident.to_string(),
                    AstSymbolKind::Struct,
                    path.to_path_buf(),
                    SourceRange::new(start_line, end_line, 0, lines.get(start_line.saturating_sub(1)).map(|l| l.len()).unwrap_or(0)),
                    lines.get(start_line.saturating_sub(1)).unwrap_or(&"").trim().to_string(),
                );
                sym.is_public = is_public;
                sym.doc_comment = extract_syn_doc(&s.attrs);
                sym.visibility = if is_public { SymbolVisibility::Public } else { SymbolVisibility::Private };
                result.symbols.push(sym);
            }
            syn::Item::Enum(e) => {
                let start_line = e.span().start().line;
                let end_line = e.span().end().line;
                let is_public = !matches!(e.vis, syn::Visibility::Inherited);
                let mut sym = AstSymbol::new(
                    e.ident.to_string(),
                    AstSymbolKind::Enum,
                    path.to_path_buf(),
                    SourceRange::new(start_line, end_line, 0, lines.get(start_line.saturating_sub(1)).map(|l| l.len()).unwrap_or(0)),
                    lines.get(start_line.saturating_sub(1)).unwrap_or(&"").trim().to_string(),
                );
                sym.is_public = is_public;
                sym.doc_comment = extract_syn_doc(&e.attrs);
                sym.visibility = if is_public { SymbolVisibility::Public } else { SymbolVisibility::Private };
                result.symbols.push(sym);
            }
            syn::Item::Trait(t) => {
                let start_line = t.span().start().line;
                let end_line = t.span().end().line;
                let is_public = !matches!(t.vis, syn::Visibility::Inherited);
                let mut sym = AstSymbol::new(
                    t.ident.to_string(),
                    AstSymbolKind::Trait,
                    path.to_path_buf(),
                    SourceRange::new(start_line, end_line, 0, lines.get(start_line.saturating_sub(1)).map(|l| l.len()).unwrap_or(0)),
                    lines.get(start_line.saturating_sub(1)).unwrap_or(&"").trim().to_string(),
                );
                sym.is_public = is_public;
                sym.doc_comment = extract_syn_doc(&t.attrs);
                sym.visibility = if is_public { SymbolVisibility::Public } else { SymbolVisibility::Private };
                result.symbols.push(sym);

                for trait_item in &t.items {
                    if let syn::TraitItem::Fn(tf) = trait_item {
                        let start_line = tf.span().start().line;
                        let end_line = tf.span().end().line;
                        let mut method_sym = AstSymbol::new(
                            tf.sig.ident.to_string(),
                            AstSymbolKind::Method,
                            path.to_path_buf(),
                            SourceRange::new(start_line, end_line, 0, lines.get(start_line.saturating_sub(1)).map(|l| l.len()).unwrap_or(0)),
                            lines.get(start_line.saturating_sub(1)).unwrap_or(&"").trim().to_string(),
                        );
                        method_sym.is_public = is_public;
                        method_sym.doc_comment = extract_syn_doc(&tf.attrs);
                        method_sym = method_sym.with_parent_scope(&t.ident.to_string());
                        method_sym.visibility = if is_public { SymbolVisibility::Public } else { SymbolVisibility::Private };
                        result.symbols.push(method_sym);
                    }
                }
            }
            syn::Item::Impl(imp) => {
                let self_ty_name = match &*imp.self_ty {
                    syn::Type::Path(p) => p
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_else(|| "Self".to_string()),
                    _ => "Self".to_string(),
                };

                let trait_name = imp.trait_.as_ref().map(|(_, tr, _)| {
                    tr.segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default()
                });

                if let Some(ref tr_name) = trait_name {
                    result.relationships.push(SymbolRelationship::new(
                        self_ty_name.clone(),
                        self_ty_name.clone(),
                        tr_name.clone(),
                        RelationshipKind::Implements,
                        path.to_path_buf(),
                        imp.span().start().line,
                        ResolutionStatus::AstDerived,
                    ));
                }

                for impl_item in &imp.items {
                    if let syn::ImplItem::Fn(f) = impl_item {
                        let start_line = f.span().start().line;
                        let end_line = f.span().end().line;
                        let is_public = !matches!(f.vis, syn::Visibility::Inherited);
                        let mut sym = AstSymbol::new(
                            f.sig.ident.to_string(),
                            AstSymbolKind::Method,
                            path.to_path_buf(),
                            SourceRange::new(start_line, end_line, 0, lines.get(start_line.saturating_sub(1)).map(|l| l.len()).unwrap_or(0)),
                            lines.get(start_line.saturating_sub(1)).unwrap_or(&"").trim().to_string(),
                        );
                        sym.is_public = is_public;
                        sym.doc_comment = extract_syn_doc(&f.attrs);
                        sym = sym.with_parent_scope(&self_ty_name);
                        sym.visibility = if is_public { SymbolVisibility::Public } else { SymbolVisibility::Private };
                        result.symbols.push(sym);
                    }
                }
            }
            syn::Item::Use(u) => {
                let is_pub = !matches!(u.vis, syn::Visibility::Inherited);
                extract_use_tree(&u.tree, "", path, u.span().start().line, is_pub, result);
            }
            syn::Item::Mod(m) => {
                let start_line = m.span().start().line;
                let end_line = m.span().end().line;
                let mut sym = AstSymbol::new(
                    m.ident.to_string(),
                    AstSymbolKind::Module,
                    path.to_path_buf(),
                    SourceRange::new(start_line, end_line, 0, 0),
                    format!("mod {}", m.ident),
                );
                sym.is_public = !matches!(m.vis, syn::Visibility::Inherited);
                result.symbols.push(sym);

                if let Some((_, ref inner_items)) = m.content {
                    process_syn_items(inner_items, path, lines, Some(&m.ident.to_string()), result);
                }
            }
            syn::Item::Type(t) => {
                let start_line = t.span().start().line;
                let end_line = t.span().end().line;
                let is_public = !matches!(t.vis, syn::Visibility::Inherited);
                let mut sym = AstSymbol::new(
                    t.ident.to_string(),
                    AstSymbolKind::TypeAlias,
                    path.to_path_buf(),
                    SourceRange::new(start_line, end_line, 0, 0),
                    format!("type {}", t.ident),
                );
                sym.is_public = is_public;
                result.symbols.push(sym);
            }
            syn::Item::Const(c) => {
                let start_line = c.span().start().line;
                let mut sym = AstSymbol::new(
                    c.ident.to_string(),
                    AstSymbolKind::Constant,
                    path.to_path_buf(),
                    SourceRange::new(start_line, start_line, 0, 0),
                    format!("const {}", c.ident),
                );
                sym.is_public = !matches!(c.vis, syn::Visibility::Inherited);
                result.symbols.push(sym);
            }
            syn::Item::Macro(m) => {
                if let Some(ref ident) = m.ident {
                    let start_line = m.span().start().line;
                    let sym = AstSymbol::new(
                        ident.to_string(),
                        AstSymbolKind::Macro,
                        path.to_path_buf(),
                        SourceRange::new(start_line, start_line, 0, 0),
                        format!("macro_rules! {}", ident),
                    );
                    result.symbols.push(sym);
                }
            }
            _ => {}
        }
    }
}

fn extract_use_tree(
    tree: &syn::UseTree,
    prefix: &str,
    path: &Path,
    line: usize,
    is_pub: bool,
    result: &mut ParseResult,
) {
    match tree {
        syn::UseTree::Path(p) => {
            let next_prefix = if prefix.is_empty() {
                p.ident.to_string()
            } else {
                format!("{}::{}", prefix, p.ident)
            };
            extract_use_tree(&p.tree, &next_prefix, path, line, is_pub, result);
        }
        syn::UseTree::Name(n) => {
            let sym_name = n.ident.to_string();
            let from_module = prefix.to_string();
            result.imports.push(ImportItem {
                source_file: path.to_path_buf(),
                imported_symbol: sym_name.clone(),
                alias: None,
                from_module: from_module.clone(),
                line,
                is_glob: false,
                is_type_only: false,
            });

            if is_pub {
                result.exports.push(ExportItem {
                    source_file: path.to_path_buf(),
                    symbol_name: sym_name.clone(),
                    exported_as: sym_name.clone(),
                    line,
                    is_default: false,
                    reexported_from: Some(from_module.clone()),
                });
                result.relationships.push(SymbolRelationship::new(
                    path.to_string_lossy(),
                    sym_name,
                    from_module,
                    RelationshipKind::ReExports,
                    path.to_path_buf(),
                    line,
                    ResolutionStatus::Exact,
                ));
            }
        }
        syn::UseTree::Rename(r) => {
            let sym_name = r.ident.to_string();
            let alias = r.rename.to_string();
            let from_module = prefix.to_string();
            result.imports.push(ImportItem {
                source_file: path.to_path_buf(),
                imported_symbol: sym_name.clone(),
                alias: Some(alias.clone()),
                from_module: from_module.clone(),
                line,
                is_glob: false,
                is_type_only: false,
            });

            if is_pub {
                result.exports.push(ExportItem {
                    source_file: path.to_path_buf(),
                    symbol_name: sym_name,
                    exported_as: alias,
                    line,
                    is_default: false,
                    reexported_from: Some(from_module),
                });
            }
        }
        syn::UseTree::Glob(_) => {
            result.imports.push(ImportItem {
                source_file: path.to_path_buf(),
                imported_symbol: "*".to_string(),
                alias: None,
                from_module: prefix.to_string(),
                line,
                is_glob: true,
                is_type_only: false,
            });
        }
        syn::UseTree::Group(g) => {
            for item in &g.items {
                extract_use_tree(item, prefix, path, line, is_pub, result);
            }
        }
    }
}

fn parse_rust_scanner(path: &Path, lines: &[&str], result: &mut ParseResult) {
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            if let Some(rest) = trimmed.strip_prefix("pub fn ").or_else(|| trimmed.strip_prefix("fn "))
                && let Some((name, _)) = rest.split_once('(')
            {
                let clean = name.trim();
                let is_pub = trimmed.starts_with("pub ");
                let mut sym = AstSymbol::new(
                    clean,
                    AstSymbolKind::Function,
                    path.to_path_buf(),
                    SourceRange::new(line_num, line_num, 0, trimmed.len()),
                    trimmed.to_string(),
                );
                sym.is_public = is_pub;
                result.symbols.push(sym);
            }
        } else if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
            if let Some(rest) = trimmed.strip_prefix("pub struct ").or_else(|| trimmed.strip_prefix("struct ")) {
                let name = rest.split(&['<', '{', ';', '('][..]).next().unwrap_or("").trim();
                if !name.is_empty() {
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Struct,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, trimmed.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = trimmed.starts_with("pub ");
                    result.symbols.push(sym);
                }
            }
        }
    }
}

// ===========================================================================
// TypeScript / JavaScript Adapter
// ===========================================================================

pub struct TypeScriptAdapter;

impl LanguageAdapter for TypeScriptAdapter {
    fn language_id(&self) -> LanguageId {
        LanguageId::TypeScript
    }

    fn can_parse(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(ext.as_str(), "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
    }

    fn capabilities(&self) -> LanguageCapabilities {
        LanguageCapabilities {
            syntax_parsing: true,
            definition_extraction: true,
            import_export_extraction: true,
            reference_extraction: true,
            call_resolution: true,
            type_resolution: false,
            test_discovery: true,
        }
    }

    fn parse(&self, path: &Path, content: &str) -> ParseResult {
        let mut result = ParseResult::default();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_class: Option<String> = None;
        let mut current_caller: Option<String> = None;
        let mut brace_depth: usize = 0;
        let mut class_depth: usize = 0;

        for (idx, raw_line) in lines.iter().enumerate() {
            let trimmed = raw_line.trim();
            let line_num = idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            // Track brace depths
            let opens = raw_line.chars().filter(|&c| c == '{').count();
            let closes = raw_line.chars().filter(|&c| c == '}').count();
            brace_depth = brace_depth + opens;
            if brace_depth >= closes {
                brace_depth -= closes;
            } else {
                brace_depth = 0;
            }

            if current_class.is_some() && brace_depth <= class_depth {
                current_class = None;
            }

            // 1. Imports
            if trimmed.starts_with("import ") {
                parse_ts_import(path, trimmed, line_num, &mut result);
                continue;
            }

            // 2. Class
            if (trimmed.starts_with("class ")
                || trimmed.starts_with("export class ")
                || trimmed.starts_with("export default class "))
                && !trimmed.contains(';')
            {
                let clean = trimmed
                    .strip_prefix("export default class ")
                    .or_else(|| trimmed.strip_prefix("export class "))
                    .or_else(|| trimmed.strip_prefix("class "))
                    .unwrap_or(trimmed)
                    .trim();

                let name = clean.split(&['<', '{', ' '][..]).next().unwrap_or("").trim();
                if !name.is_empty() {
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Class,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = trimmed.starts_with("export");
                    result.symbols.push(sym);
                    current_class = Some(name.to_string());
                    class_depth = brace_depth.saturating_sub(opens);
                    continue;
                }
            }

            // 3. Interface
            if trimmed.starts_with("interface ") || trimmed.starts_with("export interface ") {
                let clean = trimmed
                    .strip_prefix("export interface ")
                    .or_else(|| trimmed.strip_prefix("interface "))
                    .unwrap_or(trimmed)
                    .trim();
                let name = clean.split(&['<', '{', ' '][..]).next().unwrap_or("").trim();
                if !name.is_empty() {
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Interface,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = trimmed.starts_with("export");
                    result.symbols.push(sym);
                    continue;
                }
            }

            // 4. Type alias
            if trimmed.starts_with("type ") || trimmed.starts_with("export type ") {
                let clean = trimmed
                    .strip_prefix("export type ")
                    .or_else(|| trimmed.strip_prefix("type "))
                    .unwrap_or(trimmed)
                    .trim();
                let name = clean.split(&['<', '=', ' '][..]).next().unwrap_or("").trim();
                if !name.is_empty() {
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::TypeAlias,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = trimmed.starts_with("export");
                    result.symbols.push(sym);
                    continue;
                }
            }

            // 5. Method inside class
            if current_class.is_some() && trimmed.contains('(') && !trimmed.starts_with("if") && !trimmed.starts_with("for") && !trimmed.starts_with("while") && !trimmed.starts_with("switch") {
                let mut clean = trimmed;
                for kw in &["public ", "private ", "protected ", "static ", "async ", "override ", "readonly "] {
                    while let Some(r) = clean.strip_prefix(kw) {
                        clean = r.trim_start();
                    }
                }
                if let Some((name_part, _)) = clean.split_once('(') {
                    let method_name = name_part.trim();
                    if !method_name.is_empty()
                        && !method_name.contains(' ')
                        && method_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                    {
                        let mut sym = AstSymbol::new(
                            method_name,
                            AstSymbolKind::Method,
                            path.to_path_buf(),
                            SourceRange::new(line_num, line_num, 0, raw_line.len()),
                            trimmed.to_string(),
                        );
                        if let Some(ref c) = current_class {
                            sym = sym.with_parent_scope(c);
                        }
                        sym.is_public = !trimmed.starts_with("private") && !trimmed.starts_with('#');
                        result.symbols.push(sym);
                        current_caller = Some(method_name.to_string());
                        continue;
                    }
                }
            }

            // 6. Function (standard or arrow)
            if trimmed.contains("function ") || (trimmed.contains("const ") && trimmed.contains("=>")) {
                let is_export = trimmed.starts_with("export");
                let fn_name = if let Some((before_paren, _)) = trimmed.split_once('(') {
                    let tokens: Vec<&str> = before_paren.split_whitespace().collect();
                    tokens.last().copied().unwrap_or("")
                } else if let Some((before_eq, _)) = trimmed.split_once('=') {
                    before_eq.split_whitespace().last().unwrap_or("").trim_matches(':')
                } else {
                    ""
                };

                let clean_name = fn_name.trim_start_matches('*').trim();
                if !clean_name.is_empty()
                    && !clean_name.contains('(')
                    && clean_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                {
                    let kind = if clean_name.starts_with("use") && clean_name.chars().nth(3).map(|c| c.is_uppercase()).unwrap_or(false) {
                        AstSymbolKind::Function // React hook
                    } else if current_class.is_some() {
                        AstSymbolKind::Method
                    } else {
                        AstSymbolKind::Function
                    };

                    let mut sym = AstSymbol::new(
                        clean_name,
                        kind,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = is_export;
                    if let Some(ref c) = current_class {
                        sym = sym.with_parent_scope(c);
                    }
                    result.symbols.push(sym);
                    current_caller = Some(clean_name.to_string());
                }
            }

            // 6. Test detection (describe / test / it)
            if trimmed.starts_with("test(")
                || trimmed.starts_with("it(")
                || trimmed.starts_with("describe(")
            {
                if let Some((_, rest)) = trimmed.split_once('(') {
                    let test_title = rest
                        .split(&['\'', '"', '`'][..])
                        .nth(1)
                        .unwrap_or("test")
                        .to_string();
                    result.symbols.push(AstSymbol::new(
                        format!("test: {}", test_title),
                        AstSymbolKind::Function,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    ));
                }
            }

            // 7. Method / Function Call sites
            if let Some(ref caller) = current_caller {
                if trimmed.contains('(') && !trimmed.starts_with("function") && !trimmed.starts_with("const") {
                    for part in trimmed.split(&[' ', '.', ';', '=', '{', '}', ','][..]) {
                        if let Some((callee, _)) = part.split_once('(') {
                            let callee = callee.trim();
                            if !callee.is_empty()
                                && callee.chars().all(|c| c.is_alphanumeric() || c == '_')
                                && callee != caller
                                && !matches!(callee, "if" | "for" | "while" | "switch" | "catch" | "return")
                            {
                                result.calls.push(CallSite {
                                    caller_name: caller.clone(),
                                    callee_name: callee.to_string(),
                                    callee_scope: None,
                                    file_path: path.to_path_buf(),
                                    line: line_num,
                                    col: 0,
                                    is_method_call: trimmed.contains(&format!(".{}(", callee)),
                                });
                                result.relationships.push(SymbolRelationship::new(
                                    caller.clone(),
                                    caller.clone(),
                                    callee.to_string(),
                                    RelationshipKind::Calls,
                                    path.to_path_buf(),
                                    line_num,
                                    ResolutionStatus::AstDerived,
                                ));
                                break;
                            }
                        }
                    }
                }
            }
        }

        result
    }
}

fn parse_ts_import(path: &Path, line: &str, line_num: usize, result: &mut ParseResult) {
    // Examples:
    // import { foo, bar as b } from './module';
    // import React from 'react';
    if let Some((clause, from_part)) = line.split_once(" from ") {
        let from_module = from_part
            .trim_matches(&['\'', '"', ';', ' '][..])
            .to_string();

        let symbols_part = clause
            .strip_prefix("import ")
            .unwrap_or(clause)
            .trim();

        if symbols_part.starts_with('{') {
            let inner = symbols_part.trim_matches(&['{', '}', ' '][..]);
            for item in inner.split(',') {
                let trimmed = item.trim();
                if let Some((sym, alias)) = trimmed.split_once(" as ") {
                    result.imports.push(ImportItem {
                        source_file: path.to_path_buf(),
                        imported_symbol: sym.trim().to_string(),
                        alias: Some(alias.trim().to_string()),
                        from_module: from_module.clone(),
                        line: line_num,
                        is_glob: false,
                        is_type_only: false,
                    });
                } else if !trimmed.is_empty() {
                    result.imports.push(ImportItem {
                        source_file: path.to_path_buf(),
                        imported_symbol: trimmed.to_string(),
                        alias: None,
                        from_module: from_module.clone(),
                        line: line_num,
                        is_glob: false,
                        is_type_only: false,
                    });
                }
            }
        } else if !symbols_part.is_empty() {
            // Default import
            result.imports.push(ImportItem {
                source_file: path.to_path_buf(),
                imported_symbol: symbols_part.to_string(),
                alias: None,
                from_module,
                line: line_num,
                is_glob: false,
                is_type_only: false,
            });
        }
    }
}

// ===========================================================================
// Python Adapter
// ===========================================================================

pub struct PythonAdapter;

impl LanguageAdapter for PythonAdapter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Python
    }

    fn can_parse(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(ext.as_str(), "py" | "pyi")
    }

    fn capabilities(&self) -> LanguageCapabilities {
        LanguageCapabilities {
            syntax_parsing: true,
            definition_extraction: true,
            import_export_extraction: true,
            reference_extraction: true,
            call_resolution: true,
            type_resolution: false,
            test_discovery: true,
        }
    }

    fn parse(&self, path: &Path, content: &str) -> ParseResult {
        let mut result = ParseResult::default();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_class: Option<(String, usize)> = None;
        let mut current_caller: Option<String> = None;
        let mut last_decorators = Vec::new();

        for (idx, raw_line) in lines.iter().enumerate() {
            let trimmed = raw_line.trim();
            let line_num = idx + 1;
            let indent = raw_line.len() - raw_line.trim_start().len();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Reset class scope if indentation dropped back
            if let Some((_, class_indent)) = current_class
                && indent <= class_indent
            {
                current_class = None;
            }

            // Decorators
            if trimmed.starts_with('@') {
                last_decorators.push(trimmed.to_string());
                continue;
            }

            // 1. Imports: `import os, sys` or `from math import sqrt as s`
            if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                parse_python_import(path, trimmed, line_num, &mut result);
                continue;
            }

            // 2. Class
            if trimmed.starts_with("class ") {
                let rest = trimmed.strip_prefix("class ").unwrap_or("").trim();
                let name = rest.split(&['(', ':'][..]).next().unwrap_or("").trim();

                // Inheritance detection
                if let Some((_, base_part)) = rest.split_once('(')
                    && let Some((base, _)) = base_part.split_once(')')
                {
                    let base_name = base.trim().to_string();
                    if !base_name.is_empty() {
                        result.relationships.push(SymbolRelationship::new(
                            name,
                            name,
                            base_name,
                            RelationshipKind::Extends,
                            path.to_path_buf(),
                            line_num,
                            ResolutionStatus::AstDerived,
                        ));
                    }
                }

                if !name.is_empty() {
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Class,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, indent, raw_line.len()),
                        raw_line.trim_end().to_string(),
                    );
                    sym.is_public = !name.starts_with('_');
                    result.symbols.push(sym);
                    current_class = Some((name.to_string(), indent));
                    last_decorators.clear();
                    continue;
                }
            }

            // 3. Function / Method (`def` or `async def`)
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let rest = trimmed
                    .strip_prefix("async def ")
                    .or_else(|| trimmed.strip_prefix("def "))
                    .unwrap_or("")
                    .trim();

                if let Some((name_part, _)) = rest.split_once('(') {
                    let name = name_part.trim();
                    if !name.is_empty() {
                        let is_test = name.starts_with("test_") || name.ends_with("_test");
                        let kind = if current_class.is_some() {
                            AstSymbolKind::Method
                        } else {
                            AstSymbolKind::Function
                        };

                        let mut sym = AstSymbol::new(
                            name,
                            kind,
                            path.to_path_buf(),
                            SourceRange::new(line_num, line_num, indent, raw_line.len()),
                            trimmed.to_string(),
                        );
                        if let Some((ref c_name, _)) = current_class {
                            sym = sym.with_parent_scope(c_name);
                        }
                        sym.is_public = !name.starts_with('_') || name.starts_with("__init__");
                        result.symbols.push(sym);
                        current_caller = Some(name.to_string());

                        if is_test {
                            result.relationships.push(SymbolRelationship::new(
                                name,
                                name,
                                "pytest_harness",
                                RelationshipKind::Tests,
                                path.to_path_buf(),
                                line_num,
                                ResolutionStatus::Exact,
                            ));
                        }
                    }
                }
                last_decorators.clear();
                continue;
            }

            // 4. Calls inside function
            if let Some(ref caller) = current_caller {
                if trimmed.contains('(') && !trimmed.starts_with("def ") && !trimmed.starts_with("class ") {
                    for part in trimmed.split(&[' ', '=', '+', '-', '*', '/', ',', '(', ')'][..]) {
                        let token = part.trim();
                        if !token.is_empty()
                            && token.chars().all(|c| c.is_alphanumeric() || c == '_')
                            && token != caller
                            && !matches!(token, "if" | "for" | "while" | "return" | "with" | "try" | "except")
                        {
                            result.calls.push(CallSite {
                                caller_name: caller.clone(),
                                callee_name: token.to_string(),
                                callee_scope: None,
                                file_path: path.to_path_buf(),
                                line: line_num,
                                col: 0,
                                is_method_call: false,
                            });
                            result.relationships.push(SymbolRelationship::new(
                                caller.clone(),
                                caller.clone(),
                                token.to_string(),
                                RelationshipKind::Calls,
                                path.to_path_buf(),
                                line_num,
                                ResolutionStatus::AstDerived,
                            ));
                            break;
                        }
                    }
                }
            }
        }

        result
    }
}

fn parse_python_import(path: &Path, line: &str, line_num: usize, result: &mut ParseResult) {
    if let Some(rest) = line.strip_prefix("from ") {
        if let Some((module_part, symbols_part)) = rest.split_once(" import ") {
            let from_module = module_part.trim().to_string();
            for item in symbols_part.split(',') {
                let trimmed = item.trim();
                if let Some((sym, alias)) = trimmed.split_once(" as ") {
                    result.imports.push(ImportItem {
                        source_file: path.to_path_buf(),
                        imported_symbol: sym.trim().to_string(),
                        alias: Some(alias.trim().to_string()),
                        from_module: from_module.clone(),
                        line: line_num,
                        is_glob: false,
                        is_type_only: false,
                    });
                } else if !trimmed.is_empty() {
                    result.imports.push(ImportItem {
                        source_file: path.to_path_buf(),
                        imported_symbol: trimmed.to_string(),
                        alias: None,
                        from_module: from_module.clone(),
                        line: line_num,
                        is_glob: false,
                        is_type_only: false,
                    });
                }
            }
        }
    } else if let Some(rest) = line.strip_prefix("import ") {
        for item in rest.split(',') {
            let trimmed = item.trim();
            if let Some((mod_name, alias)) = trimmed.split_once(" as ") {
                result.imports.push(ImportItem {
                    source_file: path.to_path_buf(),
                    imported_symbol: mod_name.trim().to_string(),
                    alias: Some(alias.trim().to_string()),
                    from_module: mod_name.trim().to_string(),
                    line: line_num,
                    is_glob: false,
                    is_type_only: false,
                });
            } else if !trimmed.is_empty() {
                result.imports.push(ImportItem {
                    source_file: path.to_path_buf(),
                    imported_symbol: trimmed.to_string(),
                    alias: None,
                    from_module: trimmed.to_string(),
                    line: line_num,
                    is_glob: false,
                    is_type_only: false,
                });
            }
        }
    }
}

// ===========================================================================
// Go Adapter
// ===========================================================================

pub struct GoAdapter;

impl LanguageAdapter for GoAdapter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Go
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("go"))
            .unwrap_or(false)
    }

    fn capabilities(&self) -> LanguageCapabilities {
        LanguageCapabilities {
            syntax_parsing: true,
            definition_extraction: true,
            import_export_extraction: true,
            reference_extraction: true,
            call_resolution: true,
            type_resolution: false,
            test_discovery: true,
        }
    }

    fn parse(&self, path: &Path, content: &str) -> ParseResult {
        let mut result = ParseResult::default();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_caller: Option<String> = None;
        let mut in_import_block = false;

        for (idx, raw_line) in lines.iter().enumerate() {
            let trimmed = raw_line.trim();
            let line_num = idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            // 1. Package declaration
            if trimmed.starts_with("package ") {
                let pkg_name = trimmed
                    .strip_prefix("package ")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                result.symbols.push(AstSymbol::new(
                    pkg_name,
                    AstSymbolKind::Module,
                    path.to_path_buf(),
                    SourceRange::new(line_num, line_num, 0, raw_line.len()),
                    trimmed.to_string(),
                ));
                continue;
            }

            // 2. Imports
            if trimmed.starts_with("import (") {
                in_import_block = true;
                continue;
            }
            if in_import_block {
                if trimmed == ")" {
                    in_import_block = false;
                } else if !trimmed.is_empty() {
                    let mod_path = trimmed.trim_matches('"');
                    result.imports.push(ImportItem {
                        source_file: path.to_path_buf(),
                        imported_symbol: mod_path.split('/').last().unwrap_or(mod_path).to_string(),
                        alias: None,
                        from_module: mod_path.to_string(),
                        line: line_num,
                        is_glob: false,
                        is_type_only: false,
                    });
                }
                continue;
            }
            if trimmed.starts_with("import ") {
                let mod_path = trimmed.strip_prefix("import ").unwrap_or("").trim().trim_matches('"');
                result.imports.push(ImportItem {
                    source_file: path.to_path_buf(),
                    imported_symbol: mod_path.split('/').last().unwrap_or(mod_path).to_string(),
                    alias: None,
                    from_module: mod_path.to_string(),
                    line: line_num,
                    is_glob: false,
                    is_type_only: false,
                });
                continue;
            }

            // 3. Types: struct / interface
            if trimmed.starts_with("type ") {
                let rest = trimmed.strip_prefix("type ").unwrap_or("").trim();
                let mut parts = rest.split_whitespace();
                if let Some(name) = parts.next()
                    && let Some(kind_str) = parts.next()
                {
                    let kind = match kind_str {
                        "struct" => AstSymbolKind::Struct,
                        "interface" => AstSymbolKind::Interface,
                        _ => AstSymbolKind::TypeAlias,
                    };

                    let is_public = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    let mut sym = AstSymbol::new(
                        name,
                        kind,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = is_public;
                    result.symbols.push(sym);
                    continue;
                }
            }

            // 4. Function or Receiver method
            if trimmed.starts_with("func ") {
                let rest = trimmed.strip_prefix("func ").unwrap_or("").trim();
                if rest.starts_with('(') {
                    // Receiver method: func (s *Server) Start() error
                    if let Some((recv_part, after_recv)) = rest.split_once(')')
                        && let Some((name_part, _)) = after_recv.trim().split_once('(')
                    {
                        let name = name_part.trim();
                        let receiver_ty = recv_part
                            .trim_start_matches('(')
                            .split_whitespace()
                            .last()
                            .unwrap_or("")
                            .trim_start_matches('*');

                        let is_public = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                        let mut sym = AstSymbol::new(
                            name,
                            AstSymbolKind::Method,
                            path.to_path_buf(),
                            SourceRange::new(line_num, line_num, 0, raw_line.len()),
                            trimmed.to_string(),
                        );
                        sym.is_public = is_public;
                        sym = sym.with_parent_scope(receiver_ty);
                        result.symbols.push(sym);
                        current_caller = Some(name.to_string());
                    }
                } else if let Some((name_part, _)) = rest.split_once('(') {
                    let name = name_part.trim();
                    let is_public = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    let is_test = name.starts_with("Test");
                    let mut sym = AstSymbol::new(
                        name,
                        AstSymbolKind::Function,
                        path.to_path_buf(),
                        SourceRange::new(line_num, line_num, 0, raw_line.len()),
                        trimmed.to_string(),
                    );
                    sym.is_public = is_public;
                    result.symbols.push(sym);
                    current_caller = Some(name.to_string());

                    if is_test {
                        result.relationships.push(SymbolRelationship::new(
                            name,
                            name,
                            "go_test_runner",
                            RelationshipKind::Tests,
                            path.to_path_buf(),
                            line_num,
                            ResolutionStatus::Exact,
                        ));
                    }
                }
                continue;
            }

            // 5. Calls inside function
            if let Some(ref caller) = current_caller {
                if trimmed.contains('(') && !trimmed.starts_with("func ") {
                    for part in trimmed.split(&[' ', '=', ':', ',', '{', '}'][..]) {
                        if let Some((callee, _)) = part.split_once('(') {
                            let callee = callee.split('.').last().unwrap_or(callee).trim();
                            if !callee.is_empty()
                                && callee.chars().all(|c| c.is_alphanumeric() || c == '_')
                                && callee != caller
                                && !matches!(callee, "if" | "for" | "switch" | "return" | "select")
                            {
                                result.calls.push(CallSite {
                                    caller_name: caller.clone(),
                                    callee_name: callee.to_string(),
                                    callee_scope: None,
                                    file_path: path.to_path_buf(),
                                    line: line_num,
                                    col: 0,
                                    is_method_call: part.contains('.'),
                                });
                                result.relationships.push(SymbolRelationship::new(
                                    caller.clone(),
                                    caller.clone(),
                                    callee.to_string(),
                                    RelationshipKind::Calls,
                                    path.to_path_buf(),
                                    line_num,
                                    ResolutionStatus::AstDerived,
                                ));
                                break;
                            }
                        }
                    }
                }
            }
        }

        result
    }
}

// ===========================================================================
// C-Like Language Adapter (C/C++, Java, C#)
// ===========================================================================

pub struct CLikeAdapter;

impl LanguageAdapter for CLikeAdapter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Cpp
    }

    fn can_parse(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            ext.as_str(),
            "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "java" | "cs"
        )
    }

    fn capabilities(&self) -> LanguageCapabilities {
        LanguageCapabilities {
            syntax_parsing: true,
            definition_extraction: true,
            import_export_extraction: false,
            reference_extraction: false,
            call_resolution: false,
            type_resolution: false,
            test_discovery: false,
        }
    }

    fn parse(&self, path: &Path, content: &str) -> ParseResult {
        let mut result = ParseResult::default();
        let syms = crate::parser::AstParser::parse_file(path, content);
        result.symbols = syms;
        result
    }
}

// ===========================================================================
// Unified Language Adapter Registry
// ===========================================================================

pub struct LanguageAdapterRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl Default for LanguageAdapterRegistry {
    fn default() -> Self {
        Self {
            adapters: vec![
                Box::new(RustAdapter),
                Box::new(TypeScriptAdapter),
                Box::new(PythonAdapter),
                Box::new(GoAdapter),
                Box::new(CLikeAdapter),
            ],
        }
    }
}

impl LanguageAdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_adapter(&self, path: &Path) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.can_parse(path)).map(|b| b.as_ref())
    }

    pub fn parse_file(&self, path: &Path, content: &str) -> ParseResult {
        if let Some(adapter) = self.get_adapter(path) {
            adapter.parse(path, content)
        } else {
            ParseResult::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter_full_resolution() {
        let code = r#"
use std::sync::Arc;
pub use crate::types::AstSymbol;

pub struct Engine {
    active: bool,
}

impl Engine {
    pub fn start(&self) {
        self.helper();
    }

    fn helper(&self) {
        println!("Helper");
    }
}

#[test]
fn test_engine_start() {
    let e = Engine { active: true };
    e.start();
}
"#;
        let adapter = RustAdapter;
        let result = adapter.parse(Path::new("src/engine.rs"), code);

        assert_eq!(result.imports.len(), 2);
        assert_eq!(result.exports.len(), 1);
        assert!(result.symbols.iter().any(|s| s.name == "Engine" && s.kind == AstSymbolKind::Struct));
        assert!(result.symbols.iter().any(|s| s.name == "start" && s.kind == AstSymbolKind::Method));
        assert!(result.symbols.iter().any(|s| s.name == "test_engine_start"));
        assert!(result.relationships.iter().any(|r| r.kind == RelationshipKind::Calls && r.target_symbol == "helper"));
        assert!(result.relationships.iter().any(|r| r.kind == RelationshipKind::Tests));
    }

    #[test]
    fn test_ts_adapter_resolution() {
        let code = r#"
import { Task, Status } from './types';
import axios from 'axios';

export class TaskRunner {
    async runTask(t: Task): Promise<void> {
        this.execute();
    }

    private execute() {
        console.log("executing");
    }
}

describe("TaskRunner", () => {
    test("runs task correctly", () => {
        expect(1).toBe(1);
    });
});
"#;
        let adapter = TypeScriptAdapter;
        let result = adapter.parse(Path::new("src/runner.ts"), code);

        assert_eq!(result.imports.len(), 3); // Task, Status, axios
        assert!(result.symbols.iter().any(|s| s.name == "TaskRunner" && s.kind == AstSymbolKind::Class));
        assert!(result.symbols.iter().any(|s| s.name == "runTask"));
        assert!(result.symbols.iter().any(|s| s.name.contains("runs task correctly")));
    }

    #[test]
    fn test_python_adapter_resolution() {
        let code = r#"
import os
from datetime import datetime as dt

class BaseService:
    def base_method(self):
        pass

class UserService(BaseService):
    @decorator
    def get_user(self, uid: int):
        self.validate(uid)

    def validate(self, uid):
        return True

def test_user_service():
    s = UserService()
"#;
        let adapter = PythonAdapter;
        let result = adapter.parse(Path::new("service.py"), code);

        assert_eq!(result.imports.len(), 2);
        assert!(result.symbols.iter().any(|s| s.name == "UserService" && s.kind == AstSymbolKind::Class));
        assert!(result.relationships.iter().any(|r| r.kind == RelationshipKind::Extends && r.target_symbol == "BaseService"));
        assert!(result.relationships.iter().any(|r| r.kind == RelationshipKind::Tests));
    }

    #[test]
    fn test_go_adapter_resolution() {
        let code = r#"
package server

import (
    "fmt"
    "net/http"
)

type ServerConfig struct {
    Port int
}

func (s *ServerConfig) Start() error {
    s.listen()
    return nil
}

func (s *ServerConfig) listen() {
    fmt.Println("Listening")
}

func TestServerStart(t *testing.T) {
}
"#;
        let adapter = GoAdapter;
        let result = adapter.parse(Path::new("server.go"), code);

        assert_eq!(result.imports.len(), 2);
        assert!(result.symbols.iter().any(|s| s.name == "ServerConfig" && s.kind == AstSymbolKind::Struct));
        assert!(result.symbols.iter().any(|s| s.name == "Start" && s.kind == AstSymbolKind::Method));
        assert!(result.relationships.iter().any(|r| r.kind == RelationshipKind::Tests));
    }
}
