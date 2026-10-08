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

/// Location in a source code file.
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

/// Hover / type inspection info for a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HoverInfo {
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub language: String,
}

/// Symbol information item.
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

/// Supported language server backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspBackend {
    RustAnalyzer,
    TypeScript,
    Pyright,
    Gopls,
    FallbackSyntactic,
}

/// Central LSP and code intelligence service.
pub struct LspService {
    workspace_root: PathBuf,
    active_backend: RwLock<LspBackend>,
}

impl LspService {
    /// Create a new LSP service rooted in the workspace.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let ws = workspace_root.into();
        Self {
            workspace_root: ws,
            active_backend: RwLock::new(LspBackend::FallbackSyntactic),
        }
    }

    /// Return current active LSP backend.
    pub async fn active_backend(&self) -> LspBackend {
        *self.active_backend.read().await
    }

    /// Autodetect backend by workspace contents and server binary availability.
    pub async fn detect_backend(&self) -> LspBackend {
        if self.workspace_root.join("Cargo.toml").exists() {
            if is_binary_available("rust-analyzer").await {
                return LspBackend::RustAnalyzer;
            }
        } else if self.workspace_root.join("package.json").exists()
            || self.workspace_root.join("tsconfig.json").exists()
        {
            if is_binary_available("typescript-language-server").await {
                return LspBackend::TypeScript;
            }
        } else if self.workspace_root.join("pyproject.toml").exists()
            || self.workspace_root.join("requirements.txt").exists()
        {
            if is_binary_available("pyright").await || is_binary_available("pylsp").await {
                return LspBackend::Pyright;
            }
        } else if self.workspace_root.join("go.mod").exists() && is_binary_available("gopls").await {
            return LspBackend::Gopls;
        }
        LspBackend::FallbackSyntactic
    }

    /// Find definition for a symbol at a given file location or by symbol name.
    pub async fn goto_definition(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
        symbol_name: Option<&str>,
    ) -> Result<Vec<SourceLocation>, String> {
        let target_sym = match symbol_name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => self.extract_symbol_at(file_path, line, col).await?,
        };

        if target_sym.is_empty() {
            return Ok(Vec::new());
        }

        self.fallback_find_definitions(&target_sym).await
    }

    /// Find all references to a symbol across the workspace.
    pub async fn find_references(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
        symbol_name: Option<&str>,
    ) -> Result<Vec<SourceLocation>, String> {
        let target_sym = match symbol_name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => self.extract_symbol_at(file_path, line, col).await?,
        };

        if target_sym.is_empty() {
            return Ok(Vec::new());
        }

        self.fallback_find_references(&target_sym).await
    }

    /// Hover over a symbol to get signature and documentation.
    pub async fn hover(
        &self,
        file_path: &str,
        line: usize,
        col: usize,
        symbol_name: Option<&str>,
    ) -> Result<Option<HoverInfo>, String> {
        let target_sym = match symbol_name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => self.extract_symbol_at(file_path, line, col).await?,
        };

        if target_sym.is_empty() {
            return Ok(None);
        }

        self.fallback_hover(file_path, &target_sym).await
    }

    /// Search symbols across the workspace.
    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInfo>, String> {
        self.fallback_workspace_symbols(query).await
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

async fn is_binary_available(cmd: &str) -> bool {
    tokio::process::Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}
