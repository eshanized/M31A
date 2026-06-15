# TUI Screen Reference

M31 Autonomous uses a Bubble Tea TUI with 29 screens. The active screen depends on the current `AppState`.

---

## Init Screen (ScreenFirstRun)
Shown during startup while M31 Autonomous:
- Loads configuration (`~/.m31a/config.toml`)
- Resolves API keys (env → keychain → config)
- Checks provider health (OpenRouter/Zen)
- Checks for updates (version comparison)
- Appears as a logo animation and loading indicator

---

## Ready Screen (ScreenREPL)
The main interaction screen. Layout:
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

---

## Processing Screen (ScreenExecute)
Shown while waiting for the LLM to respond:
- Spinner animation (configurable style)
- "Processing..." status text
- Input area locked
- Cancel with `Esc`

---

## Streaming Screen
Live response streaming:
- Tokens appear in real-time
- Tool calls render as expandable cards
- Thinking blocks dimmed/italic (configurable)
- Backpressure-aware rendering (doesn't block the stream)

---

## Error Screen
Shown on critical errors:
- Error message with details
- Recovery options
- Press any key to return to Ready state

---

## Confirm Quit Screen (ScreenConfirmQuit)
Shown when quitting during active processing:
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

---

## Session Picker (ScreenResume)
Activated via `/sessions` command:
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

---

## Session Detail (ScreenSessionDetail)
Shows detailed info about a session before loading it:
- Session ID, Model, Provider, Phase
- Message count and preview
- Resume with Enter, Back with Esc

---

## Ghost Picker (ScreenGhostPicker)
Activated via `/ghost` command:
```
┌──────────────────────────────────┐
│  Ghost Write Files               │
│                                  │
│  Select files to generate:       │
│                                  │
│  >  [x] src/api/handler.go      │
│     [ ] src/api/router.go       │
│     [ ] tests/api_test.go       │
│                                  │
│  [Space] toggle  [Enter] write  │
└──────────────────────────────────┘
```

---

## Ghost Output (ScreenGhostOutput)
Shows results of ghost write operation:
- Files created/appended
- Warnings (if any)
- Content preview for selected file

---

## Bisect Output (ScreenBisect)
Shows model comparison results:
```
┌──────────────────────────────────┐
│  Model A  vs  Model B           │
│  ───────────────────────────    │
│  Similarity: 87%                │
│                                  │
│  + Added lines (green)          │
│  - Removed lines (red)          │
│    Common lines (white)         │
└──────────────────────────────────┘
```

---

## Model Selector (ScreenModelSelector)
Model/provider picker with fuzzy search:
- Lists all available models
- Cost comparison
- Capability filtering

---

## Settings (ScreenSettings)
Settings editor with 6 tabs:
- Provider configuration
- Model defaults
- UI preferences
- Permission modes
- Feature toggles
- Keychain management

---

## Plan Review (ScreenPlan)
Plan review screen:
- Task graph with dependencies
- File list and acceptance criteria
- Approve, refine, or reject

---

## Verify (ScreenVerify)
Verification results screen:
- Test results
- Validation output
- Self-heal options

---

## Ship (ScreenShip)
Ship summary screen:
- Commit details
- Changes summary
- Ledger entry

---

## Diff Viewer (ScreenDiff)
Full diff viewer:
- Syntax-highlighted diff
- Line-by-line navigation

---

## Ledger (ScreenLedger)
Learning ledger browser:
- Pattern tracking across sessions
- Statistics and insights

---

## Rollback (ScreenRollback)
Commit time machine:
- Browse commit history
- Soft reset to any commit

---

## Goal Input (ScreenGoalInput)
Full-screen goal entry:
- Text input for workflow goal
- Confirmation

---

## Discuss (ScreenDiscuss)
Discuss Q&A screen:
- One-by-one question presentation
- Timeout support
- Skip all option

---

## Metrics (ScreenMetrics)
Session analytics:
- Token usage over time
- Cost breakdown
- Performance stats

---

## Config Viewer (ScreenConfig)
Full config viewer:
- Read-only config display
- JSON/TOML formatted

---

## Help (ScreenHelp)
Keybinding help overlay:
- All keybindings listed
- Screen-specific shortcuts

---

## Theme Picker (ScreenThemePicker)
Theme browser/preview:
- Dark/Light/Auto themes
- Preview before apply

---

## Notifications (ScreenNotifications)
Notification history:
- All past notifications
- Type-colored entries
- Scrollable list

---

## Dashboard (ScreenDashboard)
Workflow pipeline overview:
- Phase progress bar
- Current goal and model
- Activity timeline

---

## File Explorer (ScreenFileExplorer)
File tree browser:
- Project file tree
- Expandable directories
- Navigate with j/k

---

## Tool Detail (ScreenToolDetail)
Expandable tool output:
- Full tool call results
- Copy content

---

## Phase Model Picker (ScreenPhaseModelPicker)
Dual-model picker:
- Planning model (Discuss/Plan/Verify)
- Coding model (Execute/Ship)
- Tab to cycle, Enter to confirm

---

## Permission Modal (ScreenPermission)
Tool permission modal:
- Allow/Deny/Always allow
- Countdown timer
- Tool details

---

## Command Palette
Overlay command palette:
- Fuzzy search commands
- Category grouping
- Keyboard shortcuts

---

## Screen Count Summary

| Category | Count |
|----------|-------|
| Core screens | 6 (REPL, FirstRun, Settings, ModelSelector, Help, Config) |
| Workflow screens | 6 (GoalInput, Discuss, Plan, Execute, Verify, Ship) |
| Data viewers | 5 (Resume, SessionDetail, Ledger, Metrics, Diff) |
| Git screens | 2 (Rollback, Bisect) |
| UI screens | 5 (ThemePicker, Notifications, Dashboard, FileExplorer, ToolDetail) |
| Ghost screens | 2 (GhostPicker, GhostOutput) |
| Modals | 3 (Permission, ConfirmQuit, PhaseModelPicker) |
| **Total** | **29** |
