---
phase: 05
title: Session State & Configuration — User Acceptance Testing
created: 2026-05-28
status: in-progress
test_count: 44
passed: 0
failed: 0
blocked: 0
skipped: 0
---

## Test List

### OS Keychain Abstraction (`pkg/keychain/`)

1. **KC-01**: `New()` factory returns non-nil Keychain on current platform (no compile errors, interface compliance)
2. **KC-02**: `Set("m31a/openrouter", "sk-or-v1-xxx")` + `Get("m31a/openrouter")` returns stored key
3. **KC-03**: `Get` on non-existent key returns `ErrKeyNotFound`
4. **KC-04**: `Delete` removes stored key; subsequent `Get` returns `ErrKeyNotFound`
5. **KC-05**: Service name validation — `m31a/openrouter` allowed; `m31a/../../secret` rejected
6. **KC-06**: `Delete` on non-existent key returns `ErrKeyNotFound`
7. **KC-07**: Empty service name returns error
8. **KC-08**: `New()` factory on unsupported platform (Windows) returns a Keychain whose all methods return `ErrNotImplemented`

### Session Lifecycle (`pkg/session/`)

9. **SES-01**: `NewSession()` creates session directory with `session.json` containing ID, model, provider, StartedAt as RFC3339
10. **SES-02**: `SaveSession()` writes `session.json` and `messages.json` atomically (no `.tmp` files left after completion)
11. **SES-03**: `LoadSession(id)` reconstructs Session struct matching saved state, including Messages
12. **SES-04**: `ListSessions()` returns all sessions in sorted order (most recent first)
13. **SES-05**: `DeleteSession(id)` removes session directory entirely
14. **SES-06**: `ArchiveSession(id)` moves session to `archived/` subdirectory
15. **SES-07**: Corrupt `session.json` — `ListSessions()` marks it with `Corrupted=true`, `LoadSession()` returns `ErrSessionCorrupted`
16. **SES-08**: Missing `messages.json` — `LoadSession()` succeeds with empty `Messages` slice (graceful degradation)
17. **SES-09**: Unique IDs — two `NewSession()` calls produce different 8-char hex IDs
18. **SES-10**: `ArchiveSession` loads correctly from `archived/` directory after archive

### Planning File Persistence (`pkg/session/planning.go`)

19. **PL-01**: `SaveProject/LoadProject` roundtrip — write Goal, Type, Framework, Q&A map, read back identical structure
20. **PL-02**: `SaveTasks/LoadTasks` roundtrip — write markdown table with ID, Action, Description, Deps (comma-separated), Status, Files; read back reconstructs `[]int` deps and `[]string` files correctly
21. **PL-03**: `SaveState/LoadState` roundtrip — write Phase, Progress, Last Action with RFC3339 timestamp; read back maps phase string to correct `WorkflowPhase` enum
22. **PL-04**: Missing planning files — `LoadProject()`, `LoadTasks()`, `LoadState()` on nonexistent paths return zero values (nil/empty) with nil error (graceful degradation)
23. **PL-05**: TASKS.md with empty dependencies (`-` in deps column) yields empty `[]int` — no parse error
24. **PL-06**: TASKS.md with multiple files in Files column (comma-separated) yields correct `[]string`

### Checkpoint System (`pkg/session/checkpoint.go`)

25. **CK-01**: `SaveCheckpoint` + `LoadCheckpoints` roundtrip — save checkpoint, load back, verify Phase, Progress, LastAction, and RFC3339 timestamp preserved
26. **CK-02**: Max 2 retention — save 3 checkpoints, `LoadCheckpoints()` returns only the 2 most recent
27. **CK-03**: `LatestCheckpoint()` returns most recent checkpoint; empty checkpoint state returns `ErrCheckpointNotFound`
28. **CK-04**: Empty checkpoint state — `LatestCheckpoint()` returns `ErrCheckpointNotFound`

### Config Loader (`internal/config/loader.go`)

29. **CFG-01**: `Load()` parses valid TOML file and populates all Config fields correctly
30. **CFG-02**: `Load()` on missing file returns `DefaultConfig()` with nil error (no crash on first run)
31. **CFG-03**: `Save()` writes valid TOML that can be re-loaded with `Load()`
32. **CFG-04**: `M31A_CONFIG` env var overrides the config file path argument for both `Load()` and `Save()`
33. **CFG-05**: `M31A_THEME` env var overrides `Config.UI.Theme` after TOML parse
34. **CFG-06**: `M31A_DEFAULT_MODEL` env var overrides `Config.Model.Default` after TOML parse
35. **CFG-07**: `ResolveAPIKeys()` resolves in order: env var > keychain > config file — env var takes priority for OpenRouter
36. **CFG-08**: `ResolveAPIKeys()` with no env var and no keychain + config file key returns the config file key value
37. **CFG-09**: `ResolveAPIKeys()` with no keys configured anywhere returns empty strings (no error)
38. **CFG-10**: `Save()` creates parent directory with `os.MkdirAll` if it doesn't exist

### Token Estimator (`internal/tokens/estimator.go`)

39. **TK-01**: `Estimate()` with known model (gpt-4o) uses tiktoken and returns non-zero token count
40. **TK-02**: `Estimate()` with unknown model uses rune-based fallback (`len([]rune(text))/4*1.3`)
41. **TK-03**: `Calibrate()` updates EMA factor (alpha=0.3); factor converges toward 1.0 after multiple calibrations
42. **TK-04**: EMA factor clamped to [0.1, 10.0] — extreme single calibration ratio doesn't exceed bounds
43. **TK-05**: `FormatUsage(500, 1000)` returns `"500 / 1000 (50%)"`
44. **TK-06**: `ContextWarningBanner(0.5, 128000)` returns empty string below threshold (default 80%)
45. **TK-07**: `ContextWarningBanner(0.95, 128000)` returns styled warning string at 80%+ threshold

