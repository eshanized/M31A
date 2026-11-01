# Getting Started with M31A

This walkthrough will guide you from zero to running your first autonomous coding workflows with **M31A**.

---

## 1. Prerequisites

### Rust Toolchain
M31A is built with modern Rust (2024 edition). Ensure you have Rust 1.85+ installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

### Linux Sandbox Dependencies
On Linux systems, M31A leverages **Bubblewrap** (`bwrap`) to isolate tool execution and filesystem mutations within unprivileged namespaces:

```bash
# Ubuntu / Debian
sudo apt update && sudo apt install -y bubblewrap

# Fedora / Red Hat
sudo dnf install -y bubblewrap

# Arch Linux
sudo pacman -S bubblewrap
```

*(Note: On macOS or Windows, Bubblewrap is skipped and executions run in user space or through custom sandbox profiles).*

### NVIDIA NIM API Key
M31A uses NVIDIA NIM frontier models (e.g. Nemotron, DeepSeek R1, Codestral).
1. Visit [build.nvidia.com](https://build.nvidia.com/).
2. Generate an API Key (prefixed with `nvapi-`).

---

## 2. Installation

### Option A: Install via Cargo (Recommended)

To build and install the release binary directly into `~/.cargo/bin/`:

```bash
# Clone the repository
git clone https://github.com/eshanized/M31A.git
cd M31A

# Build and install m31a
cargo install --path crates/m31a-cli
```

Verify your installation:

```bash
m31a --version
```

### Option B: Build Release Binary Manually

```bash
cargo build --release -p m31a-cli
# The binary is generated at target/release/m31a
cp target/release/m31a ~/.local/bin/
```

### Option C: Build with Bazel

```bash
bazel build //bazel:m31a
# The binary is generated at bazel-bin/bazel/m31a
```

---

## 3. Initial Configuration & Health Check

### Interactive Setup Wizard

Run the interactive setup wizard to configure your NVIDIA NIM credentials, choose your default model, and establish your preferred sandbox security level:

```bash
m31a setup
```

Alternatively, configure your environment directly:

```bash
export NVIDIA_API_KEY="nvapi-your-key-here"
```

Or create `~/.m31a/config.toml`:

```toml
model = "nvidia/nemotron-3.5-lightning-30b-a3b"
api_base = "https://integrate.api.nvidia.com/v1"
api_key = "nvapi-your-key-here"
sandbox = "workspace-write"
approval = "on-request"
```

### Verify System Health

Run the diagnostic doctor to confirm all dependencies, network connectivity, and sandbox features are ready:

```bash
m31a doctor
```

A healthy system will report:
```
[✓] NVIDIA NIM API Key: Configured
[✓] API Endpoint: https://integrate.api.nvidia.com/v1 (Reachability OK)
[✓] Bubblewrap Sandbox: /usr/bin/bwrap functional
[✓] Git Repository: Detected working tree
[✓] SQLite Storage: ~/.m31a/m31a.db (Active, Migrations current)
[✓] System Health: Ready
```

---

## 4. Interactive TUI Mode

Launch the full-screen terminal user interface:

```bash
# Launch interactive TUI in current directory
m31a

# Launch TUI pre-populated with an initial task
m31a "Examine the test suite and fix any broken assertions"
```

### Core TUI Concepts

1. **Agent Modes (`Tab` / `Shift+Tab`)**:
   - `[⚡ BUILD]`: Autonomous execution mode. The model inspects files, runs commands, applies multi-file patches, and iterates until the goal is achieved.
   - `[📋 PLAN]`: Read-only advisory mode. The model explores the codebase and proposes architectural plans without modifying files.

2. **Instant Shell Execution (`!` prefix)**:
   - Type `!cargo test` or `!git status` in the prompt composer to immediately run local commands and inject their output into your conversation context.

3. **Collapsible DeepSeek R1 / Nemotron Reasoning**:
   - When reasoning models generate thinking steps, the transcript renders them in a collapsible box. Toggle them anytime using `Ctrl+T`.

### Essential Keyboard Shortcuts

| Shortcut | Description |
| :--- | :--- |
| `Ctrl + P` | Open Universal Fuzzy Command Palette (quick tool & command launcher) |
| `Ctrl + M` | Open NVIDIA NIM Model Selector modal to switch models on the fly |
| `Ctrl + D` | Open interactive Git diff viewer showing working tree changes |
| `Ctrl + R` | Open Session Picker to resume or branch past conversations |
| `Ctrl + A` | Open Token & Cost Analytics dashboard |
| `Ctrl + T` | Fold or unfold reasoning and tool output blocks |
| `Ctrl + Y` | Copy latest code block from assistant to clipboard |
| `Ctrl + C` | Cancel active model streaming / double-press to exit |
| `?` | Show keyboard shortcuts overlay |

### Essential Slash Commands

Type `/` in the composer to access auto-completing slash commands:
- `/help`: Display complete reference guide.
- `/settings`: Interactive Settings Manager (General, Security, TUI, Features).
- `/model [slug]`: Switch NVIDIA NIM model immediately.
- `/diff`: View colorized diff of uncommitted changes.
- `/undo`: Revert the last turn's workspace modifications.
- `/verify`: Run compiler and linter checks on workspace.
- `/test`: Run full workspace test suite.
- `/review`: Conduct an automated code review on git changes.
- `/vibe`: Switch into continuous auto-healing vibe mode.
- `/compact`: Compact conversation history to reduce context window usage.

---

## 5. Non-Interactive CLI Workflows

For automation, CI pipelines, and scripting, M31A provides powerful CLI subcommands:

### Run Single Task (`m31a exec`)
```bash
m31a exec "Write comprehensive unit tests for crates/m31a-patch"
```

### Continuous Background Vibe Loop (`m31a vibe`)
```bash
# Watch filesystem, detect compiler/type errors, and automatically synthesize fixes
m31a vibe

# Combine with visual web testing
m31a vibe "Refactor navigation header" --visual-url http://localhost:3000
```

### Speculative Parallel Reality Arena (`m31a speculate`)
Run multiple competing implementation strategies in parallel isolated sandboxes:

```bash
m31a speculate "Implement SIMD-accelerated string search" \
  "Hypothesis A: AVX2 vector intrinsics" \
  "Hypothesis B: Portable std::simd" \
  --promote
```
The engine runs both approaches in temporary sandboxes, evaluates compiler output, test suites, and benchmarks, and automatically commits the winning branch.

### Automated Code Review (`m31a review`)
```bash
# Review changes compared to main branch
m31a review --base main
```

---

## 6. Next Steps

- Consult the [Configuration Guide](file:///home/snigdha/Desktop/org.m31a/docs/CONFIGURATION.md) for custom sandbox rules and model parameters.
- Explore the [CLI Command Reference](file:///home/snigdha/Desktop/org.m31a/docs/CLI_REFERENCE.md) for all 54 subcommands.
- Learn how to author custom skills in [Extensions, Skills & MCP Integration](file:///home/snigdha/Desktop/org.m31a/docs/EXTENSIONS_AND_SKILLS.md).
- Review [Sandbox & Security Architecture](file:///home/snigdha/Desktop/org.m31a/docs/SANDBOX_SECURITY.md) to understand isolation boundaries.
