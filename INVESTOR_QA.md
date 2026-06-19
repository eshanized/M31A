# M31 Autonomous — Investor Q&A

> Answers grounded in the M31A codebase. No fabrication.

---

## 1. Why are you the right founder/team to work on this?

**Eshan Roy is a solo founder with 16 months of sustained, high-cadence execution.**

- **2,333 commits** since February 2025 — 98.7% authored by Eshan alone
- Consistent output of **~110–140 commits/month** for 16 straight months, no month below 85
- Built the entire system solo: workflow engine, 29-screen TUI, provider abstraction, 14 tools, 4-language code intelligence, security model, cross-platform release infrastructure
- The codebase is **1.6M+ lines of Go** across 3,572 source files with 2,271 test files
- Shipped **3 production releases** (v1.0.0 → v1.0.2 → v1.1.0) with full CI/CD, cross-compilation (5 targets), and package manager distribution (Homebrew, Scoop, deb/rpm/apk)

This isn't a prototype. It's a production-grade system built by someone who can ship end-to-end — from low-level SSE parsing to pixel-perfect TUI, from git bisect automation to D-Bus keychain integration across three OSes.

---

## 2. Why did you pick this idea to work on?

**The problem: AI coding tools do 20% of the work and leave you with 80%.**

From the project's own documentation (`DEV.md:17–28`):

> "Most AI coding assistants are glorified autocomplete on steroids. They suggest code, maybe write a function or two, but leave you holding the bag when it comes to testing, verification, and actually shipping the changes."

The typical workflow today:
1. Ask AI to write code
2. Copy-paste into editor
3. Run tests manually
4. Debug the issues
5. Repeat
6. Commit yourself

**The insight** (`RESEARCH.md:24–28`): Existing tools fall into two constrained paradigms — editor-bound (losing terminal flexibility) or thin CLI wrappers around a single LLM call (lacking structured workflow). Nobody owned the **full loop**.

M31 Autonomous is the first tool where the AI agent orchestrates the entire lifecycle: Initialize → Discuss → Plan → Execute → Verify → Ship. Every run ends with a verified git commit and a ledger entry. The human reviews and approves — the agent does the work.

---

## 3. Who are your competitors, and what do you understand that they don't?

**Competitors**: Cursor (editor-bound), Aider (terminal, no workflow), Cline (editor extension)

**What they do**: Generate code. They are sophisticated autocomplete.

**What M31 understands**: **Workflow ownership matters more than code generation.**

| Capability | M31 | Cursor | Aider | Cline |
|---|:---:|:---:|:---:|:---:|
| Six-phase workflow engine | **yes** | no | no | no |
| Git commit rollback chain | **yes** | no | partial | no |
| Cross-session learning ledger | **yes** | no | no | no |
| AutoDream context consolidation | **yes** | no | no | no |
| Model arbitrage (cost optimizer) | **yes** | no | no | no |
| Provider auto-fallback | **yes** | no | partial | partial |
| Code intelligence (4 languages) | **yes** | yes | limited | no |
| Static binary, no CGO | **yes** | no | no | no |
| Telemetry | **none** | yes | none | yes |
| Cost per task | **~$0.01** | $20/mo | ~$0.01 | ~$0.01 |

The moat isn't any single feature — it's the **systems-level integration**. The task runner uses Kahn's topological sort for dependency resolution. The rollback chain creates backup branches before every destructive reset. AutoDream compresses context when the window fills up, protecting system prompts and recent messages. The security model has 17 formally audited findings with mitigations. The provider layer does parallel health checks with stale-while-revalidate caching.

No competitor has this because no competitor is building a **workflow engine** — they're all building **code generators**.

---

## 4. What's your revenue and/or growth rate?

**Honest answer: Pre-revenue, pre-distribution. MIT-licensed, free.**

What does exist:
- **3 tagged releases** (v1.0.0, v1.0.2, v1.1.0) with GoReleaser automation
- **Full distribution pipeline**: Homebrew tap, Scoop (Windows), deb/rpm/apk/archlinux packages, one-liner `curl | bash` installer
- **GitHub Sponsors** configured (`.github/FUNDING.yml`)
- **Promotion plan** written: 11 pre-built launch posts for HN, Reddit, Twitter, Dev.to, Product Hunt, Lobsters
- **Target metrics** defined: 50–100 stars Day 1, 200–500 Week 1, 500–1000 Month 1

The product is ready for launch. The infrastructure to measure and monetize is in place. Revenue will follow distribution.

---

## 5. Anything else you would like investors to know?

**Three things:**

**a) The technical depth is real.** This isn't a wrapper around the OpenAI API. The provider layer handles SSE streaming with watchdog timeouts, exponential backoff with Retry-After header support, and capability detection heuristics. The code intelligence module parses Go AST, TypeScript, Python, and Rust to build import dependency graphs. The security model includes SSRF protection with DNS pinning, TOCTOU prevention, path traversal guards, and rate limiting via token buckets. A formal security audit documented 17 findings with mitigations.

**b) The architecture is built to compound.** The cross-session learning ledger (`pkg/ledger/`) stores patterns, failures, and recoveries in markdown tables — the agent gets smarter over time. The AutoDream context consolidation means conversations can run indefinitely without hitting context limits. The subagent system with git worktree isolation means the tool scales to complex, parallelizable tasks. These aren't features — they're **flywheels**.

**c) Zero telemetry is a feature, not a limitation.** In a market where every AI tool phones home, M31 Autonomous does exactly what the code says and nothing more. `CGO_ENABLED=0` static binary, OS-native keychain for API keys (never plaintext on disk), no analytics, no crash reporting. This is the tool enterprises will trust because it's auditable.

---

*All claims sourced from the M31A repository: README.md, DEV.md, RESEARCH.md, SECURITY.md, CONTEXT.md, and the Go source code.*
