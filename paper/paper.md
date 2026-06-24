---
title: "M31 Autonomous: A Terminal-Native AI Coding Agent with Eight-Phase Workflow Orchestration"
tags:
  - Go
  - AI coding agent
  - workflow orchestration
  - terminal UI
  - large language models
  - software engineering automation
authors:
  - name: Eshan Roy
    orcid: 0009-0007-1261-6805
    affiliation: 1
affiliations:
  - name: Independent Researcher
    index: 1
date: 22 June 2026
bibliography: paper.bib
---

# Summary

M31 Autonomous is a terminal-native AI coding agent, written entirely in Go, that
orchestrates an eight-phase software engineering workflow end-to-end. Beyond its
practical utility as a development tool, M31A serves as a research platform for
computational software engineering, hosting novel algorithms with formal
mathematical foundations and enabling reproducible research workflows through its
built-in research phase and cross-session learning ledger. From project
initialization through discussion, planning, execution, verification, runtime smoke
testing, and shipping, every run concludes with a verified git commit and a
cross-session learning record. The system is distributed as a static binary
(15-20 MB) with zero runtime dependencies and zero telemetry.

The architecture follows a strict layered design: a 33-screen terminal UI built on
Bubble Tea [@bubbletea], an eight-phase workflow engine, a provider abstraction
layer supporting multiple LLM backends with automatic fallback, a permission-gated
tool system with 17 registered tools, a composable skills framework, and a
concurrent session coordinator with demand coalescing.

Key innovations include: (1) a complexity-adaptive workflow engine that classifies
user goals into trivial, simple, moderate, or complex categories and selects an
appropriate phase subset, avoiding unnecessary overhead for simple tasks; (2) an
AutoDream context consolidation system that compresses older messages into compact
memory segments while protecting system prompts, tool calls, and recent context;
(3) an automatic session compaction engine that triggers LLM-driven summarization
when token usage approaches the model's context window limit; (4) a runtime
verification phase that starts a development server, discovers routes from goal
text and filesystem conventions, and runs concurrent HTTP smoke tests to catch
errors that static checks miss; and (5) a cross-session learning ledger that
records goal, task outcomes, file patterns, model used, and duration for every
shipped session, enabling the agent to learn from its own history.

The workflow engine implements phase transition guards and a plan-discuss
oscillation guard (capped at three cycles) to prevent infinite refinement loops.
The plan parser uses cascading regex strategies with fallback JSON extraction,
validated by duplicate detection, cycle detection via iterative DFS, and
granularity checks. The execute phase uses Kahn's algorithm [@kahn1962] for
topological task scheduling with bounded concurrency and per-task self-healing
loops that fall back to git bisect when standard healing fails.

# Statement of Need

AI-assisted coding tools fall into two paradigms: editor-embedded assistants that
sacrifice terminal flexibility, and command-line wrappers around single LLM calls
that lack structured workflow. Real engineering tasks such as migrating an
authentication middleware require understanding the codebase, discussing tradeoffs,
creating a plan, executing changes across files, verifying correctness, and
committing the result. Each phase demands different capabilities, autonomy levels,
and safety guarantees.

From a research perspective, M31A addresses the gap between practical software
engineering tools and formal computational methods. The system's codebase
intelligence subsystem (EFIE) demonstrates how graph-theoretic algorithms —
PageRank, Louvain community detection, and betweenness centrality — can be
applied to code exploration with provable complexity bounds. This makes M31A
both a useful tool and a platform for studying AI-assisted software engineering.

M31 Autonomous addresses this gap by treating the terminal as the primary
interface and owning the entire task lifecycle. Unlike existing tools such as
Aider [@aider] and Cline [@cline], which provide conversational code editing with
git integration, M31 Autonomous provides a structured workflow engine with explicit
phases, automatic complexity classification, runtime verification with smoke
testing, cross-session learning, and composable user-defined skills. The system
requires explicit user permission for every file operation and shell command,
providing safety without friction.

The composable skills system enables users to define custom slash commands via
Markdown files with YAML frontmatter, discovered automatically from project and
global configuration directories. This extensibility model allows domain-specific
workflows without modifying the agent's source code.

The provider abstraction layer supports multiple LLM backends (OpenRouter, Zen)
with a shared HTTP transport for connection pool reuse, singleflight-guarded model
catalog caching, and automatic fallback based on health checks. The model cache
uses a dual-TTL strategy (five minutes fresh, twenty-four hours stale) and supports
extended thinking parameters across four model families.

