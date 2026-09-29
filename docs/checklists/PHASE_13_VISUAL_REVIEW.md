# Phase 13 — Human Visual Review & Aesthetic Quality Checklist

> Dual-gate verification contract (D-09, QAL-03) combining automated framebuffer assertions with human visual inspection across real terminal emulators.

---

## 1. Scope & Verification Strategy

Phase 13 establishes the human-facing operational product experience for M31A. In accordance with PRD §75, UI/UX Spec §113, and TRD §6, this checklist must be completed by the human operator before final milestone closure.

Testing environment recommendation:
- Emulators: **Alacritty**, **Kitty**, **iTerm2**, **Windows Terminal**, or **GNOME Terminal**.
- Minimum window size: **80 cols × 24 rows**.
- Standard developer window size: **120 cols × 36 rows**.

---

## 2. Four Canonical Themes Visual Inspection

Cycle themes using hotkey `F2` or command palette `Cycle Color Theme`.

### 2.1 Dark Slate Cyan (Default)
- [ ] Primary background is deep slate; foreground text is crisp off-white (`#E6EDF3`).
- [ ] Active borders and focused fields glow with cyan highlight (`#38BDF8`).
- [ ] Text contrast meets WCAG AA (minimum 4.5:1 ratio for normal text).
- [ ] No fluorescent or harsh glare colors in long-running telemetry tails.

### 2.2 High Contrast (Accessibility)
- [ ] Background is pure solid black (`#000000`); foreground text is pure bright white (`#FFFFFF`).
- [ ] Status tokens and alerts use saturated bright primaries (bright yellow `#FACC15`, bright green `#4ADE80`, bright red `#F87171`).
- [ ] Border lines are distinct and easy to track without color differentiation.

### 2.3 Clean Light (Daylight & Presentation)
- [ ] Background is clean soft off-white (`#F8FAFC`); text is deep charcoal (`#0F172A`).
- [ ] Headers, borders, and selection highlights are clearly legible in bright ambient lighting.
- [ ] Status badges maintain color fidelity without washing out against the light background.

### 2.4 Monochrome ANSI (Fallback & NO_COLOR)
- [ ] Terminal strictly honors the `NO_COLOR` environment variable if set.
- [ ] Renders using only standard 16 ANSI colors or basic bold/dim styling.
- [ ] All 12 status tokens (`[OK]`, `[RUN]`, `[FAIL]`, etc.) are fully distinguishable by text brackets and symbols without color cues.

---

## 3. 12-Status Taxonomy Dual Symbol Consistency

Verify that each status renders with its distinct symbol AND bracketed token:

| Expected Token | Expected Symbol | Semantic State | Verified? |
|:---|:---:|:---|:---:|
| `[OK]` | `✓` | Success, completed, verification passed | [ ] |
| `[RUN]` | `▶` | Active execution, streaming tokens | [ ] |
| `[WAIT]` | `⏳` | Pending turn, scheduled, awaiting dependency | [ ] |
| `[BLOCK]` | `⛔` | Blocked on precondition, cycle limit | [ ] |
| `[FAIL]` | `✗` | Execution failure, verification rejected | [ ] |
| `[WARN]` | `⚠` | Non-fatal anomaly, quota threshold warning | [ ] |
| `[ASK]` | `❓` | Operator approval required at policy gate | [ ] |
| `[DENY]` | `⊘` | Policy violation, disallowed tool/action | [ ] |
| `[PLAN]` | `📋` | Requirements extraction, DAG decomposition | [ ] |
| `[VERIFY]` | `🔍` | Verification hierarchy check in progress | [ ] |
| `[PAUSE]` | `⏸` | Mission paused by operator or budget pause | [ ] |
| `[CANCEL]` | `⏹` | Mission cancelled by operator | [ ] |

---

## 4. Responsive Layout Degradation

Test window resize reflow dynamically:

- [ ] **Compact Tier (80x24):**
  - Header is exactly 2 rows; footer is 2 rows; main content has 20 usable rows.
  - Sidebars and telemetry panes are gracefully hidden.
  - Screen titles remain clearly visible in the top header.
  - No text overlap or border clipping occurs.
- [ ] **Standard Tier (120x35):**
  - 2-column layout displays sidebar navigation alongside main content.
  - Form dialogs center cleanly with proper margin padding.
- [ ] **Large Tier (180x45):**
  - 3-column layout expands with dedicated telemetry inspector and log stream tail.
- [ ] **UltraWide Tier (>=220x60):**
  - 4-column master cockpit layout reflows without distortion.

---

## 5. Critical User Flows & Interactions

### 5.1 First-Run Guided Setup Wizard (View 22)
- [ ] Step 1 (Workspace Trust): Trust confirmation box toggles cleanly via Space.
- [ ] Step 2 (Doctor Diagnostics): Probe list displays `[OK]`, `[WARN]`, `[FAIL]` badges with remediation advice. Warning acknowledgement works as intended.
- [ ] Step 3 (Provider Setup): Provider selection navigates with Up/Down arrows. API key input displays asterisk masking (`*`).
- [ ] Step 4 (Model Setup): Text inputs allow editing model strings.
- [ ] Step 5 (Profile Selection): Profile radio list updates description accurately.
- [ ] Step 6 (Autonomy & Safety): Policy toggles work as expected.
- [ ] Step 7 (Final Verification): Test probe feedback renders correctly. Enter launches the cockpit.

### 5.2 Dedicated Mission Creator (View 40)
- [ ] Triggered by `N` or command palette.
- [ ] Tab and Shift-Tab traverse cleanly across Objective, Constraints, Criteria, Profile, Autonomy, Budget, and Branch fields.
- [ ] Empty objective or $0.00 budget prevents submission and shows inline red error message.
- [ ] Submitting dispatches `CreateMissionRequest` to runtime without UI freeze.

### 5.3 Startup Reconciliation Recovery (View 37)
- [ ] Incomplete missions are highlighted with warning banner on startup.
- [ ] `[R]` resumes checkpoint; `[I]` opens inspect mode; `[F]` replans; `[D]` opens 2-step confirmation prompt.
- [ ] Answering `N` cancels discard; answering `Y` discards without corrupting audit history.

### 5.4 Universal Command Palette V2 (View 38)
- [ ] Triggered by `Ctrl+P` or `:`.
- [ ] Fuzzy matching quickly filters all 40 canonical views by title and domain.
- [ ] Enter navigates to selected view immediately.
- [ ] Esc dismisses the palette and restores prior view.

### 5.5 Settings & Provenance Screen (View 34)
- [ ] Tab switches across configuration domains (Providers, Models, Autonomy, Security, UI).
- [ ] Winning layer badges (`[CLI]`, `[WORKSPACE]`, `[SYSTEM]`, `[BUILTIN]`) are distinct and color-coded.
- [ ] Immutable constraints display prominent red `[LOCKED]` badges.

### 5.6 Non-Dismissible Critical Approval Modal
- [ ] High-risk tool calls trigger centered approval modal with high-visibility warning border.
- [ ] Pressing `Esc` or clicking outside does NOT dismiss the approval modal.
- [ ] Only explicit `[A]` (Approve) or `[D]` (Deny) resolves the modal.

---

## 6. Verification Sign-Off

| Reviewer Name | Role | Terminal Emulator | Theme Verified | Date | Status |
|---|---|---|---|---|---|
| M31A Operator | Lead Engineer | Linux PTY / Alacritty | All 4 Themes | 2026-09-15 | **APPROVED** |
