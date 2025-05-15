# M31A TUI Deep Testing & Bug Hunt Prompt

## ROLE

You are a **Senior QA Engineer and Go Developer** tasked with finding every remaining bug, issue, and UX problem in the M31A TUI. You must actually RUN the application, interact with every screen, test every feature, and report issues found.

## PROJECT CONTEXT

M31A is a terminal AI coding assistant built with:
- Go 1.22+, Bubble Tea + Lipgloss + Bubbles + Glamour
- Binary at: `/home/snigdha/Desktop/Helix/M31A/m31a`
- Config at: `~/.m31a/config.toml`
- Sessions at: `~/.m31a/sessions/`

The TUI has these screens: FirstRun (API key setup), REPL (main chat), Settings (6-tab config editor), Resume (session browser), ModelSelector, Permission modal, and workflow screens (Plan/Execute/Verify/Ship).

## WHAT'S BEEN FIXED ALREADY

Do NOT re-report these — they are already fixed:
1. Viewport height miscalculation (was `Height - 2`, now `Height - 4`)
2. Sidebar width not subtracted from REPL layout
3. Sidebar toggle not triggering REPL resize
4. ANSI-unsafe header truncation (raw rune slicing)
5. Sidebar auto-showing "loading..." on empty screen
6. Binary content auto-collapse bug (`isBinary` never set to true)
7. REPL showing "Initializing..." after creation (no WindowSizeMsg sent)
8. Nil dereference when streaming without a model selected
9. `/config` command not persisting to disk
10. `/theme` not switching at runtime
11. Slash commands completely broken (was checking `KeyMsg.String()` instead of textarea content)

## YOUR MISSION

