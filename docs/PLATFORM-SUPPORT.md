# M31A Platform Support & Native Runtime Architecture

Authoritative statement of platform support, semantic contracts, and native runtime mechanisms.

## Targets & Classification

| Target | Architecture | Native Mechanism | Status | Evidence |
|--------|--------------|------------------|--------|----------|
| Linux x86_64 | GNU / musl | Bubblewrap + userns + cgroup v2 / POSIX rlimit | **SUPPORTED** | Release-qualified, local and CI evidence green |
| Linux aarch64 | GNU / musl | Bubblewrap + userns + cgroup v2 / POSIX rlimit | **COMPILE-CHECKED** | Cross-compiled in CI matrix |
| macOS x86_64 / arm64 | Apple Darwin | Seatbelt (`sandbox-exec`) + libproc + POSIX rlimit | **INTEGRATED** | Native Seatbelt provider, libproc start-time identity |
| Windows x86_64 | MSVC | Win32 Job Objects + DACLs + ConPTY + GetProcessTimes | **INTEGRATED** | Native Job Object containment, Windows sandbox provider, CI workflow |
| Windows arm64 | MSVC | Win32 Job Objects + DACLs + ConPTY + GetProcessTimes | **COMPILE-CHECKED** | Cross-compiled in CI matrix |

---

## Core Runtime Principle: Platform Neutrality

> "The model proposes. The runtime decides."

M31A avoids Linux-only assumptions across all core runtime layers. Native operating system mechanisms are selected based on the executing platform, while maintaining identical semantic contracts.

```text
                    M31A Runtime
                         │
                Platform Abstraction
                         │
        ┌────────────────┼────────────────┐
        │                │                │
      Linux            macOS           Windows
        │                │                │
  Bubblewrap /     Seatbelt /       Job Objects /
  cgroups v2 /     libproc /        Win32 DACLs /
  /proc identity   POSIX rlimit     GetProcessTimes
```

Shell portability (e.g., running `pwsh` on Linux) is strictly distinguished from operating system portability: PowerShell is an automation shell, not an indicator of the Windows OS.

---

## Subsystem Architecture & Native Mechanisms

### 1. Process Supervision & Tree Control
- **Windows**: All child processes and background jobs spawn directly assigned to a native Win32 **Job Object** handle (`Arc<JobHandle>`).
  - Limits enforced via `SetInformationJobObject` with `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`.
  - Tree termination is guaranteed by `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and fallback `TerminateJobObject(1)`.
  - Bare PID termination is never treated as equivalent to process tree control.
- **Linux**: Linux process groups (`setpgid`, `kill(-pgid)`) and optional cgroup v2 tree tracking.
- **macOS**: Process groups (`setpgid`) with `libproc` process validation.

### 2. Process Identity & Crash Recovery
- Replaced Linux-specific `/proc/<pid>/stat` `linux_starttime` with platform-neutral `ProcessIdentity`.
- **Windows**: Queries `GetProcessTimes` to obtain a stable 64-bit creation `FILETIME`.
- **Linux**: Reads process start time tick count from `/proc/<pid>/stat`.
- **macOS**: Uses `libproc` (`PROC_PIDTBSDINFO`).
- **Backward Compatibility**: Deserializes legacy `{"linux_starttime": ...}` metadata cleanly via `#[serde(alias = "linux_starttime")]`.

### 3. Sandbox Provider Routing
- Dynamic host routing via `LocalSandboxProvider`:
  - **Linux**: `BubblewrapSandboxProvider` (when `bwrap` and user namespaces are present).
  - **macOS**: `SeatbeltSandboxProvider` (when `sandbox-exec` is present).
  - **Windows**: `WindowsSandboxProvider` (enforces Job Object containment boundary, path containment, and private tempdirs).
  - **Fallback**: `ProcessIsolationProvider` (strictly fails closed if filesystem or network isolation is required).

### 4. Environment Construction & Sanitization
- Deny-by-default environment builder (`EnvironmentBuilder`):
  - **Windows Baseline**: Preserves `SystemRoot`, `ComSpec`, `PATHEXT`, `USERPROFILE`, `APPDATA`, `LOCALAPPDATA`, `TEMP`, `TMP`, `CARGO_HOME`, `RUSTUP_HOME`. Case-insensitive lookups.
  - **Windows Blocklist**: Strips dangerous injection variables (`__COMPAT_LAYER`, `COR_ENABLE_PROFILING`, `COR_PROFILER`, `COR_PROFILER_PATH`, `APP_POOL_ID`).
  - **Unix Baseline**: `PATH`, `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_ALL`, `TERM`, `TMPDIR`.

### 5. Native Shell Selection
- Shell selection is strictly decoupled from OS platform:
  - **Windows**: `cmd.exe /D /S /C`, `powershell.exe -NoProfile -NonInteractive -Command`, `pwsh.exe -NoProfile -NonInteractive -Command`.
  - **Unix**: POSIX `sh -c`.

### 6. Platform Paths & Security Defaults
- Configuration and policy paths resolve through `PlatformPaths`:
  - **Linux**: `/etc/m31a` (system), `~/.config/m31a` (user).
  - **macOS**: `/Library/Application Support/m31a` (system), `~/Library/Application Support/m31a` (user).
  - **Windows**: `%ProgramData%\m31a` (system), `%APPDATA%\m31a\config` (user).
- Default security rules enforce Windows vetoes:
  - System policy tampering: `**/ProgramData/m31a/**`, `**/System32/config/SAM*`, `**/System32/config/SYSTEM*`.
  - Shell profiles: `**/profile.ps1`, `**/Microsoft.PowerShell_profile.ps1`.
  - Destructive commands: `rmdir /s /q`, `del /f /s /q`, `format`, `diskpart`.

### 7. Filesystem & Tool Discovery
- `HostFilesystem::find_executable`: Probes `PATHEXT` (`.com`, `.exe`, `.bat`, `.cmd`) on Windows.
- `HostFilesystem::available_space_bytes`: Calls native `GetDiskFreeSpaceExW` on Windows, `statvfs` on Unix.
- `HostFilesystem::paths_identical`: Employs case-insensitive path comparison on Windows, byte-exact on Unix.

---

## Verification Rules

1. Pure reasoning contract tests (path containment, quoting, limit mapping, identity serialization) execute and pass on all development hosts.
2. Live Win32 API operations are verified in native Windows CI (`.github/workflows/windows.yml`) using `windows-latest` runners running PowerShell 7+.
3. A capability is reported as `Available` only when genuinely enforceable; degraded or unavailable features strictly report `Degraded` or `Unsupported`.
