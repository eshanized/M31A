# M31A Crates Workspace

This directory contains the 39 modular domain crates comprising the **M31A** architecture. The codebase is organized according to clean architectural separation of concerns, strict dependency layering, and uncompromising zero-mock testing standards.

---

## Architectural Layers

```
+-----------------------------------------------------------------------+
|                             m31a-cli                                  |
|               (Unified Entrypoint & 54 CLI Subcommands)               |
+-----------------------------------+-----------------------------------+
                                    |
          +-------------------------+-------------------------+
          |                                                   |
+---------v---------+                               +---------v---------+
|     m31a-tui      |                               |  m31a-app-server  |
|  (Ratatui Terminal|                               |  (JSON-RPC 2.0    |
|   User Interface) |                               |   Daemon Server)  |
+---------+---------+                               +---------+---------+
          |                                                   |
          +-------------------------+-------------------------+
                                    |
+-----------------------------------v-----------------------------------+
|                          m31a-agent-engine                            |
|    (Turn Execution Loop, Vibe Loop, Speculative Parallel Arena, Swarm)|
+---+--------------------+--------------------+--------------------+----+
    |                    |                    |                    |
+---v-------------+ +----v------------+ +-----v-----------+ +------v----+
|  m31a-provider  | |   m31a-verify   | |   m31a-tools    | |  m31a-    |
|  (NVIDIA NIM    | |  (Visual TDD,   | |  (18 Tools &    | |  guardian |
|   SSE Streaming)| |   Blast Radius) | |   Codex Patch)  | |  (Review) |
+-----------------+ +-----------------+ +-----------------+ +-----------+
    |                    |                    |                    |
+---v--------------------v--------------------v--------------------v----+
|                m31a-sandbox & m31a-execpolicy                         |
|      (Bubblewrap Linux Namespaces, AST Prefix & Network Rules)        |
+-----------------------------------+-----------------------------------+
                                    |
+-----------------------------------v-----------------------------------+
|                    m31a-storage & m31a-core                           |
|       (SQLite Threads & Pub-Sub Board, Data Models & Skills)          |
+-----------------------------------------------------------------------+
```

---

## Crate Inventory by Subsystem

### 1. Application Binary & Daemon
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-cli`](file:///home/snigdha/Desktop/M31A/crates/m31a-cli) | Main application entrypoint exposing 55 subcommands including autonomous workflow commands (`m31a workflow run/status/inspect/resume/cancel/events/artifacts`), argument parsing, shell completions, and system diagnostics (`m31a doctor`). | `clap`, `tokio`, all subsystem crates |
| [`m31a-app-server`](file:///home/snigdha/Desktop/M31A/crates/m31a-app-server) | JSON-RPC 2.0 daemon facilitating IPC over stdio, Unix Domain Socket (`app-server-control.sock`), and HTTP/SSE with autonomous workflow and speculative evaluation endpoints. | `tokio`, `serde_json`, `m31a-storage`, `m31a-agent-engine` |

### 2. User Interface & Terminal Experience
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-tui`](file:///home/snigdha/Desktop/M31A/crates/m31a-tui) | Rich terminal UI featuring streaming transcript rendering, modals, model switcher, diff viewer, slash commands, workflow DAG monitoring, and speculative candidate matrices on Screen 5. | `ratatui`, `crossterm`, `m31a-composer`, `m31a-agent-engine` |
| [`m31a-composer`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-composer) | Multiline prompt composer with syntax highlighting, slash-command autocompletion, and prompt history navigation. | `m31a-fuzzy`, `ratatui` |
| [`m31a-term`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-term) | Terminal raw mode handling, escape sequences, color palettes, and alternate screen buffer management. | `crossterm`, `colored` |
| [`m31a-fuzzy`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-fuzzy) | High-performance fuzzy matching algorithms used for command palettes, file searches, and prompt matching. | `unicode-segmentation` |
| [`m31a-diff`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-diff) | Syntactic unified diff computation, side-by-side formatting, and patch previews. | `similar`, `colored` |
| [`m31a-analytics`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-analytics) | Session token accounting, latency metrics, and real-time model invocation pricing. | `serde`, `chrono` |

### 3. Agent Execution & Intelligence
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-agent-engine`](file:///home/snigdha/Desktop/M31A/crates/m31a-agent-engine) | Autonomous workflow orchestrator, multi-turn agent loop, background vibe coding auto-healing loop, genuine speculative parallel reality arena, and wave scheduler. | `m31a-provider`, `m31a-tools`, `m31a-sandbox`, `m31a-verify`, `m31a-ast` |
| [`m31a-provider`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-provider) | NVIDIA NIM HTTP/SSE streaming integration, dynamic `/v1/models` catalog discovery, and DeepSeek R1 reasoning stream parsing. | `reqwest`, `eventsource`, `serde_json` |
| [`m31a-elicitation`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-elicitation) | Interactive clarification questions engine for user preference elicitation. | `serde`, `m31a-core` |
| [`m31a-review`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-review) | Automated Git diff reviewer synthesizing prioritized code improvements and security alerts. | `m31a-diff`, `m31a-provider` |
| [`m31a-image-generation`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-image-generation) | Multimodal image generation client and visual asset synthesis pipeline. | `reqwest`, `serde` |
| [`m31a-web-search`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-web-search) | Real-time web search retrieval client for documentation and developer reference. | `reqwest`, `serde_json` |
| [`m31a-prompts`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-prompts) | Model-specific system prompt builders, tool instructions, and context injection templates. | `serde` |

### 4. Verification & Static Analysis
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-verify`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-verify) | Workspace compiler check, test runner orchestration, visual TDD engine, and semantic blast-radius gate. | `m31a-browser`, `m31a-ast`, `m31a-shell` |
| [`m31a-browser`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-browser) | Headless Chromium CDP (Chrome DevTools Protocol) controller for visual inspection, element assertions, and screenshots. | `tokio`, `serde_json` |
| [`m31a-ast`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-ast) | Abstract Syntax Tree symbol extraction, call-graph analysis, and data-flow taint tracking. | `regex`, `serde` |

