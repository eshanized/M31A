# TUI Screen Reference

M31 Autonomous uses a Bubble Tea TUI with 33 screens. The active screen is determined by the current `Screen` constant.

---

## Screen Index

| # | Screen | Label | Purpose |
|---|--------|-------|---------|
| 0 | ScreenFirstRun | Setup | API key setup wizard |
| 1 | ScreenREPL | Chat | Main chat interface |
| 2 | ScreenModelSelector | Models | Model/provider picker |
| 3 | ScreenSettings | Settings | Settings editor (6 tabs) |
| 4 | ScreenResume | Sessions | Session browser |
| 5 | ScreenPermission | Permission | Tool permission modal |
| 6 | ScreenPlan | Plan | Plan review |
| 7 | ScreenExecute | Execute | Task execution progress |
| 8 | ScreenVerify | Verify | Verification results |
| 9 | ScreenShip | Ship | Ship summary |
| 10 | ScreenDiff | Diff | Git diff viewer |
| 11 | ScreenLedger | Ledger | Learning ledger browser |
| 12 | ScreenRollback | Rollback | Commit time machine |
| 13 | ScreenGoalInput | Goal | Full-screen goal entry |
| 14 | ScreenDiscuss | Discuss | Discuss Q&A |
| 15 | ScreenMetrics | Metrics | Session analytics |
| 16 | ScreenConfig | Config | Full config viewer |
| 17 | ScreenHelp | Help | Keybinding help overlay |
| 18 | ScreenBisect | Bisect | Git bisect interactive |
| 20 | ScreenNotifications | Notifications | Notification history |
| 21 | ScreenDashboard | Dashboard | Workflow pipeline overview |
| 22 | ScreenSessionDetail | Session | Session detail preview |
| 23 | ScreenFileExplorer | Files | File tree browser |
| 24 | ScreenToolDetail | Tool Output | Expandable tool output |
| 25 | ScreenPhaseModelPicker | Model Setup | Dual-model picker |
| 26 | ScreenGhostPicker | Ghost Picker | Ghost write file selector |
| 27 | ScreenGhostOutput | Ghost Output | Ghost write results |
| 28 | ScreenConfirmQuit | Confirm Quit | Confirm quit dialog |
| 29 | ScreenChatHistory | Chat History | Chat history table browser |
| 30 | ScreenCommandPalette | Commands | Command palette with detail panel |
| 31 | ScreenRuntimeCheck | Runtime Check | Runtime verification |
| 32 | ScreenHome | Home | Landing screen with logo and tips |
| 33 | ScreenDecisions | Decisions | Decision log browser |

---

## Core Screens

### Home (ScreenHome)

Landing screen shown on startup. Displays logo, recent sessions, and quick-start tips. Submit a prompt to begin a workflow.

### REPL (ScreenREPL)

The main interaction screen:

```
┌─────────────────────────────────────────────────┐
│ Header: Model | Tokens | Cost | Health Status   │
├─────────────────────────────────────────────────┤
│                                                 │
│  Conversation area (scrollable)                 │
│  - User messages (styled)                       │
│  - Assistant responses (streamed in real-time)  │
│  - Tool call cards (collapsible)                │
│  - Thinking blocks (collapsible, dimmed)        │
│                                                 │
├─────────────────────────────────────────────────┤
│ Input bar: > _                                  │
│ Suggestions dropdown (on / command partial)     │
└─────────────────────────────────────────────────┘
```

### First Run (ScreenFirstRun)

API key setup wizard shown on first launch. Guides through:
- Provider selection (OpenRouter, Zen, Nvidia)
- API key entry
- Model selection

### Settings (ScreenSettings)

Settings editor with 6 tabs:
- Provider configuration
- Model defaults
- UI preferences
- Permission modes
- Feature toggles
- Keychain management

### Config Viewer (ScreenConfig)

Full config viewer showing all TOML configuration in read-only mode.

### Help (ScreenHelp)

Keybinding help overlay listing all keyboard shortcuts by context.

