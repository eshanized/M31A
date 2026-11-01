# M31A Documentation Hub

Welcome to the official documentation hub for **M31A**, the high-performance, autonomous agentic coding assistant powered by NVIDIA NIM frontier intelligence.

---

## Documentation Index

### 📘 User Guides
- [**Getting Started Walkthrough**](GETTING_STARTED.md): Step-by-step installation, NVIDIA NIM credential setup, first interactive session, and essential workflows.
- [**Terminal User Interface (TUI) User Guide**](TUI_GUIDE.md): Interactive features, keyboard shortcuts, modal dialogs, and slash commands.
- [**Configuration Guide**](CONFIGURATION.md): In-depth guide to `~/.m31a/config.toml`, profiles, feature flags, and approval presets.
- [**CLI Command Reference**](CLI_REFERENCE.md): Complete exhaustive reference of all 54 CLI subcommands, options, and shell environment variables.
- [**Extensions, Skills & MCP Integration**](EXTENSIONS_AND_SKILLS.md): Authoring custom skills with YAML frontmatter, instruction templates, and integrating external Model Context Protocol (MCP) servers.

### 🛠️ Developer & Architecture Manuals
- [**System Architecture**](ARCHITECTURE.md): Deep architectural manual covering the multi-turn agent engine, NVIDIA NIM SSE streaming, 18-tool suite, and the 39-crate workspace.
- [**Sandbox & Security Architecture**](SANDBOX_SECURITY.md): Deep dive into Linux Bubblewrap namespace isolation, read-only path masks, AST/prefix ExecPolicy, and the Guardian supervisor circuit breaker.
- [**App Server & JSON-RPC Protocol**](APP_SERVER_RPC.md): Specification of the 40+ JSON-RPC 2.0 methods and notification events for IDE extensions and external orchestrators.
- [**Multi-Agent Swarm & Distributed Mesh**](SWARM_AND_MULTI_AGENT.md): Multi-agent swarms with specialized roles (`Architect`, `Coder`, `Tester`, `Guardian`), cross-agent message boards, and path concurrency locks.
- [**Crates Directory Catalog**](../crates/README.md): Exhaustive index and architectural map of all 39 crates in the Cargo workspace.

### 🖥️ TUI Architecture & Developer Specifications
- [**TUI Architecture Specification**](tui/TUI-ARCHITECTURE.md): Event loops, Ratatui render pipelines, and widget trees.
- [**Transcript Rendering Architecture**](tui/TRANSCRIPT-RENDERING-ARCHITECTURE.md): Streaming markdown rendering and collapsible DeepSeek R1 reasoning paths.
- [**Model Selector Specification**](tui/MODEL-SELECTOR-SPEC.md): Dynamic modal dialog querying `/v1/models` on NVIDIA NIM.
- [**Approval Interaction Specification**](tui/APPROVAL-INTERACTION-SPEC.md): Human-in-the-loop modal dialogs for mutating tool executions.
- [**TUI State Machines**](tui/TUI-STATE-MACHINES.md): Formal state machines governing mode switches, modal overlays, and event channels.
- [**TUI Session Lifecycle**](tui/TUI-SESSION-LIFECYCLE.md): Session lifecycle from launch through turn execution and shutdown.
- [**TUI Interaction Matrix**](tui/TUI-INTERACTION-MATRIX.md): Action dispatch matrix mapping keyboard and mouse events to state transitions.

### 📦 Client SDKs
- [**Python SDK (`m31a`)**](../sdk/python/README.md): Native async and synchronous Python client with zero external dependencies.
- [**TypeScript SDK (`@m31a/sdk`)**](../sdk/typescript/README.md): Strongly typed Node.js and browser client supporting streaming events and multiple transports.
