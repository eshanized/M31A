# Phase 13 Visual Review & Dual-Gate Verification Checklist

Canonical documentation and review criteria for M31A Terminal User Interface (TUI) visual fidelity, responsive layouts, color contrast, and dual-gate verification.

## 1. Canonical Themes & Contrast Invariants

The TUI must render with strict readability and high contrast across all four canonical themes:

- **Dark Slate Cyan**: The default theme featuring deep slate backgrounds, crisp foreground text, and high-contrast cyan accents (`accent_primary`).
- **High Contrast**: Designed for accessibility and strict legibility, ensuring maximal separation between foreground and canvas backgrounds.
- **Clean Light**: High-readability light canvas theme with dark text and preserved semantic accent tokens.
- **Monochrome ANSI**: Fallback theme for basic terminals restricted to 16-color ANSI output without 24-bit truecolor support.

## 2. 12-Status Taxonomy

Every task, mission, capability, and policy gate displays one of the 12 authoritative status markers:

- `[OK]` — Succeeded and verified with evidence.
- `[RUN]` — Actively executing under supervision.
- `[WAIT]` — Blocked waiting on upstream dependencies.
- `[BLOCK]` — Blocked by safety policy or missing capability.
- `[FAIL]` — Terminated with failure or unexpected error.
- `[WARN]` — Degraded execution or non-fatal warning condition.
- `[ASK]` — Awaiting human review or operator confirmation.
- `[DENY]` — Policy denied execution.
- `[PLAN]` — In planning or differential replanning stage.
- `[VERIFY]` — Running governed multi-tier verification checks.
- `[PAUSE]` — Explicitly paused by operator.
- `[CANCEL]` — Cancelled via user request or deadline expiration.

## 3. Responsive Layout Tiers

The visual interface dynamically adapts to terminal dimensions according to four standard viewport tiers:

- **Compact Tier (80x24)**: Minimal layout with collapsible sidebars and single-column activity streams.
- **Standard Tier (120x35)**: Default developer layout with split-pane navigation, task trees, and inspection panes.
- **Large Tier (180x45)**: Multi-column view showing concurrent execution lanes, live streaming logs, and diff inspectors.
- **UltraWide Tier (>=220x60)**: Comprehensive multi-panel layout including real-time graph visualization, telemetry gauges, and approval queues.

## 4. Critical User Flows

The following visual flows must pass visual review and automated layout assertions:

- **First-Run Guided Setup Wizard**: Workspace initialization, capability auto-detection, and provider key configuration.
- **Dedicated Mission Creator**: Objective input, template selection, change surface restriction, and initial plan preview.
- **Startup Reconciliation Recovery**: Crash detection banner, uncommitted worktree restoration, and state reconciliation dialog.
- **Universal Command Palette V2**: Keyboard-driven quick-action search with fuzzy matching and role filtering.
- **Settings & Provenance Screen**: View configuration, security audit logs, and cryptographic change provenance inspection.
- **Non-Dismissible Critical Approval Modal**: High-priority modal prompt for dangerous actions requiring explicit typed operator authorization.
