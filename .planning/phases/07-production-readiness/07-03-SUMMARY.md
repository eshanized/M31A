---
phase: 07-production-readiness
plan: 03
completed: "2026-08-06T17:45:00Z"
status: complete
commits:
  - 94109af5 - docs(07-03): SUPPORT.md — LTS policy, versioning, deprecation, security reporting
requirements_met:
  - LTS-01
key_files_created:
  - SUPPORT.md
---

## Plan 07-03: LTS Policy, Versioning, Deprecation, Security Reporting — COMPLETE

### What Was Built

**SUPPORT.md** — Complete long-term support documentation at repository root with five sections:

**1. Versioning Policy**
- Semantic versioning (MAJOR.MINOR.PATCH) with clear definitions
- MAJOR: breaking changes to pkg/extensions interfaces, config schema, CLI flags
- MINOR: backward-compatible features, new extension points, additive config/flags
- PATCH: bug fixes, security fixes, documentation
- Conventional Commits table mapping prefixes to version bumps
- `BREAKING CHANGE:` / `!` suffix for MAJOR bumps

**2. Support Windows (per D-05)**
- Current release: Full support (bug fixes, security, features, docs)
- Previous minor (N-1): Security fixes only for 6 months from current release
- Major versions: Security fixes only for 12 months from major release
- Support level table with bug/security/features/docs matrix
- Example timeline diagram showing version progression

**3. EOL Policy**
- EOL dates published in GitHub Releases and SUPPORT.md
- 6-month deprecation notice before feature removal
- No patches after EOL (exception: CVSS ≥ 9.0 at maintainer discretion)
- EOL communication steps documented

**4. Deprecation Process**
- Runtime warnings via `slog.Warn` with structured fields (feature, since, removal, migration)
- CHANGELOG.md entries under `Deprecated` section with migration path
- Config field marking with deprecation comments in Go types
- CLI flag warnings when deprecated flags used
- Minimum 6 months between deprecation and removal
- Removal in next MAJOR version after deprecation period

**5. Security Reporting**
- Private disclosure via GitHub Security Advisories (not public issues)
- Response timeline table by severity:
  - Critical/High: 48h ack, 1 week assessment, 2 weeks fix
  - Medium: 48h ack, 2 weeks assessment, 1 month fix
  - Low: 1 week ack, 1 month assessment, best effort fix
- Coordinated disclosure process documented
- Reporter credit in release notes (with anonymity option)

### Verification

- `test -f SUPPORT.md` — PASS
- All five required sections present: Versioning Policy, Support Windows, EOL Policy, Deprecation Process, Security Reporting
- Key content verified: "6 months", "12 months", "slog.Warn", "GitHub Security Advisories"
- Document uses professional markdown with tables

### Requirements Satisfied

- **LTS-01**: Complete LTS policy documented with versioning, support windows (6-month and 12-month per D-05), EOL policy, deprecation process, and security reporting

### Reversibility

Documentation only — can be updated or removed without code changes.