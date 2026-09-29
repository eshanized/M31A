# M31A Platform Support

Authoritative per-target support statement. Evidence lives in `PHASE-50-SUPPORT-MATRIX.md`
(qualification) and `PHASE-49-SUPPORT-MATRIX.md` / `PHASE-49-PARITY-REPORT.md` (behavioral evidence).

## Targets

| Target | Classification | Meaning |
|--------|---------------|---------|
| Linux x86_64 | **SUPPORTED** | Release-qualified: native runtime, parity, security, CI, and artifact evidence all green |
| macOS x86_64 | **CONDITIONALLY SUPPORTED** | Implemented; Seatbelt isolation is Degraded (never full sandbox); native re-verification pending a macOS runner |
| Linux aarch64 | **COMPILE-ONLY** | Compiles; no native runtime evidence — do not assume x86_64 behavior |
| macOS aarch64 | **COMPILE-ONLY** | Compiles; no native runtime evidence |
| Windows x86_64 | **COMPILE-ONLY** | Compiles; Job Objects / ACLs / ConPTY verified by contract only — native runs pending a Windows runner |
| Windows aarch64 | **COMPILE-ONLY** | Compiles; no native runtime evidence |

## Capability Gaps (all targets)

- Windows has no file-descriptor limit, no filesystem isolation, and no network
  isolation. Requests requiring them fail closed with typed errors — never silently.
- macOS memory limits are best-effort (`RLIMIT_AS`); Linux uses hard cgroup enforcement.
- Windows path containment uses lexical comparison; reparse-point resolution needs
  privilege and is a documented limitation.

## Rules

- A target is SUPPORTED only with native runtime evidence. Compilation is not support.
- Degraded never means Available. Unsupported never means successful.
- This file must agree with `PHASE-50-SUPPORT-MATRIX.md`. If they disagree, the
  phase matrix plus its evidence wins and this file must be updated.
