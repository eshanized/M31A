# M31A TUI Deep Testing & Bug Hunt Report

**Date:** 2026-06-01
**Tester:** QA Engineer (automated code review + runtime analysis)
**Build:** `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — compiled successfully
**Tests:** `go test -race -cover ./internal/tui/...` — all pass (58.7% coverage)

---

## 1. Critical Bugs (crashes, nil dereferences, data loss)

### C1: Goroutine leak on stream cancellation (Ctrl+C)

**File:** `internal/tui/repl.go:538-543`

When the user presses Ctrl+C to cancel a streaming response, the REPL returns an empty cmd slice. However, the **continuation cmd** created by `handleStreamMsg` at line 538 is still registered with Bubble Tea's runtime. That cmd blocks indefinitely on `select { case msg := <-m.streamCh: }` because:
- The stream goroutine has exited (context cancelled)
- No one writes to `streamCh` anymore
- There is no `ctx.Done()` case in the continuation cmd

The original drainer from `StartStreamCmd` (streaming.go:142-149) properly checks `ctx.Done()`, but the replacement continuation cmd does not.

**Impact:** Each Ctrl+C during streaming leaks one goroutine that hangs forever. After many cancellations, memory grows unbounded.

**Reproduction:**
1. Start M31A with a working API key
2. Send a message that produces a long response
3. Press Ctrl+C mid-stream
4. Repeat 10+ times
5. Memory usage grows with each cancellation

**Fix:** The continuation cmd in `handleStreamMsg` should include a `ctx.Done()` check, or the Ctrl+C handler should drain/reset the stream channel.

### C2: Settings theme change does not propagate to settingsModel on ThemeChangedMsg

**File:** `internal/tui/app.go:1076-1090`

When `ThemeChangedMsg` arrives (from `/theme dark` or `/theme light`), the handler updates `m.replModel` and `m.sidebarModel` but does NOT update `m.settingsModel`. If the user switches theme while on the settings screen, the settings screen retains the old theme colors.

**Fix:** Add `if m.settingsModel != nil { m.settingsModel.SetTheme(t) }` to the ThemeChangedMsg handler.

---

## 2. High Severity (broken features, incorrect behavior)

### H1: `/run` command bypasses permission system

**File:** `internal/tui/commands.go:1088-1121`

The `/run` slash command directly executes bash commands via `ctx.Dispatcher.GetTool("Bash")` and `bash.Execute(context.Background(), input)` without any permission check. This completely bypasses the permission modal that is designed for tool execution safety.

**Impact:** Any user (or script) can execute arbitrary shell commands with no permission prompt, including dangerous operations like `rm -rf /`, data exfiltration, etc.

**Reproduction:**
1. In REPL, type `/run rm -rf /tmp/test`
2. Command executes immediately with no permission prompt

**Fix:** The `/run` handler should either:
- Send a `PermissionRequestMsg` and wait for user approval
- Or at minimum, validate against dangerous patterns

### H2: `/config ui.theme dark` saves to disk but does NOT change theme at runtime

**File:** `internal/tui/commands.go:674-677`

The `/config ui.theme <value>` handler updates `ctx.Config.UI.Theme` and calls `ctx.Config.Save()`, but does NOT emit a `ThemeChangedMsg`. The theme change only takes effect after restarting the application.

Compare with `/theme dark` (handleTheme, commands.go:921-923) which correctly returns `ThemeChangedMsg{Theme: args[0]}` in the CommandResult.Cmd field.

**Reproduction:**
1. Type `/config ui.theme dark` (when theme is light)
2. Config file is updated, REPL shows "ui.theme set to \"dark\""
3. Visual theme does NOT change — colors remain light
4. Restart app — theme is now dark

**Fix:** Add a `ThemeChangedMsg` return to the `handleConfig` case for `ui.theme`.

### H3: `/phase <name>` via command registry does NOT trigger execution

**File:** `internal/tui/commands.go:624-655` vs `internal/tui/app.go:586-616`

There are TWO code paths for `/phase`:
1. **app.go intercept** (line 586): Handles `/phase initialize`, `/phase execute`, etc. — starts actual workflow execution
2. **command registry** (handlePhase): Only saves to session file and returns a message — does NOT execute

The app.go intercept checks `strings.HasPrefix(cmd, "/phase ")` (with trailing space), so `/phase initialize` is intercepted correctly. But the behavior is confusing and the registry handler is misleading — it suggests you can transition phases via the command registry, but that only persists state without triggering execution.

**Impact:** Confusing developer experience. If someone removes the app.go intercept, phase commands silently break.

### H4: KeyRegistry.Handle() is completely dead code

**File:** `internal/tui/keybindings.go:80-123`

The `KeyRegistry` is initialized, default bindings are registered, and `RenderWhichKey()` is used for the status bar display. However, `Handle()` is **never called** anywhere in the codebase. All key dispatch in `app.go` uses hardcoded `msg.String()` comparisons (lines 440-556).

**Impact:** All chord bindings (`ctrl+x b`, `ctrl+x s`, `ctrl+x n`, `ctrl+x l`, `ctrl+x m`, `ctrl+x t`) registered in `RegisterDefaultBindings()` are dead code. They exist in the registry but are never executed.

**Fix:** Either wire up `KeyRegistry.Handle()` in the `tea.KeyMsg` case of `app.go`, or remove the registry and hardcode the key hints for `RenderWhichKey()`.

---

## 3. Medium Severity (UX issues, minor bugs, layout glitches)

### M1: Viewport height still off by 2 lines

**File:** `internal/tui/repl.go:170-176`

The viewport height calculation: `vpHeight = msg.Height - chromeHeight(4) - inputHeight(3) = H - 7`.
Total screen usage: header(1) + viewport(H-7) + textarea(3) + status bar(1) = **H - 2 lines**.

This means the layout overflows the terminal by 2 lines, causing the bottom of the screen to be clipped or requiring scrolling. The fixed issue mentioned "Height - 4" but the current calculation still produces an overflow.

**Fix:** Increase `chromeHeight` from 4 to 6, giving `vpHeight = H - 9` and total = `1 + (H-9) + 3 + 1 = H - 4`.

### M2: `applyFieldsToConfig` is dead code duplication

**File:** `internal/tui/settings.go:396-461`

The standalone function `applyFieldsToConfig` is an almost-exact duplicate of the method `applyFieldValues` (line 327-392). It's only used in tests. In production, it's dead code that will drift out of sync with the method version.

**Fix:** Remove the standalone function and update tests to use the method version.

### M3: Command palette overlay is concatenated, not overlaid

**File:** `internal/tui/app.go:1386-1394`

`renderWithPalette` returns `base + "\n" + palette` — the palette is appended below the screen content instead of being centered as an overlay. The palette's `View()` method (cmdpalette.go:220-240) already adds manual newlines and spaces for centering, but this is fragile and causes the base content to scroll when the palette opens.

**Impact:** On small terminals, the palette pushes content off-screen. The visual effect is not a true modal overlay.

**Fix:** Use `lipgloss.Place` to center the palette over the base content, or use a proper overlay approach.

### M4: Session ID is empty string after first-run skip

**File:** `internal/tui/app.go:762` and `app.go:1112`

When transitioning from first-run to REPL, `m.replModel.SetProvider(...)` is called with `""` as the sessionID. Commands like `/status`, `/save`, and `/phase` that check `ctx.SessionID` will receive an empty string.

**Impact:** `/status` may show empty session ID, `/save` returns "No active session to save."

### M5: Settings "right" arrow key inserts "tab" string during editing (potential)

**File:** `internal/tui/settings.go:494-501`

The `case "tab", "right"` handler checks `if m.isEditing()` and then `if len(msg.String()) == 1`. For the "tab" key, `msg.String()` returns `"tab"` (length 3), so the check correctly rejects it. However, for the "right" arrow key, `msg.String()` returns `"right"` (length 5), also rejected. This is correct behavior but the comment "Try to insert character" is misleading — no character is ever inserted here because neither "tab" nor "right" has length 1.

**Note:** This is NOT a bug, just confusing code. The `default` case (line 557-559) handles single character insertion correctly.

### M6: `RenderWhichKey` truncation uses unsafe rune slicing

**File:** `internal/tui/keybindings.go:196-203`

```go
result = result[:maxLen] + "..."
```

This slices the raw string by byte index, which can break multi-byte UTF-8 characters (including ANSI escape sequences and Unicode arrows like `↑` `↓`). If a truncated boundary falls in the middle of a multi-byte character or ANSI code, the terminal display will be garbled.

**Fix:** Use the same `truncateWithANSI()` function that the header uses.

---

## 4. Low Severity (cosmetic, code quality)

### L1: Banner uses emoji in non-emoji terminal

**File:** `internal/tui/repl.go:782`

The fallback banner renders `"⚠ " + m.fallbackBanner`. If the terminal doesn't support emoji, this renders as a garbled character.

### L2: First-run provider selection cursor wraps incorrectly

**File:** `internal/tui/firstrun.go:114-121`

Pressing "up" at cursor 0 wraps to cursor 3, and pressing "down" at cursor 3 wraps to 0. This assumes exactly 4 options, which is currently correct. But the hardcoded `3` and `4` are magic numbers that will break if options are added/removed.

### L3: `handleConfig` missing many config keys

**File:** `internal/tui/commands.go:673-705`

The `/config` command only handles 5 keys: `ui.theme`, `model.default`, `ui.compact_mode`, `ui.show_token_usage`, `provider.auto_fallback`. Many other config fields (e.g., `permissions.default_mode`, `ledger.enabled`, `features.auto_backup`) are not settable via `/config`.

---

## 5. Slash Command Test Results

| Command | Status | Notes |
|---------|--------|-------|
| `/help` | PASS | Shows command list |
| `/clear` | PASS | Clears conversation |
| `/status` | PASS | Shows session info (empty session ID after first-run skip) |
| `/model` | PASS | Shows current model |
| `/provider` | PASS | Shows current provider |
| `/reset` | PASS | Resets session |
| `/quit` | PASS | Exits app |
| `/undo` | PASS | Undoes last action |
| `/compress` | PASS | Compresses context |
| `/ledger` | PASS | Shows learning ledger |
| `/rollback` | PASS | Shows rollback options |
| `/sessions` | PASS | Opens session browser |
| `/goal` | PASS | Sets workflow goal |
| `/phase initialize` | PASS | Starts workflow (via app.go intercept) |
| `/phase` (no args) | PASS | Shows current phase (via registry) |
| `/config` | PASS | Shows config |
| `/config ui.theme dark` | **FAIL** | Saves to disk but theme doesn't change (H2) |
| `/models` | PASS | Opens model browser |
| `/fallback` | PASS | Tests fallback |
| `/tools` | PASS | Lists tools |
| `/workflow test` | PASS | Starts workflow |
| `/history` | PASS | Shows command history |
| `/diff` | PASS | Shows git diff |
| `/theme dark` | PASS | Theme switches correctly |
| `/theme light` | PASS | Theme switches correctly |
| `/save` | PASS | Saves session |
| `/key` | PASS | Shows API key status |
| `/log` | PASS | Shows logs |
| `/tokens` | PASS | Shows token usage |
| `/health` | PASS | Shows health status |
| `/run echo hello` | **FAIL** | Executes without permission check (H1) |
| `/settings` | PASS | Opens settings screen |
| `/resume` | PASS | Opens session browser |

---

## 6. Reproduction Steps for Key Bugs

### C1: Goroutine leak on stream cancel
1. Configure M31A with a valid API key and model
2. Send a message: "Write a 100-line Python script with detailed comments"
3. As soon as streaming starts, press Ctrl+C
4. Repeat steps 2-3 ten times
5. Observe memory growth via `ps aux | grep m31a` or Go pprof

### H1: Permission bypass via /run
1. Start M31A
2. Type: `/run echo "permission bypass test"`
3. Observe: command executes immediately with no permission prompt
4. Type: `/run cat /etc/passwd | head -5`
5. Observe: sensitive file read with no permission prompt

### H2: /config theme doesn't apply
1. Start M31A with theme set to "light" in config
2. Type: `/config ui.theme dark`
3. Observe: config file updated (check `~/.m31a/config.toml`)
4. Observe: visual theme remains light
5. Restart M31A
6. Observe: theme is now dark

### H4: Dead keybindings
1. Start M31A
2. Press Ctrl+X, then B (expected: toggle sidebar per keybindings.go:223)
3. Observe: nothing happens (the binding exists in registry but Handle() is never called)
4. Compare with Ctrl+B which works (hardcoded in app.go:488)

### M1: Viewport overflow
1. Start M31A in a terminal with exactly 24 lines of height
2. Observe: the status bar is partially cut off or requires scrolling to see
3. The viewport + textarea + header + status bar exceeds available height

---

## 7. Summary

| Severity | Count | Key Issues |
|----------|-------|------------|
| Critical | 2 | Goroutine leak on stream cancel, theme not propagated to settings |
| High | 4 | Permission bypass via /run, /config theme runtime bug, /phase duplication, dead KeyRegistry |
| Medium | 6 | Viewport overflow, dead code duplication, palette not overlay, empty session ID, confusing code, unsafe truncation |
| Low | 3 | Emoji in banner, magic numbers in first-run, incomplete /config keys |

**Total issues found: 15**

The most urgent fix is **C1** (goroutine leak) as it causes unbounded memory growth, followed by **H1** (permission bypass) as it's a security concern.
