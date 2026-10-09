//! Language Server Protocol (LSP) and deep code intelligence subsystem (Issue 12).
//!
//! Provides typed code navigation, hover documentation, definition lookups, and reference
//! finding across Rust (`rust-analyzer`), TypeScript (`typescript-language-server`),
//! Python (`pyright`/`pylsp`), and Go (`gopls`), with automatic fallbacks to
//! syntactic and code-graph search when language servers are unavailable.

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

use crate::platform::process::is_binary_available;

/// location in a source code file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceLocation {
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub col_start: usize,
    pub col_end: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// hover / type inspection info for a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HoverInfo {
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub language: String,
}

/// symbol information item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

/// supported language server backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LspBackend {
    RustAnalyzer,
    TypeScript,
    Pyright,
    Gopls,
    FallbackSyntactic,
}

/// result of an lsp or code intelligence operation with backend and degradation provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LspQueryResult<T> {
    pub data: T,
    pub backend_used: LspBackend,
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

/// central lsp and code intelligence service.
pub struct LspService {
    workspace_root: PathBuf,
    active_backend: RwLock<LspBackend>,
    server_override: Option<String>,
    sessions: std::sync::Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<
                LspBackend,
                crate::capability::providers::local_lsp::PersistentLspSession,
            >,
        >,
    >,
}

