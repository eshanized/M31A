//! Lifecycle hook configuration discovery and execution coordinator.
//!
//! Discovers workspace-level and user-level `hooks.json` files and matches event triggers.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    PreModelRequest,
    PostModelRequest,
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    PreCompact,
    PostCompact,
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    SubagentStart,
    SubagentStop,
    Stop,
    Interrupt,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::PreModelRequest => "PreModelRequest",
            HookEvent::PostModelRequest => "PostModelRequest",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PermissionRequest => "PermissionRequest",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::PreCompact => "PreCompact",
            HookEvent::PostCompact => "PostCompact",
            HookEvent::SessionStart => "SessionStart",
            HookEvent::SessionEnd => "SessionEnd",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::SubagentStart => "SubagentStart",
            HookEvent::SubagentStop => "SubagentStop",
            HookEvent::Stop => "Stop",
            HookEvent::Interrupt => "Interrupt",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    pub name: String,
    pub event: HookEvent,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_timeout_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    pub hooks: Vec<HookDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutcome {
    pub allowed: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spilled_stdout_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spilled_stderr_path: Option<PathBuf>,
}

pub const DEFAULT_HOOK_OUTPUT_LIMIT_BYTES: usize = 10 * 1024; // 10 KB (~2,500 tokens)
pub const DEFAULT_PREVIEW_LINES: usize = 12;

#[derive(Debug, Clone)]
pub struct HookOutputSpiller {
    output_dir: PathBuf,
    max_bytes: usize,
}

impl HookOutputSpiller {
    pub fn new(session_id: &str) -> Self {
        let base_dir = std::env::temp_dir()
            .join("m31a_hook_outputs")
            .join(session_id);
        let _ = fs::create_dir_all(&base_dir);
        Self {
            output_dir: base_dir,
            max_bytes: DEFAULT_HOOK_OUTPUT_LIMIT_BYTES,
        }
    }

    pub fn with_limit(session_id: &str, max_bytes: usize) -> Self {
        let mut s = Self::new(session_id);
        s.max_bytes = max_bytes;
        s
    }

    /// Checks if the stream output exceeds `max_bytes`.
    /// If it does, writes the full output to disk under `<output_dir>/<hook_name>_<stream>_<uuid>.log`
    /// and returns the formatted head/tail preview along with the absolute file path.
    pub fn maybe_spill(
        &self,
        hook_name: &str,
        stream_name: &str,
        text: String,
    ) -> (String, Option<PathBuf>) {
        if text.len() <= self.max_bytes {
            return (text, None);
        }

        let file_id = uuid::Uuid::new_v4().to_string();
        let safe_name: String = hook_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let log_file = self.output_dir.join(format!(
            "{}_{}_{}.log",
            safe_name,
            stream_name,
            &file_id[..8]
        ));

        if let Err(e) = fs::write(&log_file, text.as_bytes()) {
            tracing::warn!(
                "Failed to spill hook output to {}: {}",
                log_file.display(),
                e
            );
            return (truncate_head_tail(&text, DEFAULT_PREVIEW_LINES), None);
        }

        let lines: Vec<&str> = text.lines().collect();
        let total_lines = lines.len();
        let head = lines
            .iter()
            .take(DEFAULT_PREVIEW_LINES)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let tail = lines
            .iter()
            .rev()
            .take(DEFAULT_PREVIEW_LINES)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        let preview = format!(
            "{}\n\n... [Hook output truncated: {} lines ({} KB) omitted] ...\n[Full hook output preserved at: {}]\n\n{}",
            head,
            total_lines.saturating_sub(DEFAULT_PREVIEW_LINES * 2),
            text.len() / 1024,
            log_file.display(),
            tail
        );

        (preview, Some(log_file))
    }
}

fn truncate_head_tail(text: &str, preview_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= preview_lines * 2 {
        return text.to_string();
    }
    let head = lines
        .iter()
        .take(preview_lines)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let tail = lines
        .iter()
        .rev()
        .take(preview_lines)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n... [Output truncated] ...\n{}", head, tail)
}

pub struct HookEngine {
    hooks: Vec<HookDefinition>,
}

impl HookEngine {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn load_from_dir(dir: &Path) -> Self {
        let mut engine = Self::new();

        // 1. Check workspace hooks configuration file (.m31a/hooks.json)
        let candidates = [
            dir.join(".m31a").join("hooks.json"),
            dir.join(".codex").join("hooks.json"),
        ];

        for c in &candidates {
            if c.exists()
                && let Ok(content) = fs::read_to_string(c)
                && let Ok(cfg) = serde_json::from_str::<HooksConfig>(&content)
            {
                engine.hooks.extend(cfg.hooks);
            }
        }

        // 2. Check user home hooks configuration file (~/.m31a/hooks.json)
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let home_candidates = [
            PathBuf::from(&home).join(".m31a").join("hooks.json"),
            PathBuf::from(&home).join(".codex").join("hooks.json"),
        ];

