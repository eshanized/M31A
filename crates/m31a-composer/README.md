# `m31a-composer`

Advanced multiline terminal textarea, paste burst protection, `@` file mentions, and slash command autocompletion for **M31A**.

---

## Overview

`m31a-composer` powers the user prompt input experience in the M31A Terminal User Interface. It handles multiline editing, cursor navigation, terminal paste-burst buffering (compressing 1,000+ line pastes into interactive preview pills), `@` symbol/file fuzzy mentions, and slash command parsing with real-time popup autocompletion.

---

## User-Facing Capabilities

- **Multiline Textarea**: Smooth text entry with `Shift+Enter` for newlines and `Enter` to submit.
- **Paste Burst Protection**: When pasting huge blocks of code or logs, prevents terminal UI lag by rendering a compact `[Pasted Code: 250 lines]` pill widget.
- **Slash Command Autocompletion (`/`)**: Real-time fuzzy popup suggesting commands like `/help`, `/model`, `/vibe`, `/diff`, and `/safety`.
- **`@` Mentions**: Type `@` to search files and inject targeted workspace context directly into your prompt.

---

## Developer Guide & Architecture

### Key Modules

- [`textarea`](src/textarea.rs): Core `TextArea` widget supporting undo/redo, cursor positioning, and line wrapping.
- [`slash`](src/slash.rs): `BUILTIN_SLASH_COMMANDS` registry, `tokenize_slash_command()`, `parse_and_resolve()`, and `SlashPopup` UI widget.
- [`paste_burst`](src/paste_burst.rs): `PasteBurstDetector` identifying rapid terminal bracketed pastes and wrapping them into `PastedSnippetPill`.
- [`mentions`](src/mentions.rs): `MentionsPopup` querying workspace paths and symbols.

---

## Testing

```bash
cargo test -p m31a-composer
```