impl LspService {
    /// create a new lsp service rooted in the workspace.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let ws = workspace_root.into();
        Self {
            workspace_root: ws,
            active_backend: RwLock::new(LspBackend::FallbackSyntactic),
            server_override: None,
            sessions: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
        }
    }

    /// override server binary command for testing or custom environments.
    pub fn with_server_override(mut self, cmd: impl Into<String>) -> Self {
        self.server_override = Some(cmd.into());
        self
    }

    /// return current active lsp backend.
    pub async fn active_backend(&self) -> LspBackend {
        *self.active_backend.read().await
    }

    /// autodetect backend by workspace contents and server binary availability.
    pub async fn detect_backend(&self) -> LspBackend {
        if let Some(ref override_cmd) = self.server_override {
            let backend = if is_binary_available(override_cmd).await {
                LspBackend::RustAnalyzer
            } else {
                LspBackend::FallbackSyntactic
            };
            *self.active_backend.write().await = backend;
            return backend;
        }

        let backend = if self.workspace_root.join("Cargo.toml").exists() {
            if is_binary_available("rust-analyzer").await {
                LspBackend::RustAnalyzer
            } else {
                LspBackend::FallbackSyntactic
            }
        } else if self.workspace_root.join("package.json").exists()
            || self.workspace_root.join("tsconfig.json").exists()
        {
            if is_binary_available("typescript-language-server").await {
                LspBackend::TypeScript
            } else {
                LspBackend::FallbackSyntactic
            }
        } else if self.workspace_root.join("pyproject.toml").exists()
            || self.workspace_root.join("requirements.txt").exists()
        {
            if is_binary_available("pyright").await || is_binary_available("pylsp").await {
                LspBackend::Pyright
            } else {
                LspBackend::FallbackSyntactic
            }
        } else if self.workspace_root.join("go.mod").exists() && is_binary_available("gopls").await
        {
            LspBackend::Gopls
        } else {
            LspBackend::FallbackSyntactic
        };

        *self.active_backend.write().await = backend;
        backend
    }

    /// execute stdio json-rpc query with bounded timeout via persistent session.
    async fn query_stdio(
        &self,
        backend: LspBackend,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let (cmd_bin, cmd_args): (&str, &[&str]) = if let Some(ref over) = self.server_override {
            (over.as_str(), &[])
        } else {
            match backend {
                LspBackend::RustAnalyzer => ("rust-analyzer", &[]),
                LspBackend::TypeScript => ("typescript-language-server", &["--stdio"]),
                LspBackend::Pyright => ("pyright-langserver", &["--stdio"]),
                LspBackend::Gopls => ("gopls", &[]),
                LspBackend::FallbackSyntactic => {
                    return Err("no language server configured".to_string());
                }
            }
        };

        let mut sessions = self.sessions.lock().await;
        let session = if let Some(s) = sessions.get(&backend) {
            if s.is_alive().await {
                s.clone()
            } else {
                let new_session =
                    crate::capability::providers::local_lsp::PersistentLspSession::start(
                        &self.workspace_root,
                        cmd_bin,
                        cmd_args,
                    )
                    .await?;
                sessions.insert(backend, new_session.clone());
                new_session
            }
        } else {
            let new_session = crate::capability::providers::local_lsp::PersistentLspSession::start(
                &self.workspace_root,
                cmd_bin,
                cmd_args,
            )
            .await?;
            sessions.insert(backend, new_session.clone());
            new_session
        };
        drop(sessions);

        match session
            .query(method, params.clone(), std::time::Duration::from_secs(5))
            .await
        {
            Ok(res) => Ok(res),
            Err(e) => {
                // bounded restart if session died
                if !session.is_alive().await {
                    let mut sessions = self.sessions.lock().await;
                    sessions.remove(&backend);
                    let restarted =
                        crate::capability::providers::local_lsp::PersistentLspSession::start(
                            &self.workspace_root,
                            cmd_bin,
                            cmd_args,
                        )
                        .await?;
                    sessions.insert(backend, restarted.clone());
                    drop(sessions);
                    restarted
                        .query(method, params, std::time::Duration::from_secs(5))
                        .await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// notify persistent lsp session of document open.
    pub async fn did_open(
        &self,
        file_path: &Path,
        content: &str,
        language_id: &str,
    ) -> Result<(), String> {
        let backend = self.detect_backend().await;
        if backend == LspBackend::FallbackSyntactic {
            return Ok(());
        }
        let (cmd_bin, cmd_args): (&str, &[&str]) = if let Some(ref over) = self.server_override {
            (over.as_str(), &[])
        } else {
            match backend {
                LspBackend::RustAnalyzer => ("rust-analyzer", &[]),
                LspBackend::TypeScript => ("typescript-language-server", &["--stdio"]),
                LspBackend::Pyright => ("pyright-langserver", &["--stdio"]),
                LspBackend::Gopls => ("gopls", &[]),
                LspBackend::FallbackSyntactic => return Ok(()),
            }
        };

        let mut sessions = self.sessions.lock().await;
        let session = if let Some(s) = sessions.get(&backend) {
            if s.is_alive().await {
                s.clone()
            } else {
                let s = crate::capability::providers::local_lsp::PersistentLspSession::start(
                    &self.workspace_root,
                    cmd_bin,
                    cmd_args,
                )
                .await?;
                sessions.insert(backend, s.clone());
                s
            }
        } else {
            let s = crate::capability::providers::local_lsp::PersistentLspSession::start(
                &self.workspace_root,
                cmd_bin,
                cmd_args,
            )
            .await?;
            sessions.insert(backend, s.clone());
            s
        };
        drop(sessions);
        session.did_open(file_path, content, language_id).await
    }

    /// notify persistent lsp session of document change.
    pub async fn did_change(&self, file_path: &Path, content: &str) -> Result<(), String> {
        let backend = self.active_backend().await;
        let sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&backend) {
            if session.is_alive().await {
                return session.did_change(file_path, content).await;
            }
        }
        Ok(())
    }

    /// notify persistent lsp session of document close.
    pub async fn did_close(&self, file_path: &Path) -> Result<(), String> {
        let backend = self.active_backend().await;
        let sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&backend) {
            if session.is_alive().await {
                return session.did_close(file_path).await;
            }
        }
        Ok(())
    }

    /// cleanly shutdown all active persistent lsp sessions.
    pub async fn shutdown(&self) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        for (_, session) in sessions.drain() {
            let _ = session.shutdown().await;
        }
        Ok(())
    }

    /// find definition for a symbol at a given file location or by symbol name.
    pub async fn goto_definition(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
        symbol_name: Option<&str>,
    ) -> Result<LspQueryResult<Vec<SourceLocation>>, String> {
        let backend = self.detect_backend().await;
        let target_sym = match symbol_name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => self.extract_symbol_at(file_path, line, col).await?,
        };

        if target_sym.is_empty() {
            return Ok(LspQueryResult {
                data: Vec::new(),
                backend_used: backend,
                fallback_used: backend == LspBackend::FallbackSyntactic,
                degraded_reason: if backend == LspBackend::FallbackSyntactic {
                    Some("no language server available or symbol is empty".to_string())
                } else {
                    None
                },
            });
        }

        if backend != LspBackend::FallbackSyntactic {
            let full_path = self.workspace_root.join(file_path);
            let file_uri = format!(
                "file://{}",
                full_path.canonicalize().unwrap_or(full_path).display()
            );
            let params = serde_json::json!({
                "textDocument": { "uri": file_uri },
                "position": {
                    "line": if line > 0 { line - 1 } else { 0 },
                    "character": if col > 0 { col - 1 } else { 0 },
                }
            });

            match self
                .query_stdio(backend, "textDocument/definition", params)
                .await
            {
                Ok(result_val) => {
                    let locs = parse_locations(&result_val, &self.workspace_root);
                    return Ok(LspQueryResult {
                        data: locs,
                        backend_used: backend,
                        fallback_used: false,
                        degraded_reason: None,
                    });
                }
                Err(err) => {
                    let locs = self.fallback_find_definitions(&target_sym).await?;
                    return Ok(LspQueryResult {
                        data: locs,
                        backend_used: backend,
                        fallback_used: true,
                        degraded_reason: Some(format!(
                            "lsp stdio failed ({err}); served via syntactic analyzer"
                        )),
                    });
                }
            }
        }

        let locs = self.fallback_find_definitions(&target_sym).await?;
        Ok(LspQueryResult {
            data: locs,
            backend_used: backend,
            fallback_used: true,
            degraded_reason: Some(
                "no language server binary available; served via syntactic analyzer".to_string(),
            ),
        })
    }

    /// find all references to a symbol across the workspace.
    pub async fn find_references(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
        symbol_name: Option<&str>,
    ) -> Result<LspQueryResult<Vec<SourceLocation>>, String> {
        let backend = self.detect_backend().await;
        let target_sym = match symbol_name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => self.extract_symbol_at(file_path, line, col).await?,
        };

        if target_sym.is_empty() {
            return Ok(LspQueryResult {
                data: Vec::new(),
                backend_used: backend,
                fallback_used: backend == LspBackend::FallbackSyntactic,
                degraded_reason: if backend == LspBackend::FallbackSyntactic {
                    Some("no language server available or symbol is empty".to_string())
                } else {
                    None
                },
            });
        }

        if backend != LspBackend::FallbackSyntactic {
            let full_path = self.workspace_root.join(file_path);
            let file_uri = format!(
                "file://{}",
                full_path.canonicalize().unwrap_or(full_path).display()
            );
            let params = serde_json::json!({
                "textDocument": { "uri": file_uri },
                "position": {
                    "line": if line > 0 { line - 1 } else { 0 },
                    "character": if col > 0 { col - 1 } else { 0 },
                },
                "context": { "includeDeclaration": true }
            });

            match self
                .query_stdio(backend, "textDocument/references", params)
                .await
            {
                Ok(result_val) => {
                    let locs = parse_locations(&result_val, &self.workspace_root);
                    return Ok(LspQueryResult {
                        data: locs,
                        backend_used: backend,
                        fallback_used: false,
                        degraded_reason: None,
                    });
                }
                Err(err) => {
                    let refs = self.fallback_find_references(&target_sym).await?;
                    return Ok(LspQueryResult {
                        data: refs,
                        backend_used: backend,
                        fallback_used: true,
                        degraded_reason: Some(format!(
                            "lsp stdio failed ({err}); served via syntactic analyzer"
                        )),
                    });
                }
            }
        }

        let refs = self.fallback_find_references(&target_sym).await?;
        Ok(LspQueryResult {
            data: refs,
            backend_used: backend,
            fallback_used: true,
            degraded_reason: Some(
                "no language server binary available; served via syntactic analyzer".to_string(),
            ),
        })
    }

    /// hover over a symbol to get signature and documentation.
    pub async fn hover(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
        symbol_name: Option<&str>,
    ) -> Result<LspQueryResult<Option<HoverInfo>>, String> {
        let backend = self.detect_backend().await;
        let target_sym = match symbol_name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => self.extract_symbol_at(file_path, line, col).await?,
        };

        if target_sym.is_empty() {
            return Ok(LspQueryResult {
                data: None,
                backend_used: backend,
                fallback_used: backend == LspBackend::FallbackSyntactic,
                degraded_reason: None,
            });
        }

        let lang = match Path::new(file_path).extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust",
            Some("ts") | Some("tsx") => "typescript",
            Some("js") | Some("jsx") => "javascript",
            Some("py") => "python",
            Some("go") => "go",
            _ => "text",
        };

        if backend != LspBackend::FallbackSyntactic {
            let full_path = self.workspace_root.join(file_path);
            let file_uri = format!(
                "file://{}",
                full_path.canonicalize().unwrap_or(full_path).display()
            );
            let params = serde_json::json!({
                "textDocument": { "uri": file_uri },
                "position": {
                    "line": if line > 0 { line - 1 } else { 0 },
                    "character": if col > 0 { col - 1 } else { 0 },
                }
            });

            match self
                .query_stdio(backend, "textDocument/hover", params)
                .await
            {
                Ok(result_val) => {
                    let info = parse_hover(&result_val, lang);
                    return Ok(LspQueryResult {
                        data: info,
                        backend_used: backend,
                        fallback_used: false,
                        degraded_reason: None,
                    });
                }
                Err(err) => {
                    let hover_info = self.fallback_hover(file_path, &target_sym).await?;
                    return Ok(LspQueryResult {
                        data: hover_info,
                        backend_used: backend,
                        fallback_used: true,
                        degraded_reason: Some(format!(
                            "lsp stdio failed ({err}); served via syntactic analyzer"
                        )),
                    });
                }
            }
        }

        let hover_info = self.fallback_hover(file_path, &target_sym).await?;
        Ok(LspQueryResult {
            data: hover_info,
            backend_used: backend,
            fallback_used: true,
            degraded_reason: Some(
                "no language server binary available; served via syntactic analyzer".to_string(),
            ),
        })
    }

    /// search symbols across the workspace.
    pub async fn workspace_symbols(
        &self,
        query: &str,
    ) -> Result<LspQueryResult<Vec<SymbolInfo>>, String> {
        let backend = self.detect_backend().await;

        if backend != LspBackend::FallbackSyntactic {
            let params = serde_json::json!({
                "query": query
            });

            match self.query_stdio(backend, "workspace/symbol", params).await {
                Ok(result_val) => {
                    let symbols = parse_symbols(&result_val, &self.workspace_root);
                    return Ok(LspQueryResult {
                        data: symbols,
                        backend_used: backend,
                        fallback_used: false,
                        degraded_reason: None,
                    });
                }
                Err(err) => {
                    let symbols = self.fallback_workspace_symbols(query).await?;
                    return Ok(LspQueryResult {
                        data: symbols,
                        backend_used: backend,
                        fallback_used: true,
                        degraded_reason: Some(format!(
                            "lsp stdio failed ({err}); served via syntactic analyzer"
                        )),
                    });
                }
            }
        }

        let symbols = self.fallback_workspace_symbols(query).await?;
        Ok(LspQueryResult {
            data: symbols,
            backend_used: backend,
            fallback_used: true,
            degraded_reason: Some(
                "no language server binary available; served via syntactic analyzer".to_string(),
            ),
        })
    }

    // ── Fallback Syntactic Implementation ──────────────────────────────────────

    async fn extract_symbol_at(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
    ) -> Result<String, String> {
        let full_path = self.workspace_root.join(file_path);
        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| format!("failed to read {file_path}: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();
        if line == 0 || line > lines.len() {
            return Err(format!("line {line} out of range (total {})", lines.len()));
        }

        let line_text = lines[line - 1];
        let col_idx = if col > 0 { col - 1 } else { 0 };
        if col_idx >= line_text.len() {
            return Ok(String::new());
        }

        let chars: Vec<char> = line_text.chars().collect();
        let mut start = col_idx;
        while start > 0 && is_ident_char(chars[start - 1]) {
            start -= 1;
        }
        let mut end = col_idx;
        while end < chars.len() && is_ident_char(chars[end]) {
            end += 1;
        }

        let sym: String = chars[start..end].iter().collect();
        Ok(sym)
    }

    async fn fallback_find_definitions(&self, symbol: &str) -> Result<Vec<SourceLocation>, String> {
        let mut locs = Vec::new();
        let re_str = format!(
            r"(?:fn|struct|enum|trait|type|class|interface|def)\s+{}\b",
            regex::escape(symbol)
        );
        let re = Regex::new(&re_str).map_err(|e| e.to_string())?;
        let mut dirs = vec![self.workspace_root.clone()];
        while let Some(current_dir) = dirs.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" || name == "node_modules" {
                        continue;
                    }
                    if let Ok(file_type) = entry.file_type().await {
                        if file_type.is_dir() {
                            dirs.push(path);
                        } else if file_type.is_file() && is_code_file(&path) {
                            if let Ok(text) = tokio::fs::read_to_string(&path).await {
                                for (idx, line) in text.lines().enumerate() {
                                    if re.is_match(line) {
                                        let rel_path = path
                                            .strip_prefix(&self.workspace_root)
                                            .unwrap_or(&path)
                                            .display()
                                            .to_string();
                                        locs.push(SourceLocation {
                                            file_path: rel_path,
                                            line_start: idx + 1,
                                            line_end: idx + 1,
                                            col_start: 1,
                                            col_end: line.len(),
                                            preview: Some(line.trim().to_string()),
                                        });
                                        if locs.len() >= 20 {
                                            return Ok(locs);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(locs)
    }

    async fn fallback_find_references(&self, symbol: &str) -> Result<Vec<SourceLocation>, String> {
        let mut locs = Vec::new();
        let re_str = format!(r"\b{}\b", regex::escape(symbol));
        let re = Regex::new(&re_str).map_err(|e| e.to_string())?;

        let mut dirs = vec![self.workspace_root.clone()];
        while let Some(current_dir) = dirs.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" || name == "node_modules" {
                        continue;
                    }
                    if let Ok(file_type) = entry.file_type().await {
                        if file_type.is_dir() {
                            dirs.push(path);
                        } else if file_type.is_file() && is_code_file(&path) {
                            if let Ok(text) = tokio::fs::read_to_string(&path).await {
                                for (idx, line) in text.lines().enumerate() {
                                    if re.is_match(line) {
                                        let rel_path = path
                                            .strip_prefix(&self.workspace_root)
                                            .unwrap_or(&path)
                                            .display()
                                            .to_string();
                                        locs.push(SourceLocation {
                                            file_path: rel_path,
                                            line_start: idx + 1,
                                            line_end: idx + 1,
                                            col_start: 1,
                                            col_end: line.len(),
                                            preview: Some(line.trim().to_string()),
                                        });
                                        if locs.len() >= 50 {
                                            return Ok(locs);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(locs)
    }

    async fn fallback_hover(
        &self,
        file_path: &str,
        symbol: &str,
    ) -> Result<Option<HoverInfo>, String> {
        let defs = self.fallback_find_definitions(symbol).await?;
        if let Some(def) = defs.first() {
            let sig = def.preview.clone().unwrap_or_else(|| symbol.to_string());
            let lang = match Path::new(file_path).extension().and_then(|e| e.to_str()) {
                Some("rs") => "rust",
                Some("ts") | Some("tsx") => "typescript",
                Some("js") | Some("jsx") => "javascript",
                Some("py") => "python",
                Some("go") => "go",
                _ => "text",
            };
            Ok(Some(HoverInfo {
                signature: sig,
                documentation: Some(format!("Defined at {}:{}", def.file_path, def.line_start)),
                language: lang.to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn fallback_workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInfo>, String> {
        let mut symbols = Vec::new();
        let query_lower = query.to_lowercase();
        let re = Regex::new(
            r"(?m)^\s*(?:pub\s+)?(?:fn|struct|enum|trait|type|class|interface|def)\s+([A-Za-z0-9_]+)",
        )
        .map_err(|e| e.to_string())?;

        let mut dirs = vec![self.workspace_root.clone()];
        while let Some(current_dir) = dirs.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "target" || name == "node_modules" {
                        continue;
                    }
                    if let Ok(file_type) = entry.file_type().await {
                        if file_type.is_dir() {
                            dirs.push(path);
                        } else if file_type.is_file() && is_code_file(&path) {
                            if let Ok(text) = tokio::fs::read_to_string(&path).await {
                                for (idx, line) in text.lines().enumerate() {
                                    if let Some(caps) = re.captures(line) {
                                        let sym_name = caps[1].to_string();
                                        if sym_name.to_lowercase().contains(&query_lower) {
                                            let rel_path = path
                                                .strip_prefix(&self.workspace_root)
                                                .unwrap_or(&path)
                                                .display()
                                                .to_string();
                                            symbols.push(SymbolInfo {
                                                name: sym_name,
                                                kind: "definition".to_string(),
                                                file_path: rel_path,
                                                line: idx + 1,
                                                column: 1,
                                                container_name: None,
                                            });
                                            if symbols.len() >= 50 {
                                                return Ok(symbols);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(symbols)
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_code_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "toml" | "json")
    )
}

fn uri_to_relative_path(uri: &str, workspace_root: &Path) -> String {
    let clean_path = if let Some(stripped) = uri.strip_prefix("file://") {
        stripped
    } else {
        uri
    };
    let path = Path::new(clean_path);
    if let Ok(rel) = path.strip_prefix(workspace_root) {
        rel.display().to_string()
    } else {
        clean_path.to_string()
    }
}

fn parse_locations(result: &serde_json::Value, workspace_root: &Path) -> Vec<SourceLocation> {
    let mut locs = Vec::new();
    let items = if let Some(arr) = result.as_array() {
        arr.as_slice()
    } else if result.is_object() {
        std::slice::from_ref(result)
    } else {
        &[]
    };

    for item in items {
        let uri = item
            .get("uri")
            .or_else(|| item.get("targetUri"))
            .and_then(|u| u.as_str());
        let range = item
            .get("range")
            .or_else(|| item.get("targetSelectionRange"))
            .or_else(|| item.get("targetRange"));

        if let (Some(uri_str), Some(range_val)) = (uri, range) {
            let start = range_val.get("start");
            let end = range_val.get("end");
            let line_start = start
                .and_then(|s| s.get("line"))
                .and_then(|l| l.as_u64())
                .unwrap_or(0) as usize
                + 1;
            let col_start = start
                .and_then(|s| s.get("character"))
                .and_then(|c| c.as_u64())
                .unwrap_or(0) as usize
                + 1;
            let line_end = end
                .and_then(|e| e.get("line"))
                .and_then(|l| l.as_u64())
                .unwrap_or(0) as usize
                + 1;
            let col_end = end
                .and_then(|e| e.get("character"))
                .and_then(|c| c.as_u64())
                .unwrap_or(0) as usize
                + 1;
            let rel_path = uri_to_relative_path(uri_str, workspace_root);

            locs.push(SourceLocation {
                file_path: rel_path,
                line_start,
                line_end,
                col_start,
                col_end,
                preview: None,
            });
        }
    }
    locs
}

fn parse_hover(result: &serde_json::Value, lang: &str) -> Option<HoverInfo> {
    if result.is_null() {
        return None;
    }
    let contents = result.get("contents")?;
    let (sig, doc) = if let Some(s) = contents.as_str() {
        (s.to_string(), None)
    } else if let Some(obj) = contents.as_object() {
        let val = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
        (val.to_string(), None)
    } else {
        let arr = contents.as_array()?;
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(val) = item.get("value").and_then(|v| v.as_str()) {
                parts.push(val.to_string());
            }
        }
        let sig = parts.first().cloned().unwrap_or_default();
        let doc = if parts.len() > 1 {
            Some(parts[1..].join("\n"))
        } else {
            None
        };
        (sig, doc)
    };

    Some(HoverInfo {
        signature: sig,
        documentation: doc,
        language: lang.to_string(),
    })
}

fn parse_symbols(result: &serde_json::Value, workspace_root: &Path) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    let items = match result.as_array() {
        Some(arr) => arr,
        None => return symbols,
    };
    for item in items {
        let name = match item.get("name").and_then(|n| n.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let kind_num = item.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
        let kind_str = match kind_num {
            5 => "class",
            6 => "method",
            11 => "variable",
            12 => "function",
            13 => "variable",
            23 => "struct",
            _ => "symbol",
        }
        .to_string();

        let location = item.get("location");
        let uri = location
            .and_then(|l| l.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let rel_path = uri_to_relative_path(uri, workspace_root);
        let start = location
            .and_then(|l| l.get("range"))
            .and_then(|r| r.get("start"));
        let line = start
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0) as usize
            + 1;
        let column = start
            .and_then(|s| s.get("character"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize
            + 1;
        let container_name = item
            .get("containerName")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        symbols.push(SymbolInfo {
            name,
            kind: kind_str,
            file_path: rel_path,
            line,
            column,
            container_name,
        });
    }
    symbols
}
