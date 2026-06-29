# M31A Strategic Roadmap: v1.6 → v2.0

## Guiding Philosophy

M31A's identity is clear: a **terminal-native autonomous software engineering partner**. It does not live in an editor. It does not require a browser. It runs as a single binary, talks to LLMs, executes tools, and ships verified code. Every release must sharpen this identity, not dilute it.

The five releases below trace a single arc: from *workflow executor* to *engineering peer*.

---

## v1.6 — "Autonomy"

**Theme:** M31A can run headless, manage its own approval flow, and sustain long sessions without human intervention.

### Goals

1. Ghost mode: headless execution that produces structured diffs without the TUI
2. Deferred tools: queue dangerous tool calls for batch human review instead of blocking
3. Context budgeting: proactive token management that prevents overflow mid-plan
4. Session continuity: cross-session knowledge persistence and resumption

### Major Initiatives

| Initiative | Why |
|---|---|
| **Ghost mode** (`--prompt` already exists but is single-shot). Extend to full workflow execution headless: `m31a run --goal "..." --output diff` produces a git diff, decision log, and verification report without launching the TUI. Enables CI/CD integration, batch operations, and automation. | The README already lists this as a v1.1 target. It is the highest-leverage feature for making M31A useful beyond interactive sessions. |
| **Deferred tools** | Dangerous tool calls (file writes, bash commands matching blocklists) are queued with context snapshots. User approves/rejects in batch from a `/pending` screen or via `m31a review`. Non-blocking execution continues for safe tools while pending. | Eliminates the #1 friction point in autonomous mode: mid-plan halts waiting for a single approval. |
| **Adaptive context budgeting** | Per-task token estimation with runtime calibration. Budget allocation across the full plan. Automatic redistribution when tasks consume more/less than expected. Integration with AutoDream compaction. | Shipped as a spec in v1.5 but not fully realized. Essential for 20+ task plans that currently overflow context. |
| **Cross-session knowledge carryover** | Extend the project knowledge store (v1.5) to persist across sessions. Convention maps, failure patterns, and architecture understanding survive session boundaries. `/knowledge` screen to browse stored intelligence. | Makes M31A smarter every time it runs on a project. Compounds value over weeks. |

### Success Metrics

| Metric | Target |
|---|---|
| Ghost mode: end-to-end plan execution headless | 80% of ≤10 task plans complete without TUI |
| Deferred tools: average interruption frequency | Drop from ~3/plan to <1/plan for typical workflows |
| Context overflow failures in long sessions | <3% (down from ~20% baseline) |
| Cross-session knowledge retrieval accuracy | >70% of relevant conventions surfaced automatically |

---

## v1.7 — "Trust"

**Theme:** Every action M31A takes is explainable, reversible, and verifiable. Users trust it with critical codebases.

### Goals

1. Behavioral verification proves correctness through execution, not text matching
2. Selective rollback at hunk level for surgical undo
3. Decision audit trail with reasoning chains
4. Predictive risk assessment before destructive operations

### Major Initiatives

| Initiative | Why |
|---|---|
| **Behavioral verification engine** | Beyond text-substring acceptance criteria: execute commands (with security allowlists), hit HTTP endpoints (localhost only), run negative assertions ("this must NOT exist"), regex assertions. Verify phase becomes proof, not just checking. | The v1.5.0 spec identified this as critical. Text matching misses runtime bugs. Trust requires behavioral proof. |
| **Hunk-level rollback** | Extend file-level rollback (v1.5) to git hunk granularity. `RevertHunk(file, lineRange)` for surgical undo of specific changes within a file. Integrates with the decision log to show what was undone and why. | File-level rollback is too coarse when a plan modifies 30 lines across 5 functions in one file. |
| **Decision reasoning chains** | Extend decision transparency (v1.5) with causal chains: "I chose approach X because Y failed, Z was a constraint, and W is the precedent." Store as structured data, not just log text. `/decisions --explain` shows full reasoning trees. | Explainability without reasoning is just logging. Trust comes from understanding *why*, not just *what*. |
| **Pre-flight risk scoring** | Before executing any tool call, score the action's risk: blast radius (files affected), reversibility (can undo?), confidence (how sure is the LLM?). High-risk actions require explicit confirmation even in autonomous mode. | Makes the permission system intelligent rather than rule-based. Reduces surprise. |

### Success Metrics

| Metric | Target |
|---|---|
| Behavioral verification coverage | >60% of acceptance criteria use behavioral checks |
| Hunk-level rollback accuracy | 100% of rollback attempts restore exact prior state |
| Decision reasoning chain completeness | >90% of major decisions have ≥3 reasoning steps |
| User-reported "surprise" incidents | <1 per 10 plans executed |

---

## v1.8 — "Intelligence"

**Theme:** M31A learns from every session, predicts problems before they occur, and adapts its strategy to each project.

### Goals

1. Cross-session learning with failure pattern recognition
2. Predictive model selection based on task complexity
3. Project-specific strategy adaptation
4. Proactive issue detection

### Major Initiatives

