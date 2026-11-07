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
- [**Autonomous Workflow Architecture**](architecture/AUTONOMOUS_WORKFLOW_ARCHITECTURE.md): Autonomous workflow orchestration with typed task kinds (`TaskKind`), phase transitions, and dependency fencing.
- [**Speculative Execution Architecture**](architecture/SPECULATIVE_EXECUTION_ARCHITECTURE.md): Speculative arena, multi-hypothesis exploration, and branch isolation.
- [**Execution Integrity Architecture (Round 2)**](architecture/EXECUTION_INTEGRITY_ROUND_2.md): Fail-closed runtime validation, hardened UDS socket binding, and turn boundary isolation.
- [**Execution Integrity Contract**](architecture/EXECUTION_INTEGRITY_CONTRACT.md): Formal verification contracts, tool permission boundaries, and invariants.
- [**Sandbox & Security Architecture**](SANDBOX_SECURITY.md): Deep dive into Linux Bubblewrap namespace isolation, read-only path masks, AST/prefix ExecPolicy, and the Guardian supervisor circuit breaker.
- [**App Server & JSON-RPC Protocol**](APP_SERVER_RPC.md): Specification of the 40+ JSON-RPC 2.0 methods and notification events for IDE extensions and external orchestrators.
- [**Multi-Agent Swarm & Distributed Mesh**](SWARM_AND_MULTI_AGENT.md): Multi-agent swarms with specialized roles (`Architect`, `Coder`, `Tester`, `Guardian`), cross-agent message boards, and path concurrency locks.
- [**Crates Directory Catalog**](../crates/README.md): Exhaustive index and architectural map of all 39 crates in the Cargo workspace.

### 📐 Formal Workflow & Speculative Specifications
- [**Workflow State Machine Specification**](specs/WORKFLOW_STATE_MACHINE.md): Formal workflow state transitions, events, guards, and phase sequencing.
- [**Workflow Storage Schema**](specs/WORKFLOW_STORAGE_SCHEMA.md): SQLite schema definitions for tasks, dependencies, checkpoints, and execution states.
- [**Speculative Evaluation Model**](specs/SPECULATIVE_EVALUATION_MODEL.md): Mathematical scoring model and ranking heuristics for speculative solution trees.

### 📋 Baseline Audits & Remediation Reports
- [**Round 3 Baseline Audit Report**](reports/ROUND_3_BASELINE_AUDIT.md): Comprehensive evaluation of execution integrity gaps, missing tests, and remediation priorities.
- [**Round 3 Implementation Report**](reports/ROUND_3_IMPLEMENTATION_REPORT.md): Remediation implementation log for execution integrity and architectural hardening.
- [**Round 3 Verification Report**](reports/ROUND_3_VERIFICATION_REPORT.md): Zero-mock verification evidence, automated regression test results, and release sign-off.
- [**Local-First Auth Removal & Remediation**](reports/LOCAL_FIRST_AUTH_REMOVAL_AND_REMEDIATION_REPORT.md): Architectural migration from token façades to authentic local-first resource validation.
- [**Deep Implementation Gap Remediation**](reports/DEEP_IMPLEMENTATION_GAP_REMEDIATION_REPORT.md): Deep subsystem wiring and runtime recovery audit.
- [**Logical Bug Remediation Report**](reports/LOGICAL_BUG_REMEDIATION_REPORT.md): Root-cause analyses and test matrices for discovered edge cases.

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
