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
orchestrates an eight-phase software engineering workflow end-to-end. From project
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

# References
