# `m31a-watcher`

Asynchronous workspace filesystem watcher and debounced change notification engine for **M31A**.

---

## Overview

`m31a-watcher` powers the continuous **Vibe Coding Loop** (`m31a vibe`). It observes filesystem modifications across the workspace, debounces rapid successive edits, and filters out noise from build outputs, temporary files, and version control directories (`target/`, `.git/`, `node_modules/`).

---

## User-Facing Capabilities

- **Continuous Vibe Coding (`m31a vibe`)**: Automatically triggers verification and self-healing whenever you edit and save files in your favorite editor.
- **Noise Filtering**: Ignores irrelevant compiler artifacts and lockfile touches, preventing wasteful build triggers.

---

## Developer Guide & Architecture

### Key Modules

- [`watcher`](src/watcher.rs): `WorkspaceWatcher` wrapping `notify` with asynchronous Tokio channels.
- [`filter`](src/filter.rs): `WatcherFilter` applying gitignore rules and ignoring build directories (`target/`, `.git/`, `.m31a/`, `node_modules/`).
- [`events`](src/events.rs): `FileChangeEvent`, `FileChangeKind` (Create, Modify, Delete), and `WorkspaceChangeBatch`.

---

## Testing

```bash
cargo test -p m31a-watcher
```
