# M31A — Requirements

All V1 acceptance criteria from `adrenaline/idea.md` sections 15 and 17,
plus the 6 differentiator criteria.

## Acceptance Criteria (V1)

- [ ] 1. **Binary builds**: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` produces static binary
- [ ] 2. **Dual-provider discovery**: On first run with valid key(s), both OpenRouter and Zen model lists populate within 3 seconds
- [ ] 3. **Provider selector**: `/model` shows searchable list with provider badges [OR]/[ZEN], pricing, context, and capabilities
- [ ] 4. **Provider switching**: Switching from OpenRouter to Zen is instant (warm cache); active model re-resolves on target provider
- [ ] 5. **Inline thinking**: Using `deepseek/deepseek-r1` on either provider, reasoning tokens appear in collapsible block above final answer
- [ ] 6. **Streaming UX**: Token-by-token streaming with no flicker; thinking tokens render in real-time
- [ ] 7. **Tool cards**: Bash/FileRead results render as collapsible cards with syntax highlighting
- [ ] 8. **Workflow end-to-end**: Type a goal → Initialize → Discuss → Plan → Accept → Execute → Verify → Ship completes with markdown rendering at each phase
- [ ] 9. **Plan generation**: Plan phase produces a dependency-annotated task list with estimated cost/time; user can Accept, Edit, or Retry
- [ ] 10. **Execution**: Tasks execute in dependency order; completed tasks produce atomic git commits; progress persists to `TASKS.md` and `STATE.md`
- [ ] 11. **Verification**: After execution, Verify phase runs acceptance checks; failures offer Self-Heal (max 2 attempts) or Skip
- [ ] 12. **File-based state**: `PROJECT.md`, `TASKS.md`, and `STATE.md` are human-readable Markdown in `~/.m31a/sessions/<id>/planning/`; session resume parses them correctly
- [ ] 13. **Permission modal**: Dangerous tool triggers centered modal with timeout and syntax preview
- [ ] 14. **Context bar**: Header shows accurate `used/total` context that updates live
- [ ] 15. **Session resume**: Close and reopen; previous session restores with metadata, workflow phase, task progress, and context-aware truncated history
- [ ] 16. **Theme switch**: `/theme` cycles with instant application, no restart required
- [ ] 17. **Zero manual config**: Only required input is at least one API key (OpenRouter or Zen); everything else is discovered
- [ ] 18. **Auto-fallback**: When `auto_fallback=true` and active provider returns 429/503, M31A switches to the other provider with a visible banner
- [ ] 19. **Workflow slash commands**: `/new`, `/plan`, `/execute`, `/verify`, `/ship`, `/workflow`, `/pause`, `/resume-task` all function correctly
- [ ] 20. **Context engineering**: Each phase launches with a pruned context; AutoDream consolidates at 60% threshold

## Acceptance Criteria — Differentiators (V1)

- [ ] 21. **MIT License**: Source code is available under MIT license; no telemetry, no vendor lock-in, no paid tiers
- [ ] 22. **Model arbitrage**: `/optimize` analyzes current plan and suggests cheaper model alternatives with cost savings percentage; `O` key accepts all suggestions
- [ ] 23. **Git bisect**: After self-heal fails twice on a task, Verify phase runs `git bisect` and displays the offending commit with diff; retry heal uses bisect context
- [ ] 24. **Diff preview**: Plan screen `D` key opens diff preview overlay showing predicted file changes (new/modify/delete) with estimated line counts across all tasks
- [ ] 25. **Learning ledger**: `~/.m31a/LEDGER.md` exists and is updated after each Ship phase; `/ledger` shows filtered viewer; `/ledger stats` shows aggregate statistics; relevant past sessions are injected during Initialize phase
- [ ] 26. **Commit rollback**: `/rollback` shows interactive commit timeline for current session; soft and hard rollback modes both function; backup branch is created before rollback; task statuses update correctly
