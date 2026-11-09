# `m31a-watcher`

> High-performance asynchronous filesystem watcher, debounced event batching, and `.gitignore` filtering powering M31A's continuous vibe coding engine.

---

## Why Does This Crate Exist?

Continuous vibe coding (`m31a vibe`) requires knowing the exact instant a developer saves a file so the background verification and auto-healing loop can run.

However, naive file watching has notorious failure modes:
- Saving a file in an IDE often triggers 4-6 distinct filesystem events in rapid succession (temp file creation, write, rename, inode update).
- Watching a workspace with `target/` or `node_modules/` burns 100% CPU scanning thousands of generated build artifacts.
- Without debouncing, the compiler and auto-healer trigger repeatedly mid-write.

`m31a-watcher` provides an intelligent, non-intrusive filesystem observer:
- Uses OS-native notifications (`inotify` on Linux, `kqueue` on macOS/BSD, `ReadDirectoryChangesW` on Windows).
- Automatically filters ignored paths (`.git/`, `target/`, `node_modules/`, `.m31a/`, lockfiles).
- Aggregates and debounces rapid file modifications into a single cohesive `WorkspaceChangeBatch`.

---

## Core Concepts & How It Works

### 1. Intelligent Filtering (`filter.rs`)
Ignores high-churn build directories, editor swap files (`*.swp`, `*~`), and version control metadata. Only genuine source code edits emit change notifications.

### 2. Configurable Debouncing (`debounce_duration`)
Waits for file writes to settle (default: `150ms`) before emitting a `WorkspaceChangeBatch`. Multiple rapid saves coalesce into a single event batch containing modified, created, and deleted paths.

### 3. Broadcast Event Channels (`tokio::sync::broadcast`)
Multiple subsystems (the TUI diff viewer, the compiler runner, and the visual TDD browser) can subscribe concurrently to the change feed.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/watcher.rs`](src/watcher.rs) | `WorkspaceWatcher`, `WatcherConfig` | Primary watcher coordinator managing OS notify handles. |
| [`src/events.rs`](src/events.rs) | `WorkspaceChangeBatch`, `FileChangeEvent`, `FileChangeKind` | Structured representation of coalesced filesystem changes. |
| [`src/filter.rs`](src/filter.rs) | `WatcherFilter` | Filters out ignored directories, binary caches, and temporary files. |

---

## Practical Usage Example

```rust
use m31a_watcher::watcher::{WatcherConfig, WorkspaceWatcher};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = WatcherConfig {
        debounce_duration: Duration::from_millis(200),
        ..Default::default()
    };

    let mut watcher = WorkspaceWatcher::new(PathBuf::from("."), Some(config));
    let mut rx = watcher.subscribe();

    // Start background watcher task
    watcher.start().await?;
    println!("Watching workspace for changes...");

    // Receive debounced change batches
    while let Ok(batch) = rx.recv().await {
        println!("Detected changes in {} files:", batch.changes.len());
        for change in batch.changes {
            println!("- {:?}: {:?}", change.kind, change.path);
        }
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-watcher
```