M31 Autonomous targets developers who work primarily in the terminal and need an
AI agent that manages the full software engineering loop rather than providing
isolated completions. The zero-telemetry design and OS keychain integration for
API key storage make it suitable for security-sensitive environments.

# Software Design

M31 Autonomous follows a strict layered architecture with a clear dependency rule:
higher layers may depend on lower layers, but never the reverse. Public packages
live in `pkg/`, while private implementation details are in `internal/`. The system
comprises six major subsystems:

1. **Terminal User Interface:** A 33-screen TUI built on Bubble Tea [@bubbletea]
   using the Elm architecture, providing keyboard-driven efficiency with screen
   transitions, command palette, and theming.

2. **Workflow Engine:** An eight-phase pipeline (Initialize, Discuss, Plan, Execute,
   Verify, Runtime, Ship) with complexity-adaptive mode selection that classifies
   goals into trivial, simple, moderate, or complex categories and selects an
   appropriate phase subset.

3. **Provider Abstraction:** A multi-provider LLM layer supporting OpenRouter and
   Zen with automatic fallback, health checks, and cost tracking via atomic CAS
   operations.

4. **Tool System:** 17 registered tools with permission-gated execution, rate
   limiting, and risk-level classification (safe, medium, dangerous, destructive).

5. **Code Intelligence (EFIE):** A novel community-structured, importance-weighted
   codebase exploration algorithm with formal mathematical foundations, including
   PageRank-based importance scoring, Louvain community detection, and betweenness
   centrality approximation.

6. **Cross-Session Learning:** A persistent ledger recording session outcomes,
   enabling the agent to learn from its own history across sessions.

The design prioritizes safety (explicit permission for every file operation), zero
telemetry, and static binary distribution (15-20 MB with zero runtime dependencies).

# State of the Field

Terminal-native AI coding agents have emerged as a distinct category following the
adoption of large language models for code generation [@chen2021codex]. Aider [@aider] provides git-integrated multi-file editing
with support for multiple LLM providers but does not include a structured workflow
engine, runtime verification, or cross-session learning. Cline [@cline] operates
as a VS Code extension with tool-use capabilities but is not terminal-native.
OpenHands [@openhands] and SWE-agent [@sweagent] target autonomous issue resolution
on benchmarks rather than interactive developer workflows.

M31 Autonomous distinguishes itself through its eight-phase workflow engine with
complexity-adaptive mode selection, runtime smoke testing, composable skills, and
persistent cross-session memory. To our knowledge, no existing terminal-native
agent combines structured workflow orchestration, automatic context consolidation,
runtime verification, and cross-session learning in a single static binary.

# Research Impact Statement

M31 Autonomous serves as both a software engineering tool and a platform for
computational research. The system's codebase intelligence subsystem (EFIE)
implements novel algorithms for community-structured, importance-weighted
codebase exploration, with formal mathematical proofs of correctness,
convergence guarantees, and complexity bounds published in the repository's
`EFIE/` directory.

Key research contributions include:

1. **Novel Algorithm Design:** The EFIE algorithm introduces adaptive expansion
   with community detection (Louvain modularity) [@blondel2008], PageRank-based
   importance scoring [@brin1998], and betweenness centrality approximation — a
   combination not previously applied to codebase exploration.

2. **Formal Mathematical Rigor:** The EFIE research paper contains 11 formal
   definitions, 3 numbered theorems with proofs, and complete complexity analysis
   (O(N × F / P + E × log V) build time, O(S × B) query time).

3. **Reproducible Research Workflows:** M31A's built-in research phase
   (`internal/workflow/research.go`) supports systematic codebase investigation
   before implementation planning, enabling reproducible software engineering
   research.

4. **Cross-Session Learning:** The cross-session learning ledger records goal,
   task outcomes, file patterns, model used, and duration for every session,
   creating a persistent record suitable for longitudinal research studies.

The software has been developed iteratively over six months with public releases,
tagged versions, and comprehensive documentation. The author is an independent
researcher with ORCID 0009-0007-1261-6805.

# AI Usage Disclosure

No generative AI tools were used in the development of this software. The EFIE
algorithm, its mathematical proofs, and the M31 Autonomous workflow engine were
designed and implemented by the author without AI assistance.

The paper manuscript was written by the author. No AI writing assistants were used
in drafting, editing, or revising this paper.

The author confirms that all design decisions, architectural choices, and
mathematical contributions are original human work, and that all code, proofs,
and documentation have been reviewed and validated by the author.

# References
