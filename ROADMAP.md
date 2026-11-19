# M31A Roadmap

This document outlines the architectural roadmap and strategic vision for **M31A**.

---

## Strategic Vision

M31A is built to be the premier open-source, sovereign, enterprise-grade autonomous coding assistant. By combining NVIDIA NIM low-latency frontier models, strict Linux sandbox isolation, verifiable AST patch generation, and multi-agent coordination, M31A delivers high-trust AI engineering without vendor lock-in.

---

## Phase 1: Baseline Architecture & Release Readiness (v0.1.0) — :white_check_mark: Completed

- [x] **Modular 39-Crate Monorepo**: Decoupled domain architecture in Rust 2024 edition.
- [x] **NVIDIA NIM Frontier Integration**: SSE streaming, dynamic `/v1/models` discovery, and DeepSeek R1 / Nemotron reasoning chunk support.
- [x] **Full-Featured Terminal UI**: Ratatui/Crossterm TUI with fuzzy palette, diff viewer, model switcher, and slash commands.
- [x] **Codex Multi-File Patch Engine**: 4-tier fuzzy match engine (`seek_sequence`) with whitespace and unicode normalization.
- [x] **Multi-Layer Security Model**: Linux Bubblewrap isolation, read-only masks, AST/prefix ExecPolicy, and Guardian supervisor.
- [x] **Vibe Coding Engine & Speculative Arena**: Continuous background compilation/auto-healing loop and parallel hypothesis sandboxes.
- [x] **Official Client SDKs**: First-party TypeScript (`@m31a/sdk`) and Python (`m31a`) packages with multi-transport JSON-RPC 2.0 support.
- [x] **Multi-Platform Build Matrix**: Cargo workspace, Nextest test runner, and Just / cargo-dist release configuration covering Linux, macOS, and Windows.

---

## Phase 2: IDE Ecosystem & Native Integrations (v0.2.0) — :construction: In Progress

- [ ] **VS Code Official Extension**: First-party extension leveraging `m31a app-server` via UDS/stdio, featuring inline diff reviews and floating composer.
- [ ] **JetBrains Plugin Suite**: IntelliJ, PyCharm, and CLion support via JSON-RPC daemon.
- [ ] **Language Server Protocol (LSP) Bridge**: Bidirectional communication with `rust-analyzer`, `pyright`, and `ts-server` for real-time symbol definitions and diagnostic feeds.
- [ ] **Distributed Task Mesh Evolution**: Multi-host distributed swarm coordination using end-to-end encrypted peer-to-peer transport (`m31a pair`).
- [ ] **Persistent Memory Graphs**: Vector-indexed and SQLite-backed cross-session memory recalling past architectural decisions and patterns across repositories.

---

## Phase 3: Multi-Modal Verification & Extended Sandboxing (v0.3.0) — :construction: Security remediation in progress

- [x] **Visual comparison foundation**: Multi-resolution screenshot comparison, heatmaps, region masking, and SHA-256 baseline integrity checks.
- [ ] **Autonomous visual repair workflow**: Current code provides repair prompts, checkpoints, approval/attempt guards, and rollback primitives; runtime-integrated patch application and verified retry orchestration remain incomplete.
- [ ] **eBPF-Based Execution Auditing**: The current crate processes caller-supplied events only. Kernel program loading, tracepoint attachment, kernel ring-buffer consumption, and runtime event-source integration are not implemented.
- [ ] **Cross-Platform Sandboxing**:
  - [x] Linux: Bubblewrap namespaces, Landlock filesystem bounds, and Seccomp filtering (subject to host capability detection).
  - [ ] macOS: Seatbelt profile generation exists, but `sandbox-exec` is legacy/deprecation-sensitive; per-destination egress policy and native OS integration tests remain outstanding.
  - [ ] Windows: AppContainer execution is disabled until restricted process creation is implemented. Restricted policies require a verified MXC backend; otherwise execution fails closed.
  - [x] Unified `SandboxPlatform` capability reporting and policy abstraction (does not imply every backend is implemented).
- [x] **WebAssembly Plugin Foundation**: `wasmi` ABI v1, capability declarations, per-invocation fuel, store-level memory limits, component-aware path grants, and bounded host reads. Wall-clock interruption, signed manifests, and comprehensive adversarial validation remain outstanding.

---

## Phase 4: Autonomous Repository Stewardship (v1.0.0+) — :telescope: Long-term

- [ ] **Zero-Babysitter Continuous Upgrades**: Autonomous dependency migration, deprecation removal, and security patch verification with zero human intervention.
- [ ] **Autonomous Formal Verification**: Integration with formal proof engines (Kani, Creusot, Coq) for mathematically verified code synthesis.
- [ ] **Enterprise Policy Enforcement**: Federated policy servers managing allowed models, data leakage prevention, and compliance audits across organization fleets.
