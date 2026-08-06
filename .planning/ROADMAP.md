# ROADMAP

> **Vision**
>
> Build the most reliable, predictable, and enjoyable autonomous terminal software engineer.
>
> Every phase exists to improve one or more of:
>
> - Reliability
> - User Experience
> - Engineering Quality
> - Performance
> - Intelligence
> - Ecosystem
>
> New features are only added after the existing experience reaches production quality.

---

# Phase 1 — Reliability First

## Objective

Build a foundation users can trust.

Success is measured by correctness and predictability—not feature count.

---

## Workflow Stability

- Eliminate workflow deadlocks
- Eliminate infinite execution loops
- Remove duplicate task execution
- Ensure deterministic workflow transitions
- Ensure phase recovery after interruption
- Prevent invalid state transitions
- Prevent inconsistent session state

---

## Concurrency

Audit every:

- goroutine
- mutex
- RWMutex
- WaitGroup
- channel
- timer
- ticker
- context

Eliminate:

- race conditions
- goroutine leaks
- channel leaks
- context leaks
- deadlocks
- lock-order issues

---

## Cancellation

Ensure cancellation propagates everywhere.

Stopping execution must:

- stop LLM streaming
- stop tool execution
- stop subagents
- stop runtime servers
- stop retries
- terminate child processes
- clean temporary resources
- persist recoverable state

---

## Error Handling

Standardize every error path.

Every error should answer:

- What failed?
- Why?
- Can recovery happen?
- What should the user do?

Remove:

- silent failures
- swallowed errors
- hidden retries
- inconsistent error messages

---

## Recovery

Implement reliable recovery for:

- crashes
- forced exits
- interrupted sessions
- failed workflows

Guarantee:

- resume correctly

or

- rollback safely

Never leave corrupted state.

---

## Testing

Focus on confidence instead of coverage.

Expand:

- integration tests
- workflow tests
- recovery tests
- cancellation tests
- race tests
- parallel execution tests
- state machine tests

Replace low-value coverage tests with meaningful behavioral tests.

---

## Refactoring

Reduce architectural complexity without changing behavior.

Examples:

- split oversized files
- reduce shared mutable state
- reduce mutex count
- improve ownership boundaries
- simplify workflow engine

---

## Exit Criteria

- Stable workflow execution
- Deterministic behavior
- Clean cancellation
- Reliable recovery
- No known critical race conditions
- High-confidence test suite

---

## Plans

**Plans:** 4 plans

Plans:

- [x] 01-01-PLAN.md — Concurrency refactoring (extract WorkflowState, document lock ordering, stress tests) — Complete 2026-08-05
- [x] 01-02-PLAN.md — Cancellation and error handling (context propagation, process kill, error wrapping) — Complete 2026-08-05
- [x] 01-03-PLAN.md — Testing confidence (delete coverage fluff, integration tests for gaps) — Complete 2026-08-05
- [ ] 01-04-PLAN.md — Recovery (crash-safe persistence, session resume, workflow rollback, recovery tests)

---

# Phase 2 — User Experience

## Objective

Make M31A feel effortless.

Users should always understand:

- what M31A is doing
- why it is doing it
- what happens next

---

## Interaction

Improve:

- onboarding
- first-run experience
- installation
- configuration
- authentication
- project detection

Reduce setup friction to the absolute minimum.

---

## Progress

Replace opaque execution with visible progress.

Expose:

- current task
- completed tasks
- active reasoning stage
- running tools
- verification progress

Avoid generic "Thinking..." indicators.

---

## Streaming

Improve:

- first-token latency
- streaming smoothness
- incremental updates
- perceived responsiveness

---

## Planning

Present plans that are:

- concise
- understandable
- editable
- trustworthy

---

## Feedback

Improve:

- warnings
- confirmations
- permission prompts
- recovery suggestions

Errors should always provide actionable guidance.

---

## Terminal Experience

Refine:

- layout
- keyboard navigation
- scrolling
- colors
- accessibility
- responsiveness

---

## Exit Criteria

Users consistently understand what M31A is doing without reading documentation.

---

## Plans

**Plans:** 4 plans

Plans:

- [x] 02-01-PLAN.md — Progress display: persistent status bar showing phase, task, elapsed time
- [x] 02-02-PLAN.md — Permission prompts: risk labels, batch approval, auto-approve safe tools
- [x] 02-03-PLAN.md — Plan presentation: collapsible sections, inline editing, time estimates
- [x] 02-04-PLAN.md — First-run tutorial: interactive feature tour, skippable, post-setup

---

# Phase 3 — Engineering Excellence