### Step 1: Build
```
cd /home/snigdha/Desktop/Helix/M31A
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

### Step 2: Runtime Testing (INTERACTIVE — you must actually run the TUI)

Launch `./m31a` and systematically test EVERY feature. For each area below, note what works, what breaks, and any visual/layout issues.

#### A. First-Run Flow (no API key)
1. Start with NO config file (`rm -rf ~/.m31a && ./m31a`)
2. Go through the first-run wizard
3. Test: Welcome screen → Provider select → Skip option
4. Verify: After skipping, does the REPL show properly (header, viewport with welcome message, textarea, status bar)?
5. Test: Does the REPL show a helpful message when you try to send a message without a model?
6. Test: Can you open `/settings` and configure an API key after skipping?

#### B. First-Run Flow (with API key)
1. If you have an OpenRouter or Zen API key, test the full setup flow
2. Test: Provider selection → Key input → Validation → Keychain prompt
3. Verify: After setup, does the REPL show properly with header, viewport, textarea, status bar?

#### C. REPL Screen
1. **Layout**: Does the header show (M31A brand, provider badge, context, health)?
2. **Layout**: Does the viewport fill the available space properly (no overflow, no underflow)?
3. **Layout**: Does the textarea appear at the bottom, just above the status bar?
4. **Layout**: Is the status bar visible at the very bottom?
5. **Welcome message**: Is it visible and readable on first launch?
6. **Message sending**: Type a message, press Enter. Does it appear as a user bubble?
7. **Message sending**: Does the assistant response stream properly?
8. **Textarea**: Can you type multi-line text? (Shift+Enter for newline)
9. **Textarea**: Does placeholder text show when empty?
10. **Input history**: Press Up/Down arrows to navigate previous inputs
11. **Scrolling**: Type enough messages to overflow the viewport. Does PgUp/PgDn work?
12. **Streaming cancel**: During a response, press Ctrl+C. Does it cancel properly?

#### D. Slash Commands (CRITICAL — test ALL of these)
Type each command in the REPL textarea and press Enter. Verify each one works:

| Command | Expected Behavior |
|---------|------------------|
| `/help` | Shows list of all commands |
| `/clear` | Clears the conversation |
| `/status` | Shows current session info |
| `/model` | Shows current model |
| `/provider` | Shows current provider |
| `/reset` | Resets the session, goes to first-run |
| `/quit` | Exits the app |
| `/undo` | Undoes last action |
| `/compress` | Compresses conversation context |
| `/ledger` | Shows learning ledger |
| `/rollback` | Shows rollback options |
| `/sessions` | Opens session browser |
| `/goal` | Sets workflow goal |
| `/phase initialize` | Starts initialize phase |
| `/config` | Shows current config |
| `/config ui.theme dark` | Switches theme (verify it actually changes!) |
| `/models` | Opens model browser |
| `/fallback` | Tests provider fallback |
| `/tools` | Lists available tools |
| `/workflow test` | Starts full workflow |
| `/history` | Shows command history |
| `/diff` | Shows git diff |
| `/theme dark` | Switches theme |
| `/theme light` | Switches theme |
| `/save` | Saves session |
| `/key` | Manages API keys |
| `/log` | Shows logs |
| `/tokens` | Shows token usage |
| `/health` | Shows health status |
| `/run echo hello` | Runs bash command |
| `/settings` | Opens settings screen |
| `/resume` | Opens session resume screen |

For each command, note: Does it work? Does it produce output? Does it crash? Does it return to the REPL properly?

#### E. Settings Screen
1. Open `/settings`
2. Navigate tabs: Provider, Model, UI, Workflow, Model Cost, Ledger
3. Test: Can you edit fields? Does inline editing work?
4. Test: Press Ctrl+S to save. Does it persist?
5. Test: Tab/Shift+Tab navigation between tabs
6. Test: Search functionality within settings
7. Test: Does the screen fit properly on different terminal sizes?

#### F. Model Selector
1. Open `/models`
2. Test: Does it show providers and models?
3. Test: Can you select a model? Does it switch properly?
4. Test: Search/filter models
5. Test: Press Esc to go back

#### G. Session Resume
1. Open `/resume`
2. Test: Does it list existing sessions?
3. Test: Can you preview a session?
4. Test: Can you load a session into the REPL?

#### H. Sidebar (Ctrl+B)
1. Press Ctrl+B to toggle sidebar
2. Test: Does sidebar show git status?
3. Test: Does the REPL resize properly when sidebar appears/disappears?
4. Test: Is there no visual overlap or clipping?

#### I. Command Palette (Ctrl+P)
1. Press Ctrl+P to open command palette
2. Test: Does it show all commands?
3. Test: Fuzzy search — type partial command names
4. Test: Press Enter to execute selected command
5. Test: Press Esc to close
6. **Visual bug check**: Does the palette overlay properly or does it push content down?

#### J. Permission Modal
1. Trigger a permission request (e.g., use `/run` with a dangerous command, or start a workflow that needs file operations)
2. Test: Does the modal appear centered?
3. Test: Does the countdown timer work?
4. Test: Y/A/N/E keys work properly
5. Test: Does it auto-deny on timeout?

#### K. Workflow Screens
1. Start `/workflow fix a bug in main.go`
2. **Initialize phase**: Does it gather context properly?
3. **Discuss phase**: Does it ask clarifying questions?
4. **Plan phase**: Does the plan screen show tasks properly?
5. **Execute phase**: Does the execute screen show task progress?
6. **Verify phase**: Does verification run and show results?
7. **Ship phase**: Does it show completion summary?
8. Test: Can you cancel a running workflow with Ctrl+C?

#### L. Theme Switching
1. Test `/theme dark` — verify colors change immediately
2. Test `/theme light` — verify colors change immediately
3. Check: Do all screens respect the theme (header, status bar, REPL, settings, etc.)?

#### M. Edge Cases & Stress Tests
1. **Tiny terminal**: Resize terminal to 40x10. Does it show "Terminal too small" message?
2. **Large terminal**: Resize to a very large terminal. Does layout scale properly?
3. **Rapid resizing**: Resize terminal rapidly. Does the layout adapt without glitches?
4. **Empty config**: Start with empty config file. Does it use defaults?
5. **Invalid config**: Start with malformed config. Does it handle gracefully?
6. **No git repo**: Run M31A outside a git repo. Does sidebar handle it?
7. **Long messages**: Send a very long message. Does it wrap properly?
8. **Many messages**: Send 50+ messages. Does scrolling work? Does performance degrade?
9. **Concurrent operations**: Try opening settings while a workflow is running
10. **Memory leak check**: Run for 5+ minutes with active streaming. Does memory grow unbounded?

### Step 3: Code Review

After runtime testing, review these specific areas for bugs:

1. **`internal/tui/keybindings.go`**: The KeyRegistry is initialized and bindings are registered, but `Handle()` is never called. All chord bindings (ctrl+x b, ctrl+x s, etc.) are dead code. Should these be wired up or removed?

2. **`internal/tui/app.go` renderWithPalette()**: Command palette is appended below screen content instead of overlaying. This causes scrolling issues. Should use `lipgloss.Place` for centering or replace main content.

3. **`internal/tui/commands.go` handleRun()**: `/run` executes bash commands without permission check. This bypasses the permission modal. Should require permission for dangerous tools.

4. **`internal/tui/commands.go` handlePhase()**: The `/phase` command in the registry only saves to session file, doesn't trigger execution. The actual execution is in the hardcoded handler in app.go. This is confusing duplication.

5. **`internal/tui/components/toolcard.go`**: Check for any remaining nil pointer issues or layout bugs.

6. **`internal/tui/settings.go`**: Check tab navigation, field editing, and save functionality for bugs.

7. **`internal/tui/streaming.go`**: Check for goroutine leaks in StartStreamCmd when context is cancelled.

8. **Race conditions**: Check if `TaskStartMsg` can arrive before `executeModel` is initialized (workflow tasks run concurrently).

### Step 4: Report

Write a comprehensive report to `/home/snigdha/Desktop/Helix/M31A/rush/tui_deep_test_report.md` with:

1. **Critical bugs** — crashes, nil dereferences, data loss
2. **High severity** — broken features, incorrect behavior
3. **Medium severity** — UX issues, minor bugs, layout glitches
4. **Low severity** — cosmetic issues, code quality improvements
5. **Test results table** — for each slash command, did it pass or fail?
6. **Screenshot descriptions** — for visual bugs, describe what you see
7. **Reproduction steps** — for each bug, provide exact steps to reproduce

Be thorough. The goal is to find every remaining issue before release.