| Initiative | Why |
|---|---|
| **Failure pattern library** | Every self-heal attempt (v1.5 `HealReport`) feeds into a persistent pattern library. When M31A encounters a similar error, it skips the generic retry and applies the known fix. Patterns are project-scoped and globally shared. | Self-healing currently retries blindly. Learning from past failures turns retry loops into instant fixes. |
| **Predictive model routing** | Extend `arbitrage/` (currently cost-based) with quality-based routing: analyze task type, codebase complexity, and historical success rates to select the optimal model for each phase, not just the cheapest. | Not all tasks need GPT-4. Plan generation benefits from different models than code editing. Intelligent routing improves quality and reduces cost. |
| **Project strategy profiles** | M31A learns a project's preferred patterns: testing frameworks, naming conventions, commit style, architecture patterns. New sessions start with these preferences pre-loaded, reducing discuss/plan overhead for recurring projects. | Eliminates the "explain your project every session" friction. Makes M31A feel like a team member who knows the codebase. |
| **Proactive issue detection** | During Initialize phase, scan for: dependency vulnerabilities, outdated patterns, TODO debt, test coverage gaps, architectural drift. Present as a health report before planning begins. | Shifts from reactive (fix what breaks) to proactive (prevent what will break). Builds trust through foresight. |

### Success Metrics

| Metric | Target |
|---|---|
| Failure pattern hit rate | >30% of self-heal attempts use known patterns |
| Model routing improvement | >15% reduction in failed tasks vs. single-model approach |
| Project profile accuracy | >80% of convention preferences auto-detected correctly |
| Proactive issue detection precision | >70% of flagged issues are genuine problems |

---

## v1.9 — "Extensibility"

**Theme:** M31A becomes a platform. Users extend it with custom tools, workflows, and providers without forking.

### Goals

1. Plugin architecture for tools, providers, and workflow phases
2. Custom workflow definitions
3. Community tool ecosystem
4. Local model support

### Major Initiatives

| Initiative | Why |
|---|---|
| **Plugin SDK** | Define tools, providers, and workflow phases as Go plugins (or WASM modules for safety). `m31a plugin install <name>` adds capabilities. Plugin manifest declares permissions, risk levels, and dependencies. | M31A's 18 tools cover common cases. Domain-specific tools (database migrations, deployment scripts, custom linters) need an extension mechanism that doesn't require forking. |
| **Custom workflow phases** | Allow users to define new phases (or modify existing ones) via YAML/JSON configuration. A "lint" phase, a "deploy" phase, a "security audit" phase. Phase composition with dependency graphs. | The 7-phase workflow is opinionated. Power users need to inject their own gates and steps. |
| **Local model support** | Ollama/llama.cpp integration as a provider. Run small models (CodeLlama, DeepSeek) locally for fast operations (intent classification, code completion) while routing complex tasks to cloud models. | Reduces latency for simple operations, enables offline use, and cuts costs for high-frequency low-complexity tasks. |
| **Tool marketplace** | Community-contributed tools shared via a registry. `m31a tools search "database"` finds relevant tools. Quality scoring based on community usage and ratings. | Turns M31A from a tool into a platform. Network effects compound value. |

### Success Metrics

| Metric | Target |
|---|---|
| Plugin adoption | >5 community plugins within 3 months of release |
| Custom workflow usage | >20% of active users define custom phases |
| Local model latency | <2s for intent classification (vs. 3-5s cloud) |
| Tool marketplace size | >20 published tools |

---

## v2.0 — "Partnership"

**Theme:** M31A is a peer-level engineering partner. It understands project context deeply, collaborates naturally, and operates with the judgment of a senior engineer.

### What Justifies a Major Version Bump

v2.0 is not a feature release. It is an **identity shift**. M31A stops being a tool you use and becomes a partner you work with. The key differentiators:

1. **Multi-project awareness**: M31A understands relationships between repositories, shared libraries, and deployment pipelines. It can work across a monorepo or coordinate changes across related projects.

2. **Natural collaboration**: Conversational interaction replaces command-response. M31A remembers context from previous conversations, asks proactive questions, and proposes alternatives before being asked.

3. **Judgment**: M31A develops project-specific judgment through accumulated experience. It knows which patterns have worked, which changes are risky, and when to ask for help vs. when to act autonomously.

4. **Reliability guarantees**: M31A can make commitments ("I will have this ready in 10 minutes") and track them. It reports progress without being asked and escalates issues immediately.

5. **Clean architecture**: By v2.0, the engine decomposition (Engine → WorkflowState + PhaseHandlers + ContextManager) is complete. The codebase is maintainable, testable, and extensible.

### Major Initiatives