### Settings Screen (`internal/tui/settings.go`)

46. **ST-01**: Settings initial render shows 4 tabs: General, Provider, Permissions, Features
47. **ST-02**: Tab key cycles forward (0→1→2→3→0); Shift+Tab cycles backward (0→3→2→1→0)
48. **ST-03**: Each tab's View() returns distinct content — Provider tab shows API key fields masked as `••••••••`
49. **ST-04**: `Esc` key returns to previous screen (emits `AppMsg{Screen: ScreenREPL}`)
50. **ST-05**: Save action writes config via `cfg.Save()` and emits `AppMsg{Screen: ScreenREPL}`

### Resume Screen (`internal/tui/resume.go`)

51. **RS-01**: Resume initial render shows session list with footer hints: `Enter: resume, N: new, D: delete`
52. **RS-02**: Sessions with corrupt `session.json` show `[!]` badge in list item description
53. **RS-03**: `Enter` on a session emits `AppMsg{Screen: ScreenREPL, SessionID: "..."}`
54. **RS-04**: `N` key emits `AppMsg{Screen: ScreenREPL, SessionID: ""}` for new session
55. **RS-05**: `D` key on selected item shows delete confirmation overlay
56. **RS-06**: `Y` in delete confirmation removes session and refreshes list; `N` dismisses overlay
57. **RS-07**: `Esc` on main list returns to previous screen (emits `AppMsg{Screen: ScreenREPL}`)
58. **RS-08**: Window resize updates list dimensions correctly

## Results

| # | ID | Description | Status | Notes |
|---|----|-------------|--------|-------|
| 1 | KC-01 | Factory returns non-nil Keychain | pending | |
| 2 | KC-02 | Set/Get roundtrip | pending | |
| 3 | KC-03 | Get nonexistent returns error | pending | |
| 4 | KC-04 | Delete removes key | pending | |
| 5 | KC-05 | Service name validation | pending | |
| 6 | KC-06 | Delete nonexistent returns error | pending | |
| 7 | KC-07 | Empty service name error | pending | |
| 8 | KC-08 | Windows stub returns ErrNotImplemented | pending | |
| 9 | SES-01 | NewSession creates directory with session.json | pending | |
| 10 | SES-02 | SaveSession atomic write | pending | |
| 11 | SES-03 | LoadSession reconstructs state | pending | |
| 12 | SES-04 | ListSessions sorted order | pending | |
| 13 | SES-05 | DeleteSession removes directory | pending | |
| 14 | SES-06 | ArchiveSession moves to archived/ | pending | |
| 15 | SES-07 | Corrupt session.json detection | pending | |
| 16 | SES-08 | Missing messages.json graceful degradation | pending | |
| 17 | SES-09 | Unique session IDs | pending | |
| 18 | SES-10 | ArchiveSession load roundtrip | pending | |
| 19 | PL-01 | Project roundtrip | pending | |
| 20 | PL-02 | Tasks roundtrip with dep parsing | pending | |
| 21 | PL-03 | State roundtrip with phase enum | pending | |
| 22 | PL-04 | Missing files return zero values | pending | |
| 23 | PL-05 | Empty deps parsed as empty []int | pending | |
| 24 | PL-06 | Multiple files parsed correctly | pending | |
| 25 | CK-01 | Checkpoint roundtrip | pending | |
| 26 | CK-02 | Max 2 retention | pending | |
| 27 | CK-03 | LatestCheckpoint returns most recent | pending | |
| 28 | CK-04 | Empty checkpoint returns error | pending | |
| 29 | CFG-01 | Load valid TOML | pending | |
| 30 | CFG-02 | Missing file returns defaults | pending | |
| 31 | CFG-03 | Save/load roundtrip | pending | |
| 32 | CFG-04 | M31A_CONFIG override | pending | |
| 33 | CFG-05 | M31A_THEME override | pending | |
| 34 | CFG-06 | M31A_DEFAULT_MODEL override | pending | |
| 35 | CFG-07 | ResolveAPIKeys env var priority | pending | |
| 36 | CFG-08 | ResolveAPIKeys config file fallback | pending | |
| 37 | CFG-09 | No keys returns empty strings | pending | |
| 38 | CFG-10 | Save creates parent dir | pending | |
| 39 | TK-01 | Known model uses tiktoken | pending | |
| 40 | TK-02 | Unknown model uses rune fallback | pending | |
| 41 | TK-03 | EMA calibration converges | pending | |
| 42 | TK-04 | EMA factor clamped | pending | |
| 43 | TK-05 | FormatUsage formatting | pending | |
| 44 | TK-06 | Warning below threshold empty | pending | |
| 45 | TK-07 | Warning above threshold styled | pending | |
| 46 | ST-01 | Settings renders 4 tabs | pending | |
| 47 | ST-02 | Tab/Shift+Tab cycling | pending | |
| 48 | ST-03 | API keys masked | pending | |
| 49 | ST-04 | Esc returns to REPL | pending | |
| 50 | ST-05 | Save writes config | pending | |
| 51 | RS-01 | Resume list with footer hints | pending | |
| 52 | RS-02 | Corrupted badge [!] | pending | |
| 53 | RS-03 | Enter emits SessionID | pending | |
| 54 | RS-04 | N emits new session | pending | |
| 55 | RS-05 | D shows delete confirmation | pending | |
| 56 | RS-06 | Y/N delete confirmation | pending | |
| 57 | RS-07 | Esc returns to REPL | pending | |
| 58 | RS-08 | Window resize | pending | |

## Gaps

(To be filled during testing)
