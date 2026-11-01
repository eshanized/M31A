# `m31a-shell`

Interactive Pseudoterminal (PTY) runner and stateful persistent shell execution engine for **M31A**.

---

## Overview

`m31a-shell` provides long-lived, stateful shell sessions and PTY virtualization. Unlike ephemeral subprocess calls, persistent sessions preserve current working directory changes (`cd`), environment variables, and shell aliases across multiple turns.

---

## User-Facing Capabilities

- **Stateful Shell Continuity**: When the agent navigates directories or sets environment variables in one turn, subsequent turns remember that context.
- **PTY Terminal Emulation**: Correctly handles terminal control codes, ANSI colors, and programs expecting interactive terminal devices.

---

## Developer Guide & Architecture

### Key Modules

- [`session`](src/session.rs): `PersistentShellSession` managing long-lived child shells with asynchronous stdin/stdout stream communication.
- [`pty`](src/pty.rs): `PtyRunner` allocating pseudoterminal master/slave file descriptors and configuring terminal window sizes (`PtySize`).
- [`buffer`](src/buffer.rs): `HeadTailBuffer` preserving head context and tail error messages when command output exceeds token limits.

---

## Testing

```bash
cargo test -p m31a-shell
```
