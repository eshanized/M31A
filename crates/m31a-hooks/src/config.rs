//! Workspace and user `hooks.json` configuration file schema and loader.
//!
//! Shape:
//! ```json
//! {
//!   "PreToolUse": [
//!     { "matcher": "Bash|Edit",
//!       "hooks": [ { "type": "command", "command": "guard.sh", "timeout_seconds": 10 } ] }
//!   ]
//! }
//! ```
//! Event keys accept both snake_case (`pre_tool_use`) and Claude-style
//! CamelCase (`PreToolUse`). `type: "mcp"` handlers are parsed but not yet
//! executable (MCP client surface is deferred to §7.8).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::events::HookEvent;

#[derive(Debug, Clone)]
pub struct HookConfig {
    /// Per-event rules in declaration order.
    pub rules: Vec<HookRule>,
}

#[derive(Debug, Clone)]
pub struct HookRule {
    /// Event this rule applies to.
    pub event: HookEvent,
    /// Matcher over the event's subject (e.g. tool name). `None`/empty
    /// matches everything. Regex syntax (Claude Code parity).
    pub matcher: Option<String>,
    /// Handlers to run in order for a matched event.
    pub handlers: Vec<HookHandler>,
}

#[derive(Debug, Clone)]
pub struct HookHandler {
    pub kind: HandlerKind,
    /// Command to run for `Command` handlers.
    pub command: String,
    /// Per-handler timeout in seconds (default 10).
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerKind {
    Command,
    Mcp { server: String, tool: String },
}

#[derive(Debug, Deserialize)]
struct RawHookFile {
    #[serde(flatten)]
    entries: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawHookEntry {
    #[serde(default)]
    matcher: Option<String>,
    hooks: Vec<RawHandler>,
}

#[derive(Debug, Deserialize)]
struct RawHandler {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    command: Option<String>,
    /// MCP handler target (deferred — requires the MCP client surface).
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

impl HookConfig {
    /// Empty configuration (no hooks).
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Parses a Claude-style hooks document.
    pub fn from_json(text: &str) -> Result<Self> {
        let raw: RawHookFile = serde_json::from_str(text).context("Failed to parse hooks.json")?;

        let mut rules = Vec::new();
        for (key, value) in raw.entries {
            let event = HookEvent::from_config_key(&key)
                .with_context(|| format!("Unknown hook event key `{key}`"))?;
            let entries: Vec<RawHookEntry> = serde_json::from_value(value)
                .with_context(|| format!("Invalid hook entries for `{key}`"))?;
            for entry in entries {
                let mut handlers = Vec::new();
                for h in entry.hooks {
                    let kind = match h.kind.as_str() {
                        "command" => {
                            if h.command.is_none() {
                                anyhow::bail!("Command hook for `{key}` missing `command`");
                            }
                            HandlerKind::Command
                        }
                        "mcp" => {
                            let server = h.server.clone().unwrap_or_default();
                            let tool = h.tool.clone().unwrap_or_default();
                            if server.is_empty() || tool.is_empty() {
                                anyhow::bail!(
                                    "MCP hook for `{key}` needs `server` and `tool` fields"
                                );
                            }
                            HandlerKind::Mcp { server, tool }
                        }
                        other => anyhow::bail!("Unknown hook handler type `{other}` in `{key}`"),
                    };
                    handlers.push(HookHandler {
                        kind,
                        command: h.command.clone().unwrap_or_default(),
                        timeout_seconds: h.timeout_seconds.unwrap_or(10).clamp(1, 600),
                    });
                }
                rules.push(HookRule {
                    event,
                    matcher: entry.matcher.filter(|m| !m.trim().is_empty()),
                    handlers,
                });
            }
        }

        // serde_json (without preserve_order) gives us a HashMap: object key
        // order is lost. Sort rules deterministically so dispatch order is
        // stable across runs (per-rule handler order IS preserved).
        rules.sort_by(|a, b| {
            (a.event.config_key(), a.matcher.clone().unwrap_or_default())
                .cmp(&(b.event.config_key(), b.matcher.clone().unwrap_or_default()))
        });

        Ok(Self { rules })
    }

    /// Discovers hooks config: `<cwd>/.m31a/hooks.json` (project) merged over
    /// `~/.m31a/hooks.json` (user). Missing files are not errors.
    pub fn discover(cwd: &Path) -> Self {
        let mut config = Self::empty();
        if let Some(home) = std::env::var_os("HOME") {
            let p = Path::new(&home).join(".m31a").join("hooks.json");
            if let Ok(text) = std::fs::read_to_string(&p) {
                match Self::from_json(&text) {
                    Ok(c) => config.rules.extend(c.rules),
                    Err(e) => tracing::warn!("Invalid user hooks config {}: {e}", p.display()),
                }
            }
        }
        let project = cwd.join(".m31a").join("hooks.json");
        if let Ok(text) = std::fs::read_to_string(&project) {
            match Self::from_json(&text) {
                Ok(c) => config.rules.extend(c.rules),
                Err(e) => tracing::warn!("Invalid project hooks config {}: {e}", project.display()),
            }
        }
        config
    }

    /// Rules for an event whose matcher covers `subject`.
    pub fn matched_rules(&self, event: HookEvent, subject: &str) -> Vec<&HookRule> {
        self.rules
            .iter()
            .filter(|r| r.event == event)
            .filter(|r| match &r.matcher {
                None => true,
                Some(m) => regex::Regex::new(m)
                    .map(|re| re.is_match(subject))
                    .unwrap_or_else(|e| {
                        tracing::warn!("Invalid hook matcher regex `{m}`: {e}");
                        false
                    }),
            })
            .collect()
    }
}

impl HookEvent {
    /// Parses a config key in either snake_case or Claude-style CamelCase.
    pub fn from_config_key(key: &str) -> Option<Self> {
        let snake: String = key
            .chars()
            .enumerate()
            .fold(String::new(), |mut acc, (i, c)| {
                if c.is_uppercase() && i > 0 {
                    acc.push('_');
                }
                acc.push(c.to_ascii_lowercase());
                acc
            });
        [
            "pre_model_request",
            "post_model_request",
            "pre_tool_use",
            "permission_request",
            "post_tool_use",
            "pre_compact",
            "post_compact",
            "session_start",
            "session_end",
            "user_prompt_submit",
            "subagent_start",
            "subagent_stop",
            "stop",
            "interrupt",
        ]
        .iter()
        .position(|k| *k == snake)
        .map(|idx| {
            [
                HookEvent::PreModelRequest,
                HookEvent::PostModelRequest,
                HookEvent::PreToolUse,
                HookEvent::PermissionRequest,
                HookEvent::PostToolUse,
                HookEvent::PreCompact,
                HookEvent::PostCompact,
                HookEvent::SessionStart,
                HookEvent::SessionEnd,
                HookEvent::UserPromptSubmit,
                HookEvent::SubagentStart,
                HookEvent::SubagentStop,
                HookEvent::Stop,
                HookEvent::Interrupt,
            ][idx]
        })
    }
}
