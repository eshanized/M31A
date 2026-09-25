use crate::repo::graph::RepositoryEdgeKind;
use crate::repo::types::{EvidenceProvenance, FactClass, RepositorySymbol, SymbolKind};
use syn::visit::{self, Visit};
use syn::{
    ExprCall, ExprMethodCall, ItemEnum, ItemFn, ItemImpl, ItemMod, ItemStruct, ItemTrait, ItemUse,
    UseTree,
};

/// syn 2.0 AST visitor for Rust repository files.
/// Generates `FactClass::VerifiedFact` symbols for functions, structs, enums, traits, modules, and impls.
pub struct RustAstExtractor<'a> {
    file_path: String,
    content: &'a str,
    line_offsets: Vec<usize>,
    pub symbols: Vec<RepositorySymbol>,
    pub edges: Vec<(String, String, RepositoryEdgeKind)>,
    current_module: Vec<String>,
    current_function_id: Option<String>,
    in_test_module: bool,
}

pub type ExtractedSymbolsAndEdges = (
    Vec<RepositorySymbol>,
    Vec<(String, String, RepositoryEdgeKind)>,
);

impl<'a> RustAstExtractor<'a> {
    pub fn new(file_path: impl Into<String>, content: &'a str) -> Self {
        let mut line_offsets = Vec::new();
        line_offsets.push(0);
        for (i, b) in content.bytes().enumerate() {
            if b == b'\n' {
                line_offsets.push(i + 1);
            }
        }
        Self {
            file_path: file_path.into(),
            content,
            line_offsets,
            symbols: Vec::new(),
            edges: Vec::new(),
            current_module: Vec::new(),
            current_function_id: None,
            in_test_module: false,
        }
    }

    /// Primary extraction entry point returning discovered symbols.
    pub fn extract(file_path: &str, content: &'a str) -> Result<Vec<RepositorySymbol>, String> {
        let (symbols, _) = Self::extract_with_edges(file_path, content)?;
        Ok(symbols)
    }

    /// Full extraction entry point returning both verified symbols and semantic edges.
    pub fn extract_with_edges(
        file_path: &str,
        content: &'a str,
    ) -> Result<ExtractedSymbolsAndEdges, String> {
        let syntax_tree =
            syn::parse_file(content).map_err(|e| format!("parse error in {file_path}: {e}"))?;
        let mut extractor = Self::new(file_path, content);
        extractor.visit_file(&syntax_tree);
        Ok((extractor.symbols, extractor.edges))
    }

    fn byte_offset_to_line(&self, offset: usize) -> usize {
        match self.line_offsets.binary_search(&offset) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        }
    }

    /// Locate item start line, end line, and signature from source content.
    fn locate_item(&self, keyword: &str, ident: &str) -> (usize, usize, String) {
        let search_pattern = format!("{keyword} {ident}");
        if let Some(pos) = self.content.find(&search_pattern) {
            let line_start = self.line_offsets[self.byte_offset_to_line(pos) - 1];
            let start_line = self.byte_offset_to_line(pos);

            // Look forward for signature boundary ('{' or ';')
            let remainder = &self.content[pos..];
            let (sig_len, has_body) = match remainder.find(['{', ';']) {
                Some(idx) => {
                    let char_at = remainder.as_bytes()[idx];
                    (idx, char_at == b'{')
                }
                None => (
                    remainder.lines().next().map(|l| l.len()).unwrap_or(0),
                    false,
                ),
            };

            // Include leading qualifiers on the line (e.g. `pub async `)
            let prefix = self.content[line_start..pos].trim_start();
            let sig_core = remainder[..sig_len].trim();
            let signature = if prefix.is_empty() {
                sig_core.to_string()
            } else {
                format!("{prefix} {sig_core}")
            };

            // Determine end line
            let end_line = if has_body {
                let body_start = pos + sig_len;
                let mut depth = 0;
                let mut end_offset = body_start;
                for (i, b) in self.content[body_start..].bytes().enumerate() {
                    if b == b'{' {
                        depth += 1;
                    } else if b == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            end_offset = body_start + i;
                            break;
                        }
                    }
                }
                self.byte_offset_to_line(end_offset)
            } else {
                start_line
            };

            (start_line, end_line.max(start_line), signature)
        } else {
            (1, 1, format!("{keyword} {ident}"))
        }
    }

    fn extract_doc_comments(attrs: &[syn::Attribute]) -> Option<String> {
        let mut docs = Vec::new();
        for attr in attrs {
            if attr.path().is_ident("doc")
                && let syn::Meta::NameValue(syn::MetaNameValue {
                    value:
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }),
                    ..
                }) = &attr.meta
            {
                docs.push(s.value().trim().to_string());
            }
        }
        if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        }
    }

    fn qualified_prefix(&self) -> String {
        if self.current_module.is_empty() {
            self.file_path.clone()
        } else {
            format!("{}::{}", self.file_path, self.current_module.join("::"))
        }
    }
}

