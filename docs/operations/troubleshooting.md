<p align="center">
  <img src="../../assets/M31A.svg" alt="M31 Autonomous Logo" width="80" height="80">
</p>

# Operational Troubleshooting Guide

This guide provides diagnostic procedures and recovery instructions for common operational, runtime, model, persistence, and interface issues in **M31 Autonomous (M31A)**.

---

## 1. Quick Diagnostics with `m31a doctor`

Before manual investigation, run the built-in diagnostic suite:

```bash
# Standard diagnostic check
m31a doctor

# Machine-readable JSON output for automated diagnostics
m31a --output json doctor
```

`m31a doctor` verifies:
- Rust toolchain and binary identity
- SQLite database connectivity and migration state
- NVIDIA NIM API credential availability and reachability
- Workspace directory permissions and containment
- Git repository status and branch isolation

---

## 2. Model Provider & Credential Issues

### Symptom: `ModelError: no valid credentials resolved for nvidia_nim`

**Cause:**
M31A production uses NVIDIA NIM exclusively. Ambient environment variables (like `OPENAI_API_KEY`) and arbitrary `.env` files in production are rejected.

**Resolution:**
1. Run the interactive onboarding wizard to store the API key in the canonical secure store:
   ```bash
   m31a init
   ```
2. Verify that the credentials file exists and has secure file permissions (`0600` on Unix):
   - Production: `~/.config/m31a/credentials.json` (or OS config directory)
   - Development channel: `~/.config/m31a-dev/credentials.json`
3. Inspect current configuration resolution without exposing secrets:
   ```bash
   m31a config validate
   ```

### Symptom: `ModelError: provider endpoint unreachable or rate limited`

**Cause:**
Network connectivity failure, DNS resolution failure, or upstream NIM rate limits (HTTP 429).

**Resolution:**
- Check network destination security policies: external requests must conform to destination policy rules (SSRF protection blocks link-local, loopback, and private ranges).
- Verify custom endpoint configuration if using self-hosted NIM:
  ```toml
  [provider.nvidia_nim]
  base_url = "https://integrate.api.nvidia.com/v1"
  ```

---

## 3. Authorization & Policy Denials

### Symptom: `PolicyDenial: action denied by policy gate`

**Cause:**
The proposed tool action violates the active security policy or attempts to modify a protected path.

**Resolution:**
- Inspect policy configuration in `.m31a/config.md`:
  ```yaml
  policy:
    default_action: "ask" # or "allow" / "deny"
    denied_tools: []
  ```
- If running in safe autonomy mode, privileged side effects (such as shell execution or file mutations outside the workspace) require explicit interactive operator approval or must be run in autonomous mode when permitted.
- Verify that the target file does not match a protected path pattern (e.g., `.git/`, `.env`, credentials files).

### Symptom: `PolicyDenial: maximum delegation depth of N exceeded`

**Cause:**
An agent attempted recursive mid-task delegation beyond the configured ceiling.

**Resolution:**
- Delegation recursion is strictly bounded (default depth ceiling = 2 or configured profile limit).
- Avoid recursive self-delegation loops; structure tasks as discrete sibling steps in the execution graph rather than deeply nested child chains.

---

## 4. Task Graph, Persistence & SQLite Issues

### Symptom: `DatabaseError: database is locked`

**Cause:**
SQLite concurrent access contention when multiple processes access the same mission database.

**Resolution:**
- Ensure only one active M31A instance is operating on a specific workspace `.m31a/state.db`.
- Check SQLite WAL mode journal files (`state.db-wal` and `state.db-shm`).
- Verify that `TMPDIR` points to a filesystem that supports POSIX advisory file locking (avoid network-mounted NFS/SMB shares for SQLite state files).

### Symptom: `TaskNotFound: task not found in active graph`

**Cause:**
An action attempted to reference or supersede a task that belongs to a different execution run or does not exist in the active graph revision.

**Resolution:**
- Use `m31a session list` to identify active sessions.
- Trigger adaptive replanning through the canonical runtime replan path rather than ad-hoc state modifications.

---

## 5. Terminal & Cockpit (TUI) Issues

### Symptom: Garbled characters or terminal rendering artifacts

**Cause:**
Terminal emulator lacks UTF-8 support or does not support modern ANSI escape sequences.

**Resolution:**
- Ensure terminal environment variables are properly configured:
  ```bash
  export LANG="en_US.UTF-8"
  export LC_ALL="en_US.UTF-8"
  ```
- Ensure terminal window is at least 80 columns by 24 rows.
- Use `m31a run` (CLI mode) if operating in a headless or constrained CI environment without a valid TTY.

---

## 6. Deployment Channel Isolation

M31A maintains strict isolation between **production** (`m31a`) and **development** (`m31a-dev`):

| Property | Production (`m31a`) | Development (`m31a-dev`) |
|---|---|---|
| Channel feature | Default | `--features development` |
| Config directory | `~/.config/m31a` | `~/.config/m31a-dev` |
| State directory | `.m31a/` | `.m31a-dev/` |
| Package conflicts | `m31a-dev` | `m31a` |

If state or configuration appears missing after switching packages, verify which channel binary is currently active:
```bash
m31a version
```
