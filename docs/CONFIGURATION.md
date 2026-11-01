# M31A Configuration Guide

This guide describes how to configure **M31A** across environments, including user configuration files, workspace settings, profiles, feature flags, and security policies.

---

## Configuration File Hierarchy

When M31A initializes, it resolves configuration settings in the following order of precedence (highest priority first):

1. **CLI Flags & Overrides**: Arguments supplied via `--model`, `--sandbox`, `-c key=value`.
2. **Profile Configuration**: Settings loaded via `-p <profile>` (e.g. `~/.m31a/profiles/<profile>.toml`).
3. **Workspace Configuration**: Local settings placed in `<workspace-root>/.m31a/config.toml`.
4. **User Global Configuration**: Global settings at `~/.m31a/config.toml`.
5. **Environment Variables**: `NVIDIA_API_KEY`, `NVIDIA_API_BASE`, `M31A_SANDBOX`, etc.
6. **Built-in Defaults**.

---

## Annotated Configuration Example

Save this file as `~/.m31a/config.toml` or `.m31a/config.toml`:

```toml
# ==============================================================================
# Model & Provider Configuration
# ==============================================================================

# NVIDIA NIM Model Slug
# M31A dynamically discovers supported models from /v1/models.
# Popular options:
# - "nvidia/nemotron-3.5-lightning-30b-a3b" (Default frontier workhorse)
# - "nvidia/nemotron-3.5-bolt-e4b" (Low-latency instruction follower)
# - "deepseek-ai/deepseek-v4.1-flash" (Frontier reasoning with thinking stream)
# - "mistralai/codestral-22b-instruct-v0.1" (Code specialization)
# - "nvidia/llama-3.1-nemotron-70b-instruct" (Tool-calling precision)
model = "nvidia/nemotron-3.5-lightning-30b-a3b"

# Base URL for NVIDIA NIM or custom OpenAI-compatible endpoint
api_base = "https://integrate.api.nvidia.com/v1"

# NVIDIA NIM API Key (or export NVIDIA_API_KEY in your shell environment)
# api_key = "nvapi-..."

# Maximum token output per generation turn
max_tokens = 8192

# Sampling temperature (0.0 = deterministic code, 0.7 = creative planning)
temperature = 0.2

# ==============================================================================
# Security & Sandboxing Policies
# ==============================================================================

# Sandbox Policy:
# - "read-only": Tools can read filesystem but cannot modify any files.
# - "workspace-write" (Default): Tools can write inside workspace directory;
#   root filesystem and sensitive subpaths (.git, .ssh, .env) are protected.
# - "danger-full-access": Bypasses Bubblewrap isolation (use with caution).
sandbox = "workspace-write"

# Approval Policy:
# - "on-request" (Default): Model decides when to prompt human for mutating actions.
# - "never": Never prompt human; errors and security blocks return to model.
# - "approve-for-me": Automated review supervisor (Guardian) validates actions.
approval = "on-request"

# Network namespace policy inside sandbox:
# Set to true to disable all internet access inside the Bubblewrap sandbox.
airgapped_sandbox = false

# ==============================================================================
# Daemon & Service Settings
# ==============================================================================

# Auto-start JSON-RPC 2.0 daemon in background if not already running
auto_start_daemon = true

# Path to Unix Domain Socket for local IPC
# socket_path = "~/.m31a/app-server-control/app-server-control.sock"

# ==============================================================================
# Feature Flags
# ==============================================================================
[features]
# Feature keys must match valid FeatureKey definitions in m31a-features.
code_mode = true
guardian_v2 = true
apply_patch_streaming = true
unified_exec = true
worktree_management = true
multi_agent_v2 = true
visual_tdd = true

# ==============================================================================
# Terminal User Interface (TUI) Settings
# ==============================================================================
[tui]
# Color theme: "default", "catppuccin-mocha", "tokyo-night", "nord", "monokai"
theme = "default"

# Show DeepSeek R1 / Nemotron thinking reasoning block expanded by default
expand_reasoning = false

# Enable smooth scroll animations
smooth_scrolling = true

# Mouse support enabled
mouse_capture = true

# ==============================================================================
# External Model Context Protocol (MCP) Servers
# ==============================================================================
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/shared/docs"]

# [mcp_servers.sqlite]
# command = "uvx"
# args = ["mcp-server-sqlite", "--db-path", "analytics.db"]
```

---

## Security Presets

M31A includes predefined security presets that configure sandbox and approval rules simultaneously using `--preset <PRESET>`:

| Preset Name | Sandbox Mode | Approval Mode | Network | Intended Use Case |
| :--- | :--- | :--- | :--- | :--- |
| `read-only` | `read-only` | `never` | Blocked | Safe codebase exploration and code review without risk of edits. |
| `standard` *(default)* | `workspace-write` | `on-request` | Allowed | Normal day-to-day interactive coding with human-in-the-loop approvals. |
| `permissive` | `workspace-write` | `approve-for-me` | Allowed | High-speed prototyping with Guardian autonomous safety checks. |
| `airgapped` | `workspace-write` | `on-request` | **Disabled** | Highly secure environments with sensitive intellectual property. |
| `danger-full-access` | `danger-full-access` | `never` | Allowed | Unrestricted system maintenance in disposable CI/Docker containers. |

---

## Workspace Profiles

Profiles allow sharing common configurations across team members or projects. Store profiles in `~/.m31a/profiles/<name>.toml`:

```bash
# Apply a security-hardened enterprise profile
m31a -p enterprise "Refactor the authentication module"
```

---

## Validating Configuration

Verify your configuration syntax and active runtime values using:

```bash
m31a doctor
m31a features list
```
