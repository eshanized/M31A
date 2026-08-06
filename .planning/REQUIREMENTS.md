# REQUIREMENTS

> Requirements registry for all phases. Each requirement ID must appear in at least one plan's `requirements` frontmatter field.

---

## Phase 1 — Reliability First

| ID | Title | Description |
|----|-------|-------------|
| REL-01 | Workflow Stability | Eliminate deadlocks, infinite loops, duplicate execution, ensure deterministic transitions |
| REL-02 | Concurrency Safety | Audit all goroutines, mutexes, channels; eliminate races, leaks, deadlocks, lock-order issues |
| REL-03 | Cancellation Propagation | Stop LLM streaming, tool execution, subagents, runtime servers, retries, child processes on cancel |
| REL-04 | Error Handling Standardization | Every error answers: what failed, why, can recovery happen, what should user do |
| REL-05 | Crash Recovery | Reliable recovery for crashes, forced exits, interrupted sessions, failed workflows |
| REL-06 | Testing Confidence | Integration, workflow, recovery, cancellation, race, parallel, state machine tests |

---

## Phase 2 — User Experience

| ID | Title | Description |
|----|-------|-------------|
| UX-01 | Onboarding & Setup | Reduce installation, configuration, authentication, project detection friction |
| UX-02 | Progress Visibility | Expose current task, completed tasks, reasoning stage, running tools, verification progress |
| UX-03 | Streaming Quality | Improve first-token latency, streaming smoothness, incremental updates, perceived responsiveness |
| UX-04 | Plan Presentation | Concise, understandable, editable, trustworthy plans |
| UX-05 | Feedback & Guidance | Actionable warnings, confirmations, permission prompts, recovery suggestions |
| UX-06 | Terminal Experience | Layout, keyboard navigation, scrolling, colors, accessibility, responsiveness |

---

## Phase 3 — Engineering Excellence

| ID | Title | Description |
|----|-------|-------------|
| ARCH-01 | Module Boundaries | Strengthen separation between engine, providers, tools, runtime, workflow, UI |
| API-01 | Internal API Standardization | Standardize interfaces, dependency injection, lifecycle management, ownership |
| DEBT-01 | Eliminate Oversized Files | Split large files, reduce duplicated logic, inconsistent abstractions |
| DEBT-02 | Remove Legacy Code | Remove dead code, legacy compatibility code |
| DOC-01 | Architecture Documentation | Document architecture, extension points, workflow, lifecycle, conventions |
| DX-01 | Local Setup & Debugging | Improve slog logging, profiling (pprof), testing, release automation |
| DX-02 | Developer Onboarding | New contributors can understand and modify codebase confidently |
| DX-03 | Release Process | Automated cross-platform releases via GoReleaser |

---

## Phase 4 — Performance

| ID | Title | Description |
|----|-------|-------------|
| PERF-01 | Startup Latency | Reduce initialization cost, provider loading, configuration loading |
| PERF-02 | Workflow Optimization | Optimize planning, context building, indexing, prompt generation, tool dispatch |
| PERF-03 | Memory Efficiency | Reduce allocations, copies, cache duplication, unnecessary buffering |
| PERF-04 | Safe Parallelism | Improve concurrency for indexing, analysis, verification, independent tasks |
| PERF-05 | Scaling | Smooth behavior for large repos, long conversations, long sessions, multiple subagents |

---

## Phase 5 — Intelligence

| ID | Title | Description |
|----|-------|-------------|
| INT-01 | Smaller Plans | Generate more granular, focused plans |
| INT-02 | Smarter Plans | Adaptive planning with context awareness |
| INT-03 | Resumable Plans | Plans that can be paused and resumed |
| INT-04 | Task Prioritization | Improved dependency analysis and execution ordering |
| INT-05 | Failure Recovery | Automatic re-plan from failure |
| INT-06 | Verification Quality | Reduce hallucinated success, increase confidence |
| INT-07 | Context Retrieval | Improved relevance scoring, compression, long-context handling |
| INT-08 | Model Strategy | Provider selection, model routing, fallback, cost optimization |

---

## Phase 6 — Ecosystem

| ID | Title | Description |
|----|-------|-------------|
| EXT-01 | Extension Loading Mechanism | Config-driven external commands spawned as subprocesses, language-agnostic, no CGO/WASM |
| EXT-02 | Plugin API Surface | Public `pkg/extensions` package with stable Go interfaces for ExternalTool, ExternalProvider, PhaseHookHandler |
| EXT-03 | Configuration Profiles | Multi-file layered config: global TOML, workspace TOML, project JSON, env vars with precedence |
| EXT-04 | Workflow Extensibility | Pre/post phase hooks registered by extensions, invoked via MsgEmitter at phase transitions |
| EXT-05 | Extension Distribution | Decentralized: extensions as Go binaries/scripts via GitHub Releases, Homebrew, Scoop |
| EXT-06 | Contribution Workflow | Issue templates, PR templates, CODEOWNERS, labeler, automated checks, required reviewers, branch protection |
| EXT-07 | Documentation & Samples | API reference, authoring guide, tutorial, testing guide, migration guide, FAQ + 3 sample extensions |
| EXT-08 | CI/Release/Benchmark Automation | Benchstat regression detection, nightly workflows, Dependabot, GoReleaser enhancements, gh-pages dashboard |

---

## Phase 7 — Production Readiness

| ID | Title | Description |
|----|-------|-------------|
| PROD-01 | Regression Monitoring | Continuous monitoring of regressions, crashes, failures, recovery |
| PROD-02 | Observability | Measure startup time, execution time, completion rate, verification rate, cancellation rate, retry rate, resource usage |
| PROD-03 | Cross-Platform Compatibility | Validate Linux, macOS, Windows; support multiple project sizes and languages |
| PROD-04 | Release Quality | Deterministic builds, reproducible tests, documented changes, migration guidance |
| PROD-05 | Long-Term Support | Compatibility, deprecation, versioning, maintenance policies |

---

*Last updated: 2026-08-06*