impl<'a, 'ast> Visit<'ast> for RustAstExtractor<'a> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let name = node.sig.ident.to_string();
        let q_prefix = self.qualified_prefix();
        let qualified_name = format!("{q_prefix}::{name}");
        let id = format!("{qualified_name}#fn");
        let (start_line, end_line, signature) = self.locate_item("fn", &name);
        let doc_comment = Self::extract_doc_comments(&node.attrs);

        let is_test = self.in_test_module
            || name.starts_with("test_")
            || node.attrs.iter().any(|a| {
                a.path()
                    .segments
                    .iter()
                    .any(|s| s.ident == "test" || s.ident == "tokio_test")
            });

        let provenance = EvidenceProvenance::verified_ast(&self.file_path, start_line, end_line);

        self.symbols.push(
            RepositorySymbol::new(
                id.clone(),
                name,
                qualified_name,
                SymbolKind::Function,
                &self.file_path,
                start_line,
                end_line,
                signature,
                FactClass::VerifiedFact,
                doc_comment,
            )
            .with_test(is_test)
            .with_provenance(provenance),
        );

        let prev_fn = self.current_function_id.take();
        self.current_function_id = Some(id);
        visit::visit_item_fn(self, node);
        self.current_function_id = prev_fn;
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let name = node.ident.to_string();
        let q_prefix = self.qualified_prefix();
        let qualified_name = format!("{q_prefix}::{name}");
        let id = format!("{qualified_name}#struct");
        let (start_line, end_line, signature) = self.locate_item("struct", &name);
        let doc_comment = Self::extract_doc_comments(&node.attrs);
        let provenance = EvidenceProvenance::verified_ast(&self.file_path, start_line, end_line);

        self.symbols.push(
            RepositorySymbol::new(
                id,
                name,
                qualified_name,
                SymbolKind::Struct,
                &self.file_path,
                start_line,
                end_line,
                signature,
                FactClass::VerifiedFact,
                doc_comment,
            )
            .with_test(self.in_test_module)
            .with_provenance(provenance),
        );

        visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
        let name = node.ident.to_string();
        let q_prefix = self.qualified_prefix();
        let qualified_name = format!("{q_prefix}::{name}");
        let id = format!("{qualified_name}#enum");
        let (start_line, end_line, signature) = self.locate_item("enum", &name);
        let doc_comment = Self::extract_doc_comments(&node.attrs);
        let provenance = EvidenceProvenance::verified_ast(&self.file_path, start_line, end_line);

        self.symbols.push(
            RepositorySymbol::new(
                id,
                name,
                qualified_name,
                SymbolKind::Enum,
                &self.file_path,
                start_line,
                end_line,
                signature,
                FactClass::VerifiedFact,
                doc_comment,
            )
            .with_test(self.in_test_module)
            .with_provenance(provenance),
        );

        visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
        let name = node.ident.to_string();
        let q_prefix = self.qualified_prefix();
        let qualified_name = format!("{q_prefix}::{name}");
        let id = format!("{qualified_name}#trait");
        let (start_line, end_line, signature) = self.locate_item("trait", &name);
        let doc_comment = Self::extract_doc_comments(&node.attrs);
        let provenance = EvidenceProvenance::verified_ast(&self.file_path, start_line, end_line);

        self.symbols.push(
            RepositorySymbol::new(
                id,
                name,
                qualified_name,
                SymbolKind::Trait,
                &self.file_path,
                start_line,
                end_line,
                signature,
                FactClass::VerifiedFact,
                doc_comment,
            )
            .with_test(self.in_test_module)
            .with_provenance(provenance),
        );

        visit::visit_item_trait(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let name = node.ident.to_string();
        let q_prefix = self.qualified_prefix();
        let qualified_name = format!("{q_prefix}::{name}");
        let id = format!("{qualified_name}#mod");
        let (start_line, end_line, signature) = self.locate_item("mod", &name);
        let doc_comment = Self::extract_doc_comments(&node.attrs);
        let is_test_mod = name == "tests"
            || name.ends_with("_test")
            || name.starts_with("test_")
            || node.attrs.iter().any(|a| a.path().is_ident("test"));

        let provenance = EvidenceProvenance::verified_ast(&self.file_path, start_line, end_line);

        self.symbols.push(
            RepositorySymbol::new(
                id,
                name.clone(),
                qualified_name,
                SymbolKind::Module,
                &self.file_path,
                start_line,
                end_line,
                signature,
                FactClass::VerifiedFact,
                doc_comment,
            )
            .with_test(is_test_mod || self.in_test_module)
            .with_provenance(provenance),
        );

        let prev_test = self.in_test_module;
        if is_test_mod {
            self.in_test_module = true;
        }
        self.current_module.push(name);
        visit::visit_item_mod(self, node);
        self.current_module.pop();
        self.in_test_module = prev_test;
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        fn collect_use_paths(tree: &UseTree, prefix: &str, acc: &mut Vec<String>) {
            match tree {
                UseTree::Path(p) => {
                    let next = if prefix.is_empty() {
                        p.ident.to_string()
                    } else {
                        format!("{}::{}", prefix, p.ident)
                    };
                    collect_use_paths(&p.tree, &next, acc);
                }
                UseTree::Name(n) => {
                    let full = if prefix.is_empty() {
                        n.ident.to_string()
                    } else {
                        format!("{}::{}", prefix, n.ident)
                    };
                    acc.push(full);
                }
                UseTree::Rename(r) => {
                    let full = if prefix.is_empty() {
                        r.ident.to_string()
                    } else {
                        format!("{}::{}", prefix, r.ident)
                    };
                    acc.push(full);
                }
                UseTree::Glob(_) => {
                    if !prefix.is_empty() {
                        acc.push(format!("{}::*", prefix));
                    }
                }
                UseTree::Group(g) => {
                    for item in &g.items {
                        collect_use_paths(item, prefix, acc);
                    }
                }
            }
        }

        let mut imported = Vec::new();
        collect_use_paths(&node.tree, "", &mut imported);
        for imp in imported {
            self.edges
                .push((self.file_path.clone(), imp, RepositoryEdgeKind::Imports));
        }

        visit::visit_item_use(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(ref caller_id) = self.current_function_id
            && let syn::Expr::Path(p) = &*node.func
            && let Some(target_ident) = p.path.segments.last().map(|s| s.ident.to_string())
        {
            self.edges
                .push((caller_id.clone(), target_ident, RepositoryEdgeKind::Calls));
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if let Some(ref caller_id) = self.current_function_id {
            let method_name = node.method.to_string();
            self.edges
                .push((caller_id.clone(), method_name, RepositoryEdgeKind::Calls));
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        // Extract type name from self_ty
        let self_ty_str = match &*node.self_ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
            _ => "ImplType".to_string(),
        };

        let trait_str = node.trait_.as_ref().map(|(_, path, _)| {
            path.segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "Trait".to_string())
        });

        let (name, sig_hint) = match &trait_str {
            Some(t) => (
                format!("{t} for {self_ty_str}"),
                format!("impl {t} for {self_ty_str}"),
            ),
            None => (self_ty_str.clone(), format!("impl {self_ty_str}")),
        };

        let q_prefix = self.qualified_prefix();
        let qualified_name = format!("{q_prefix}::{name}");
        let id = format!("{qualified_name}#impl");
        let (start_line, end_line, signature) = self.locate_item("impl", &self_ty_str);
        let doc_comment = Self::extract_doc_comments(&node.attrs);
        let provenance = EvidenceProvenance::verified_ast(&self.file_path, start_line, end_line);

        self.symbols.push(
            RepositorySymbol::new(
                id,
                name,
                qualified_name,
                SymbolKind::Impl,
                &self.file_path,
                start_line,
                end_line,
                if signature.starts_with("impl") {
                    signature
                } else {
                    sig_hint
                },
                FactClass::VerifiedFact,
                doc_comment,
            )
            .with_test(self.in_test_module)
            .with_provenance(provenance),
        );

        visit::visit_item_impl(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_ast_extractor_functions_and_structs() {
        let code = r#"
/// A documentation comment for MyStruct.
pub struct MyStruct {
    field: i32,
}

/// A top-level function.
pub async fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

pub enum Color {
    Red,
    Green,
    Blue,
}

pub trait Greeter {
    fn greet(&self) -> String;
}

impl Greeter for MyStruct {
    fn greet(&self) -> String {
        format!("hello {}", self.field)
    }
}
"#;

        let symbols =
            RustAstExtractor::extract("src/example.rs", code).expect("extraction succeeds");
        assert_eq!(symbols.len(), 5);

        // Struct
        let s = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Struct)
            .unwrap();
        assert_eq!(s.name, "MyStruct");
        assert_eq!(s.fact_class, FactClass::VerifiedFact);
        assert_eq!(
            s.doc_comment.as_deref(),
            Some("A documentation comment for MyStruct.")
        );
        assert!(s.start_line >= 3);
        assert!(s.end_line >= s.start_line);

        // Function
        let f = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Function)
            .unwrap();
        assert_eq!(f.name, "calculate_sum");
        assert_eq!(f.fact_class, FactClass::VerifiedFact);
        assert_eq!(f.doc_comment.as_deref(), Some("A top-level function."));
        assert!(f.signature.contains("fn calculate_sum"));

        // Enum
        let e = symbols.iter().find(|s| s.kind == SymbolKind::Enum).unwrap();
        assert_eq!(e.name, "Color");
        assert_eq!(e.fact_class, FactClass::VerifiedFact);

        // Trait
        let t = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Trait)
            .unwrap();
        assert_eq!(t.name, "Greeter");
        assert_eq!(t.fact_class, FactClass::VerifiedFact);

        // Impl
        let imp = symbols.iter().find(|s| s.kind == SymbolKind::Impl).unwrap();
        assert_eq!(imp.name, "Greeter for MyStruct");
        assert_eq!(imp.fact_class, FactClass::VerifiedFact);
    }
}
