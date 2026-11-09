# `m31a-composer`

> Multiline prompt composer, soft-wrapping textarea, undo/redo history, paste burst normalization, and slash-command autocompletion for M31A.

---

## Why Does This Crate Exist?

Prompting an autonomous coding assistant often requires multiline instructions, pasted stack traces, and embedded code snippets. Single-line terminal inputs (like standard `readline`) are frustrating when you need to navigate up a few lines to fix a typo, paste 20 lines of JSON without terminal glitching, or complete a slash command.

`m31a-composer` provides a specialized terminal text editor built for AI interaction:
- **Multiline Textarea (`TextArea`)**: Smooth 2D cursor navigation (`Up`, `Down`, `Left`, `Right`, `Home`, `End`), word jumps (`Alt+F`, `Alt+B`), and line deletion (`Ctrl+K`).
- **Undo/Redo History**: Full snapshot undo stack (`Ctrl+Z`, `Ctrl+Y`).
- **Paste Burst Normalization (`paste_burst.rs`)**: Detects bracketed paste mode, normalizing CRLF newlines and preventing input lag when pasting large code blocks.
- **Slash Commands & Mentions Autocompletion**: Live popups for `/command` suggestions and `@` file path completions using fuzzy matching.

---

## Core Concepts & How It Works

### 1. Two-Dimensional Cursor Navigation (`textarea.rs`)
Tracks `cursor_row` and `cursor_col` across a `Vec<String>` buffer. Supports newline insertion with `Shift+Enter` and form submission with `Enter`.

### 2. Slash-Command Autocompletion Popup (`slash.rs`)
When the user types `/`, `SlashEngine` filters available commands (e.g. `/model`, `/diff`, `/vibe`, `/verify`, `/review`) using fuzzy algorithms, rendering a floating completion popup above the composer.

### 3. Bracketed Paste Handling (`paste_burst.rs`)
When pasting hundreds of lines from an IDE or browser, terminals send characters in rapid bursts. `PasteBurstNormalizer` captures bracketed paste delimiters (`\x1b[200~` ... `\x1b[201~`), sanitizes line breaks, and inserts the block as a single undoable transaction.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/textarea.rs`](src/textarea.rs) | `TextArea` | Core multiline text buffer with cursor, undo/redo, and kill buffer. |
| [`src/slash.rs`](src/slash.rs) | `SlashEngine`, `SlashCommandCandidate` | Fuzzy filtering and ranking for slash command autocompletions. |
| [`src/paste_burst.rs`](src/paste_burst.rs) | `PasteBurstNormalizer` | Normalizes rapid terminal paste bursts and escapes. |
| [`src/mentions.rs`](src/mentions.rs) | `MentionResolver`, `MentionCandidate` | Resolves `@filename` and `@symbol` references in prompt text. |

---

## Practical Usage Example

```rust
use m31a_composer::textarea::TextArea;

fn main() {
    let mut composer = TextArea::new();

    // 1. Insert text
    composer.insert_char('H');
    composer.insert_char('i');
    composer.insert_newline();
    composer.insert_str("Run cargo check and fix warnings");

    println!("Current text:\n{}", composer.text());
    println!("Cursor position: row={}, col={}", composer.cursor_row, composer.cursor_col);

    // 2. Test undo
    composer.undo();
    println!("After undo:\n{}", composer.text());
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-composer
```