## Objective

Reduce maintenance cost while improving extensibility.

**Requirements:** ARCH-01, API-01, DEBT-01, DEBT-02, DOC-01, DX-01, DX-02, DX-03

---

## Architecture

Strengthen module boundaries.

Improve separation between:

- engine
- providers
- tools
- runtime
- workflow
- UI

---

## Internal APIs

Standardize:

- interfaces
- dependency injection
- lifecycle management
- ownership

Reduce implicit coupling.

---

## Technical Debt

Eliminate:

- oversized files
- duplicated logic
- inconsistent abstractions
- legacy compatibility code
- dead code

---

## Documentation

Document:

- architecture
- extension points
- workflow
- lifecycle
- conventions

---

## Developer Experience

Improve:

- local setup
- debugging
- profiling
- testing
- release process

---

## Exit Criteria

New contributors can understand and modify the codebase confidently.

---

## Plans

**Plans:** 3 plans

Plans:

- [ ] 03-01-PLAN.md — Engine.go split: decompose 1927-line file into 5 focused files by concern (pause, streaming, checkpoint, model, helpers)
- [ ] 03-02-PLAN.md — Architecture documentation: update ARCHITECTURE.md, create CONVENTIONS.md, update CONCERNS.md
- [ ] 03-03-PLAN.md — Developer experience: debug logging (slog), profiling (pprof), release automation (GoReleaser CI)

---

# Phase 4 — Performance

## Objective

Make M31A feel instantaneous whenever possible.

---

## Startup

Reduce:

- startup latency
- initialization cost
- provider loading
- configuration loading

---

## Workflow

Optimize:

- planning
- context building
- indexing
- prompt generation
- tool dispatch

---

## Memory

Reduce:

- allocations
- copies
- cache duplication
- unnecessary buffering

---

## Parallelism

Improve safe concurrency for:

- indexing
- analysis
- verification
- independent tasks

---

## Scaling

Ensure smooth behavior for:

- very large repositories
- long conversations
- long-running sessions
- multiple subagents

---

## Exit Criteria

Performance bottlenecks are measurable, documented, and continuously monitored.

---

# Phase 5 — Intelligence

## Objective

Improve decision quality—not just model capability.

---

## Planning

Generate:

- smaller plans
- smarter plans
- adaptive plans
- resumable plans

---

## Execution

Improve:

- task prioritization
- dependency analysis
- failure recovery
- verification quality

---

## Context

Improve:

- retrieval
- compression
- relevance scoring
- long-context handling

---

## Model Strategy

Enhance:

- provider selection
- model routing
- fallback behavior
- cost optimization

---

## Verification

Increase confidence before presenting results.

Reduce hallucinated success.

---

## Exit Criteria

Users trust M31A's decisions even on complex projects.

---

# Phase 6 — Ecosystem

## Objective

Make M31A a platform rather than a standalone application.

---

## Tooling

Expand extension capabilities for:

- tools
- providers
- workflows
- integrations

---

## Plugin System

Design stable APIs for external extensions.

---

## Configuration

Support:

- reusable profiles
- workspace defaults
- organization-wide policies

---

## Community

Improve:

- contribution workflow
- issue templates
- documentation
- examples
- sample projects

---

## Automation

Improve:

- CI
- releases
- benchmarking
- regression detection

---

## Exit Criteria

External contributors can extend M31A without modifying core components.

---

# Phase 7 — Production Readiness

## Objective

Reach a level suitable for daily professional use.

---

## Reliability

Continuously monitor:

- regressions
- crashes
- failures
- recovery

---

## Observability

Measure:

- startup time
- execution time
- completion rate
- verification rate
- cancellation rate
- retry rate
- resource usage

---

## Compatibility

Continuously validate:

- Linux
- macOS
- Windows

Support multiple project sizes and languages.

---

## Release Quality

Every release should satisfy:

- deterministic builds
- reproducible tests
- documented changes
- migration guidance

---

## Long-Term Support

Establish policies for:

- compatibility
- deprecation
- versioning
- maintenance

---

## Exit Criteria

M31A is dependable enough to become a developer's primary terminal coding assistant.

---

# Guiding Principles

Every contribution should improve at least one of:

- Reliability
- Predictability
- Simplicity
- Performance
- Developer Experience
- User Experience

Before introducing a new feature, ask:

- Does this improve an existing workflow?
- Can this be solved by simplifying something instead?
- Will this increase long-term maintenance cost?
- Is this aligned with M31A's core vision?

If the answer is uncertain, prefer improving the existing experience over expanding functionality.