### 5. Security & Isolation
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-sandbox`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-sandbox) | Linux Bubblewrap isolation manager, namespace unsharing, and read-only protection masks (`.git`, `.m31a`, `.env`, `.ssh`). | `which`, `tokio` |
| [`m31a-execpolicy`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-execpolicy) | Command prefix rule matching, alternative token sets (`PatternToken::Alts`), and network destination filtering. | `serde`, `shlex` |
| [`m31a-guardian`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-guardian) | Autonomous review supervisor evaluating destructive actions with sliding-window circuit breaker. | `m31a-execpolicy`, `m31a-provider` |
| [`m31a-trust`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-trust) | Workspace trust boundary enforcement and Git hook execution armor. | `m31a-core` |

### 6. Tools, Patching & Execution Runtime
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-tools`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-tools) | 18-tool suite (filesystem operations, command execution, agent orchestration, goal management). | `m31a-patch`, `m31a-sandbox`, `m31a-shell` |
| [`m31a-patch`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-patch) | Codex multi-file patch specification parser and 4-tier fuzzy match applicator (`seek_sequence`). | `similar`, `unicode-segmentation` |
| [`m31a-code-mode`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-code-mode) | Code Mode runtime executing scripts with `@exec` pragmas, cell yields, and nested tool calls. | `serde_json`, `m31a-tools` |
| [`m31a-shell`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-shell) | Subprocess execution abstraction, stream multiplexing, and terminal escape stripping. | `tokio` |
| [`m31a-proc`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-proc) | Background process supervisor, dev server management, and tail log ring buffers. | `tokio`, `sysinfo` |

### 7. Core Foundations, Storage & Extensibility
| Crate | Purpose | Key Dependencies |
| :--- | :--- | :--- |
| [`m31a-core`](file:///home/snigdha/Desktop/M31A/crates/m31a-core) | Foundational data structures, workflow domain models (DAG validation, wave partitioning, events, artifacts), session configuration, thread state, and skills frontmatter loader. | `serde`, `toml`, `dirs` |
| [`m31a-storage`](file:///home/snigdha/Desktop/M31A/crates/m31a-storage) | Embedded SQLite persistence layer (Migration 15) for workflows, plans, tasks with worker leases, append-only events, content-addressed artifacts, threads, checkpoints, and path locks. | `rusqlite`, `tokio`, `sha2` |
| [`m31a-features`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-features) | Runtime feature flag registry and experimental capability toggles. | `serde`, `clap` |
| [`m31a-compaction`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-compaction) | Context window management, conversation summarization, and turn pruning. | `m31a-core` |
| [`m31a-telemetry`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-telemetry) | System and session telemetry collection, audit trails, and performance profiling. | `tracing`, `serde` |
| [`m31a-watcher`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-watcher) | Debounced filesystem watcher monitoring workspace edits for continuous vibe coding. | `notify` |
| [`m31a-extension-api`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-extension-api) | Plugin API interfaces, extension manifest schemas, and capability discovery. | `serde`, `async-trait` |
| [`m31a-hooks`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-hooks) | Pre- and post-turn lifecycle hooks, tool execution intercepts, and workspace triggers. | `m31a-core` |
| [`m31a-mcp`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-mcp) | Model Context Protocol (MCP) client managing external stdio and HTTP tool providers. | `serde_json`, `tokio` |
| [`m31a-collaboration-templates`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-collaboration-templates) | Pre-configured team orchestration patterns and multi-agent collaboration setups. | `serde`, `toml` |
| [`m31a-git-attribution`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-git-attribution) | Git commit trailer generator appending AI co-authorship metadata (`Co-authored-by`). | `git2` |
| [`m31a-cloud-tasks`](file:///home/snigdha/Desktop/org.m31a/crates/m31a-cloud-tasks) | Remote task execution client delegating heavy builds or benchmarks to cloud nodes. | `reqwest`, `tokio` |
