# Quickstart Tutorial

This guide walks you through running your first autonomous coding mission with M31A.

---

## 1. Setting Up Provider Credentials

M31A production supports **NVIDIA NIM only** (`nvidia_nim`). The
`openai` / `anthropic` / `gemini` / local-Ollama provider IDs are retired and
rejected deterministically — only NVIDIA credentials are accepted.

Store your NVIDIA NIM API key with the masked first-run setup wizard. The
wizard writes to the canonical channel-isolated credential store (stored
with `0600` permissions). The production runtime never reads API keys from
environment variables or `.env` files:

```bash
# First-run onboarding (masked credential entry + model discovery)
m31a init
```

> Explicitly opted-in live-model *tests* (under `tests/`) may load a local
> `.env` via `tests/support/` helpers. That path is test-only and never
> configures the production runtime.

---

## 2. Initializing a Workspace Configuration

In your project repository, initialize a local `.m31a` configuration:

```bash
cd /path/to/your/project
mkdir -p .m31a
```

Create `.m31a/config.toml` (or copy from [`examples/config.example.toml`](../../examples/config.example.toml)):

```toml
profile = "coding"

[agents]
default_model = "meta/llama-3.1-70b-instruct"

[policy]
default_action = "ask"
```

The configured model must be an NVIDIA NIM hosted model (bare IDs and
`nvidia/`-qualified IDs resolve to NVIDIA NIM; retired provider qualifiers
are rejected). Run model discovery during onboarding (or refresh the
catalog) so the selection validates against live provider data: an empty
catalog never invents models, and model-dependent execution fails closed
until a suitable model is explicitly selected or discovered.

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
