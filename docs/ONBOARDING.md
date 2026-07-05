# Getting Started with M31A

M31A is an autonomous coding assistant that runs in your terminal. It uses
a structured workflow to plan, execute, and verify code changes.

---

## Quick Start

1. **Launch M31A** by running `m31a` in your terminal
2. **Set up a provider** on first launch (OpenRouter, Zen, or Nvidia)
3. **Type your goal** in the REPL and press Enter
4. **Follow the workflow** as M31A plans and executes changes

---

## The Workflow Model

M31A uses a seven-phase workflow for complex tasks:

```
Initialize -> Discuss -> Plan -> Execute -> Verify -> Runtime -> Ship
```

| Phase | What Happens |
|-------|-------------|
| **Initialize** | Sets up the session, loads context |
| **Discuss** | Asks clarifying questions about your goal |
| **Plan** | Generates a task plan with dependencies |
| **Execute** | Runs tasks with tool calls |
| **Verify** | Tests and validates the changes |
| **Runtime** | Optional: runs dev server for smoke tests |
| **Ship** | Commits changes and creates a summary |

For simple tasks (fix a bug, add a test, update docs), M31A can use
**quick mode** which skips Discuss, Plan, and Verify phases.

---

## Quick Mode

Quick mode is for simple tasks that don't need planning. Enable it:

```
/quick on
```

Or M31A may auto-detect simple tasks and enable it automatically.
Quick mode uses the Direct path: Initialize -> Execute -> Ship.

To skip to a specific phase manually:

```
/skip execute
```

---

## Navigation

### Essential Keys

| Key | Action |
|-----|--------|
| `Esc` | Go back / close overlay |
| `Enter` | Confirm / submit |
| `?` | Toggle help screen |
| `Ctrl+P` | Open command palette |
| `Ctrl+B` | Toggle sidebar |
| `j` / `k` | Scroll up/down in viewports |

### Leader Key

M31A uses a leader key (`Ctrl+X`) for screen shortcuts. Press and release
`Ctrl+X`, then press a chord key within 1 second:

| Sequence | Screen |
|----------|--------|
| `Ctrl+X` `s` | Settings |
| `Ctrl+X` `h` | Help |
| `Ctrl+X` `d` | Dashboard |
| `Ctrl+X` `f` | File explorer |
| `Ctrl+X` `r` | Session list |
| `Ctrl+X` `n` | New session |

### Breadcrumbs

The header shows your navigation path as breadcrumbs. For example:
`REPL > Settings > Provider`

Press Esc to go back to the previous screen.

---

## Slash Commands

Type `/` in the REPL to see all available commands. Common ones:

| Command | Description |
|---------|-------------|
| `/new` | Start a new workflow |
| `/goal` | Set or change the session goal |
| `/plan` | Start the plan phase |
| `/execute` | Start the execute phase |
| `/verify` | Start the verify phase |
| `/ship` | Ship the changes |
| `/model` | Switch AI model |
| `/settings` | Open settings |
| `/help getting-started` | Show this guide |
| `/quick on` | Enable quick mode |
| `/clear` | Clear the conversation |

---

## The Sidebar

The sidebar shows project context, recent files, and workflow status. It is
visible by default on terminals with 80+ columns.

- Toggle with `Ctrl+B` or leader key `Ctrl+X b`
- Widen/narrow with `Ctrl+X [` / `Ctrl+X ]`

---

## First-Time Experience

When you launch M31A for the first time:

1. The **getting-started tour** explains the workflow model
2. The **home screen** shows categorized suggestions (Code, Explore, Debug)
3. The **REPL welcome** message explains what M31A can do

To replay the tour at any time:

```
/help getting-started
```

---

## Tips

- **Be specific** in your goal: "Fix the nil pointer in user.go line 42"
- **Use quick mode** for simple fixes: `/quick on` then type your fix
- **Check the dashboard** to see workflow progress: `Ctrl+X d`
- **Review before shipping**: M31A shows a diff before committing
- **Use the command palette** (`Ctrl+P`) to discover all features

---

## Keyboard Accessibility

All interactive screens support keyboard navigation:

- **j/k** or **up/down** to move between items
- **Enter** to select/activate
- **Esc** to go back
- **Tab** for autocomplete suggestions

Empty states (when a screen has no content) also support keyboard
navigation with j/k to move between suggested actions.

---

## Getting Help

- Press `?` for the help screen with all keybindings
- Type `/help getting-started` for this guide
- Type `/about` for version and system information
- Type `/health` to check system health