        for c in &home_candidates {
            if c.exists()
                && let Ok(content) = fs::read_to_string(c)
                && let Ok(cfg) = serde_json::from_str::<HooksConfig>(&content)
            {
                for h in cfg.hooks {
                    if !engine.hooks.iter().any(|existing| existing.name == h.name) {
                        engine.hooks.push(h);
                    }
                }
            }
        }

        engine
    }

    pub fn add_hook(&mut self, hook: HookDefinition) {
        self.hooks.push(hook);
    }

    pub fn list_hooks(&self) -> &[HookDefinition] {
        &self.hooks
    }

    pub async fn dispatch(
        &self,
        event: HookEvent,
        cwd: &Path,
        context: &HashMap<String, String>,
    ) -> Result<HookOutcome> {
        let matching: Vec<&HookDefinition> =
            self.hooks.iter().filter(|h| h.event == event).collect();
        if matching.is_empty() {
            return Ok(HookOutcome {
                allowed: true,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
                spilled_stdout_path: None,
                spilled_stderr_path: None,
            });
        }

        let session_id = context
            .get("session_id")
            .map(|s| s.as_str())
            .unwrap_or("default");
        let spiller = HookOutputSpiller::new(session_id);

        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();
        let mut last_spilled_stdout = None;
        let mut last_spilled_stderr = None;

        for hook in matching {
            let mut cmd = Command::new(&hook.command);
            cmd.args(&hook.args);
            cmd.current_dir(cwd);

            // Populate hook context environment variables
            cmd.env("M31A_HOOK_EVENT", event.as_str());
            for (k, v) in &hook.env {
                cmd.env(k, v);
            }
            for (k, v) in context {
                cmd.env(format!("M31A_CONTEXT_{}", k.to_uppercase()), v);
            }

            let output = cmd
                .output()
                .with_context(|| format!("Failed to execute hook `{}`", hook.name))?;
            let exit_code = output.status.code().unwrap_or(-1);
            let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();

            let (spilled_stdout, out_path) = spiller.maybe_spill(&hook.name, "stdout", raw_stdout);
            let (spilled_stderr, err_path) = spiller.maybe_spill(&hook.name, "stderr", raw_stderr);

            if let Some(p) = out_path {
                last_spilled_stdout = Some(p);
            }
            if let Some(p) = err_path {
                last_spilled_stderr = Some(p);
            }

            if !spilled_stdout.is_empty() {
                if !combined_stdout.is_empty() {
                    combined_stdout.push('\n');
                }
                combined_stdout.push_str(&spilled_stdout);
            }
            if !spilled_stderr.is_empty() {
                if !combined_stderr.is_empty() {
                    combined_stderr.push('\n');
                }
                combined_stderr.push_str(&spilled_stderr);
            }

            if exit_code != 0 {
                return Ok(HookOutcome {
                    allowed: false,
                    stdout: combined_stdout,
                    stderr: combined_stderr,
                    exit_code,
                    spilled_stdout_path: last_spilled_stdout,
                    spilled_stderr_path: last_spilled_stderr,
                });
            }
        }

        Ok(HookOutcome {
            allowed: true,
            stdout: combined_stdout,
            stderr: combined_stderr,
            exit_code: 0,
            spilled_stdout_path: last_spilled_stdout,
            spilled_stderr_path: last_spilled_stderr,
        })
    }
}

impl Default for HookEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_dispatch() {
        let mut engine = HookEngine::new();
        engine.add_hook(HookDefinition {
            name: "echo_test".to_string(),
            event: HookEvent::SessionStart,
            command: "echo".to_string(),
            args: vec!["session_started".to_string()],
            env: HashMap::new(),
            timeout_secs: 5,
        });

        let mut ctx = HashMap::new();
        ctx.insert("session_id".to_string(), "123".to_string());

        let outcome = engine
            .dispatch(HookEvent::SessionStart, Path::new("."), &ctx)
            .await
            .unwrap();

        assert!(outcome.allowed);
    }

    #[test]
    fn test_hook_output_spilling_when_exceeding_limit() {
        let spiller = HookOutputSpiller::with_limit("test_sess", 200);

        // Small output doesn't spill
        let (small, path) = spiller.maybe_spill("small_hook", "stdout", "Short text".to_string());
        assert_eq!(small, "Short text");
        assert!(path.is_none());

        // Large output spills to disk and returns preview
        let large_lines = (1..=100)
            .map(|i| format!("Line {}: Diagnostic event detail for hook execution", i))
            .collect::<Vec<_>>()
            .join("\n");

        let (preview, spilled_path) =
            spiller.maybe_spill("big_hook", "stdout", large_lines.clone());
        assert!(spilled_path.is_some());
        let path_buf = spilled_path.unwrap();
        assert!(path_buf.exists());

        // Verify file contains the full unabridged content
        let on_disk = fs::read_to_string(&path_buf).unwrap();
        assert_eq!(on_disk, large_lines);

        // Verify preview contains truncated notice and pointer to file
        assert!(preview.contains("Hook output truncated"));
        assert!(preview.contains(&path_buf.display().to_string()));
        assert!(preview.contains("Line 1:"));
        assert!(preview.contains("Line 100:"));

        // Clean up
        let _ = fs::remove_file(path_buf);
    }
}
