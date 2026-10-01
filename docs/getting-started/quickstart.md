# Quickstart Tutorial

This guide walks you through running your first autonomous coding mission with M31A.

---

## 1. Setting Up Provider Credentials

M31A supports OpenAI, Anthropic, Gemini, NVIDIA, and OpenAI-compatible local providers (e.g. Ollama, vLLM).

Configure your preferred provider key via environment variables:

```bash
# For Anthropic Claude
export ANTHROPIC_API_KEY="sk-ant-..."

# For OpenAI
export OPENAI_API_KEY="sk-..."

# For Google Gemini
export GEMINI_API_KEY="..."

# For NVIDIA NIM
export NVIDIA_API_KEY="nvapi-..."
```

Alternatively, save credentials to the local secure credential registry (stored with `0600` permissions):

```bash
m31a config set-credential anthropic "sk-ant-..."
```

---

## 2. Initializing a Workspace Configuration

In your project repository, initialize a local `.m31a` configuration:

```bash
cd /path/to/your/project
mkdir -p .m31a
```

Create `.m31a/config.toml` (or copy from [`examples/config.example.toml`](../../examples/config.example.toml)):

```toml
[runtime]
profile = "coding"
default_model = "claude-3-5-sonnet-20241022"

[policy]
default_decision = "ask"
require_clean_git_workspace = true

[limits]
max_wall_clock_seconds = 600
max_tokens = 500000
```

Validate your configuration with:

```bash
m31a config validate
```

---

## 3. Running an Autonomous Mission

Launch an autonomous mission with a descriptive goal:

```bash
m31a mission run "Refactor database query to use parameterized statements and add unit tests" --profile coding
```

The runtime will:
1. **Intake & Validate**: Establish baseline Git state and compile workspace context into XML trust envelopes.
2. **Plan**: Decompose the mission into a directed acyclic graph (DAG) of discrete tasks.
3. **Execute**: Spawn role-specific agents (`Planner`, `Coder`, `Tester`, `SecurityAuditor`) under strict capability sandboxes.
4. **Verify**: Run automated compiler, linter, and unit test suites to confirm correctness.
5. **Report**: Emit cryptographic hashes of all modifications and commit candidates.

---

## 4. Launching the Interactive TUI Cockpit

For real-time visibility into the execution DAG, active agent roles, tool invocations, and live terminal output, run:

```bash
m31a tui
```

### Keyboard Shortcuts in Cockpit

| Key | Action |
|:---|:---|
| `Tab` | Switch between timeline, task graph, and inspector panels |
| `j` / `k` / `Down` / `Up` | Scroll through tasks or log events |
| `Space` | Expand task node details |
| `a` | Approve pending ASK policy prompt |
| `d` | Deny pending ASK policy prompt |
| `q` / `Ctrl+C` | Gracefully detach or exit |

---

## 5. Reviewing Telemetry & Checkpoints

Inspect the execution history and performance breakdown of any mission:

```bash
# List recent missions
m31a mission list

# Inspect detailed mission telemetry and traces
m31a telemetry inspect <mission-id> --summary --spans --metrics

# List durable checkpoints
m31a checkpoint list --mission-id <mission-id>
```
