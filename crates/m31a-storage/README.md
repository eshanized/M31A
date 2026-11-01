# `m31a-storage`

Embedded SQLite persistence, thread history, atomic checkpoints, message boards, and transcript exporters for **M31A**.

---

## Overview

`m31a-storage` provides persistent storage for M31A using embedded SQLite (`m31a.db`). It stores conversation threads, turns, items, binary attachments, message board channels, and path locks. It supports branching/forking sessions, checkpoint rollback DAGs, and exporters for Markdown, JSONL, and HTML.

---

## User-Facing Capabilities

- **Session Resume & Forking (`m31a resume`, `m31a fork`)**: Seamlessly resume past conversations or fork an existing session into a new experimental branch.
- **Transcript Export (`m31a export`)**: Exports clean conversation transcripts into GitHub-flavored Markdown, JSONL, or HTML.
- **Session Checkpoints & Undo (`m31a undo`, `m31a revert`)**: Restores workspace files and conversation turns back to earlier checkpoints.

---

## Developer Guide & Architecture

### Key Modules

- [`db`](src/db.rs): Core SQLite connection pool, migrations (`migrations/`), and schema management.
- [`transcript_export`](src/transcript_export.rs): Formats conversations into Markdown, HTML, and JSONL transcripts.
- [`worktree`](src/worktree.rs): `WorktreeManager` managing ephemeral branch worktrees.
- [`attachment_store`](src/attachment_store.rs): Content-addressed storage for image assets and large command outputs.

---

## Testing

```bash
cargo test -p m31a-storage
```