| Initiative | Why |
|---|---|
| **Engine decomposition** | Break the 47+ field Engine struct into composable subsystems. WorkflowState, PhaseHandlers, ContextManager, ToolRegistry as separate, testable units. This is the prerequisite for everything else. | Technical debt that blocks all advanced features. Must be done before v2.0. |
| **Conversation memory** | M31A maintains a conversation graph (not just a message list). It understands topics, decisions, and context shifts across a session. Can answer "why did we decide X?" hours later. | Current context is a flat message list. Real collaboration requires understanding conversation structure. |
| **Multi-project graph** | Represent project dependencies as a graph. When M31A changes a shared library, it knows which downstream projects are affected and can propose coordinated changes. | Most real engineering work spans multiple repositories. M31A currently treats each project in isolation. |
| **Autonomous milestone management** | M31A can break a high-level goal into milestones, track progress across sessions, adjust plans based on what it learns, and report status to stakeholders. | Shifts from "execute this plan" to "achieve this goal." The highest level of autonomy. |

### Success Metrics

| Metric | Target |
|---|---|
| Cross-project change coordination | Successfully coordinate changes across 3+ related repos |
| Conversation context retention | >90% accuracy when answering questions about earlier discussion |
| Autonomous milestone completion | Complete 50% of simple milestones (≤3 tasks) without human intervention |
| Architecture maintainability | Engine decomposed into <10 field structs, >80% test coverage on core paths |

---

## Technical Debt to Address Before v2.0

| Debt | Severity | Target Release | Why |
|---|---|---|---|
| **Engine struct decomposition** (47+ fields, 1345 lines) | Critical | v1.6–v1.8 | Blocks all extensibility and testability |
| **TUI Update dispatch** (3544 lines, single function) | High | v1.6 | Makes TUI changes risky and slow |
| **Config merge reflection** (fragile, slow, hard to debug) | High | v1.7 | Blocks custom workflow config |
| **Edit tool 7-strategy cascade** (859 lines) | Medium | v1.7 | Maintenance burden, false positive risk |
| **tiktoken-go unmaintained** | Medium | v1.8 | Silent fallback drift for non-OpenAI models |
| **Provider registry race conditions** | High | v1.6 | Correctness issue under concurrent fallback |
| **Channel-based workflow→TUI communication** (128 msg buffer) | Medium | v1.8 | Silent message drops under load |
| **Session file locking** (per-project) | Low | v1.7 | Correctness in concurrent subagent scenarios |
| **Documentation drift** (ARCHITECTURE.md, WORKFLOW.md outdated) | Low | v1.6 | New contributors confused by stale docs |

---

## Strategic Risks

| Risk | Impact | Mitigation |
|---|---|---|
| **Complexity accumulation** | Each release adds features. By v2.0, the system could become unmanageable. | Strict theme-per-release discipline. Every feature must reinforce the theme. Reject features that don't fit. |
| **Autonomous action failures** | A single bad autonomous action (wrong file edit, bad commit) destroys user trust irreversibly. | Conservative defaults. Always show what will happen before doing it. Reversibility as a design requirement, not an afterthought. |
| **Provider ecosystem changes** | OpenRouter pricing changes, API deprecations, new competitors. | Provider abstraction layer with fallback. No dependency on any single provider's pricing or API stability. |
| **Single-binary constraint limits innovation** | WASM plugins, local models, and language parsers all push against the static binary model. | Accept the constraint. It is a feature (zero-dependency deployment), not a limitation. Innovate within it. |
| **Feature drift from terminal-native identity** | Temptation to add editor plugins, browser UIs, or web interfaces. | Reject. M31A's terminal-native identity is its differentiator. Editor integration is a separate product, not a feature. |
| **Performance degradation with scale** | More features → more initialization, more memory, more startup time. | Performance budget: startup <2s, tool execution <500ms p95, memory <100MB for typical projects. Enforce in CI. |
| **Community fragmentation** | Plugin ecosystem could split into incompatible forks. | Strong plugin SDK with versioning, compatibility guarantees, and a central registry. |

---

## Product Vision: What M31A Should Be Known For

### By v2.0, M31A is known as:

> **The autonomous software engineering partner that earns trust through transparency.**

Not the fastest. Not the cheapest. Not the one with the most features. The one you trust with your codebase because it explains its reasoning, proves its work is correct, learns from its mistakes, and never surprises you.

### The Three Pillars

1. **Transparency**: Every decision has a reasoning chain. Every action has a reversal path. Every result has a verification proof. You never wonder what M31A did or why.

2. **Accumulated Intelligence**: M31A gets smarter on your project every time it runs. It remembers what worked, what failed, and what your team prefers. It does not start from scratch.

3. **Bounded Autonomy**: M31A operates independently within clearly defined trust boundaries. It does what it can, asks when it should, and never oversteps. You set the boundaries; it respects them.

### What M31A is NOT

- Not an IDE. It does not replace your editor.
- Not a chatbot. It does not just answer questions.
- Not a code generator. It does not just produce code.
- Not a CI/CD tool. It does not just run pipelines.

M31A is a **partner**. It thinks, plans, executes, verifies, and ships — and it tells you everything along the way.

### The Metric That Matters

By v2.0, the metric that defines M31A's success is not features shipped or lines of code. It is:

> **"I trust M31A to work on my codebase while I'm away."**

That is the north star. Every feature, every release, every design decision should move toward that goal.