---

## Workflow Screens

### Goal Input (ScreenGoalInput)

Full-screen goal entry for workflow initiation. Text input with confirmation.

### Discuss (ScreenDiscuss)

Q&A screen for the Discuss phase. One question at a time with:
- Timeout support
- Skip all option
- Answer completeness scoring

### Plan (ScreenPlan)

Plan review screen:
- Task graph with dependencies
- File list and acceptance criteria
- Approve, refine, or reject

### Execute (ScreenExecute)

Task execution progress screen:
- Live tool call output
- Task status indicators
- Self-heal attempt tracking

### Verify (ScreenVerify)

Verification results screen:
- Test results
- Validation output
- Self-heal options

### Ship (ScreenShip)

Ship summary screen:
- Commit details
- Changes summary
- Ledger entry
- Changelog

### Runtime Check (ScreenRuntimeCheck)

Runtime verification screen:
- Dev server status
- HTTP smoke test results
- Route discovery

---

## Data Viewers

### Session Browser (ScreenResume)

Session list with preview:
```
┌──────────────────────────────────┐
│  Sessions                        │
│                                  │
│  >  abc12345  My session       │
│     def67890  Code review      │
│     ...                         │
│                                  │
│  [↑/↓] navigate  [Enter] select │
└──────────────────────────────────┘
```

### Session Detail (ScreenSessionDetail)

Detailed info about a session:
- Session ID, Model, Provider, Phase
- Message count and preview
- Resume with Enter, Back with Esc

### Chat History (ScreenChatHistory)

Table browser for past conversations within a session.

### Ledger (ScreenLedger)

Learning ledger browser:
- Pattern tracking across sessions
- Statistics and insights

### Metrics (ScreenMetrics)

Session analytics:
- Token usage over time
- Cost breakdown
- Performance stats

### Diff Viewer (ScreenDiff)

Full diff viewer:
- Syntax-highlighted diff
- Line-by-line navigation

---

## Git Screens

### Rollback (ScreenRollback)

Commit time machine:
- Browse commit history
- Soft/hard/safe reset with preview
- Backup branch creation

### Bisect (ScreenBisect)

Git bisect interactive:
- Mark commits as good/bad
- Binary search for offending commit

---

## UI Screens

### Model Selector (ScreenModelSelector)

Model/provider picker with fuzzy search:
- Lists all available models
- Cost comparison per token
- Capability filtering

### Notifications (ScreenNotifications)

Notification history:
- All past notifications
- Type-colored entries
- Scrollable list

### Dashboard (ScreenDashboard)

Workflow pipeline overview:
- Phase progress bar
- Current goal and model
- Activity timeline

### File Explorer (ScreenFileExplorer)

File tree browser:
- Project file tree
- Expandable directories
- Navigate with j/k

### Tool Detail (ScreenToolDetail)

Expandable tool output:
- Full tool call results
- Copy content

### Decisions (ScreenDecisions)

Decision log browser:
- Architectural decisions recorded during workflow
- Timestamped entries

---

## Modals

### Permission Modal (ScreenPermission)

Tool permission prompt:
- Allow / Allow Always / Deny / Exit
- Countdown timer
- Tool details and risk level

### Confirm Quit (ScreenConfirmQuit)

Quit confirmation during active processing:
```
┌──────────────────────────────────┐
│  Confirm Quit                    │
│                                  │
│  An operation is in progress.    │
│  Are you sure you want to quit?  │
│                                  │
│  [y] Yes, quit    [n] No, stay  │
└──────────────────────────────────┘
```

### Phase Model Picker (ScreenPhaseModelPicker)

Dual-model picker:
- Planning model (Discuss/Plan/Verify)
- Coding model (Execute/Ship)
- Tab to cycle, Enter to confirm

### Command Palette (ScreenCommandPalette)

Fuzzy search command palette:
- All registered slash commands
- Category grouping (Core, AI, Config, Session, Git, Workflow)
- Keyboard shortcut display
