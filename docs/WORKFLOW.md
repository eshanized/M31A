# Six-Phase Workflow

M31 Autonomous routes every prompt through six phases, giving you cost-optimized, high-quality responses with full traceability.

---

## Phase 1: Session

**Package:** `pkg/session`

Manages conversation lifecycle. Sessions store full message history with checkpoint/restore.

- Create named sessions with `/session create <name>`
- List all sessions with `/session list`
- Save checkpoints with `/session checkpoint`
- Restore from any checkpoint
- Sessions stored as JSON in `~/.m31a/sessions/`

---

## Phase 2: Auto (DREAM)

**Package:** `pkg/autodream`

Prompt enhancement that rewrites raw user input into structured, high-quality prompts.

**Modes:**
- **DEEP** — Full rewrite: clarifies intent, adds constraints, structures output format
- **FAST** — Light touch: fixes grammar, trims noise
- **SKIP** — Pass through without modification

Toggle with `/dream on` / `/dream off`.

---

## Phase 3: Dream (Ghost Write)

**Package:** `internal/ghost`

Generates files from natural language prompts. The LLM writes append-only content to disk.

- Activate with `/ghost` command
- Select target files from the picker
- Content is appended (never overwrites existing data)
- Configurable output directory and file patterns in `[ghost]` config

---

## Phase 4: Bisect

**Package:** `pkg/bisect`

Compares responses from multiple models side-by-side, computing diff scores.

- Usage: `/bisect <model1> <model2> <prompt>`
- Shows token-level diff with similarity scoring
- Helps identify which model handles your task better
- Output displayed in dedicated bisect view

---

## Phase 5: Arbitrage

**Package:** `pkg/arbitrage`

Cost-optimization engine. Scores task complexity, estimates token usage across models, and recommends the cheapest model that meets quality requirements.

**Task Classification:**
| Level | Estimated Tokens | Example |
|-------|-----------------|---------|
| Simple | < 2000 | Yes/no questions, simple lookups |
| Moderate | 2000–8000 | Code review, summarization |
| Complex | > 8000 | Architecture design, multi-step reasoning |

Enable with `auto_arbitrage = true` in config.

---

## Phase 6: Rollback

**Package:** `pkg/rollback`

Session state snapshot and restore. Protects against data loss during long sessions.

- Automatic checkpoints at configurable intervals
- Manual checkpoints with `/session checkpoint`
- Restore to any checkpointed state
- Handles graceful degradation on partial data

---

## Workflow Diagram

```
User Input
    ↓
[Session] — Creates/loads session context
    ↓
[Auto/DREAM] — Enhances prompt (optional)
    ↓
[Dream/Ghost] — Generates files (if ghost mode)
    ↓
[Bisect] — Compares models (if bisect mode)
    ↓
[Arbitrage] — Selects optimal model
    ↓
[Provider] — Sends to LLM, streams response
    ↓
[Rollback] — Creates checkpoint
    ↓
Output displayed in TUI
```

All phases are composable — you can use any subset depending on the task. Not all phases activate for every prompt; many are opt-in via commands or config.
