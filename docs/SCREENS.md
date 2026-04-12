# TUI Screen Reference

M31A uses a Bubble Tea TUI with multiple screens. The active screen depends on the current `AppState`.

---

## Init Screen
Shown during startup while M31A:
- Loads configuration (`~/.m31a/config.toml`)
- Resolves API keys (env → keychain → config)
- Checks provider health (OpenRouter/Zen)
- Checks for updates (version comparison)
- Appears as a logo animation and loading indicator

---

## Ready Screen
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

## Processing Screen
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

## Confirm Quit Screen
Shown when quitting during active processing:
```
┌──────────────────────────────────┐
│  Confirm Quit?                    │
│  An operation is in progress.     │
│                                  │
│  [y] Yes, quit    [n] No, stay  │
└──────────────────────────────────┘
```

---

## Session Picker
Activated via `/session` command:
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

## Ghost Picker
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

## Ghost Output
Shows results of ghost write operation:
- Files created/appended
- Warnings (if any)
- Content preview

---

## Bisect Output
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
