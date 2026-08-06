# Semantic Versioning Guide

**Version format:** `MAJOR.MINOR.PATCH` (e.g., `v1.2.3`)

Pre-releases: `v1.2.3-alpha`, `v1.2.3-beta`, `v1.2.3-rc.1`

---

## Version Bump Rules

### MAJOR (X.0.0) — Breaking Changes

Increment MAJOR when making **breaking changes to public API**:

| Category | Examples |
|----------|----------|
| **pkg/extensions interfaces** | Adding/removing methods from `ExternalTool`, `ExternalProvider`, `PhaseHookHandler`; changing method signatures; changing `ExtensionsConfig` structure |
| **CLI flags** | Adding/removing/renaming flags in `cmd/m31a/main.go`; changing flag behavior |
| **Config schema** | Adding required fields to `Config`; changing field types; removing fields; changing validation rules |
| **Protocol version** | Changing JSON-RPC `protocol_version` in handshake |
| **File formats** | Changing `m31a.json` structure; changing `config.toml` keys |

**When in doubt:** If it breaks external extensions, user configs, or CLI scripts → MAJOR.

### MINOR (0.X.0) — New Features (Non-Breaking)

Increment MINOR when adding **backward-compatible** functionality:

| Category | Examples |
|----------|----------|
| **New features** | New built-in tools, new provider integrations, new workflow phases |
| **New extension points** | New hook types, new config sections, new adapter interfaces |
| **Non-breaking API additions** | Adding optional methods with defaults, new config fields with defaults |
| **Performance improvements** | Faster algorithms, reduced memory, new caching |
| **Documentation** | New guides, API docs, examples |

**When in doubt:** If existing extensions/configs/scripts still work → MINOR.

### PATCH (0.0.X) — Bug Fixes & Maintenance

Increment PATCH for **backward-compatible bug fixes**:

| Category | Examples |
|----------|----------|
| **Bug fixes** | Crash fixes, logic errors, race conditions, memory leaks |
| **Docs** | Typos, clarifications, new examples |
| **CI/CD** | Workflow fixes, dependency updates, build fixes |
| **Internal refactors** | Code reorganization, renaming private symbols, optimization |

**When in doubt:** If no user-visible behavior change → PATCH.

---

## Pre-Release Versions

| Suffix | Meaning | When to Use |
|--------|---------|-------------|
| `-alpha` | Early development, unstable | Internal testing, preview features |
| `-beta` | Feature complete, testing | External testing, release candidates |
| `-rc.N` | Release candidate | Final validation before release |

**Examples:**
- `v2.0.0-alpha` — First MAJOR preview
- `v1.3.0-beta` — Feature complete, testing
- `v1.2.5-rc.1` — Release candidate, final validation

---

## Extension API Versioning

Extension protocol version is **independent** of M31A version:

| M31A Version | Protocol Version | Notes |
|--------------|------------------|-------|
| v1.x | 1.0 | Initial release |
| v2.x | 1.0 or 2.0 | If protocol changed |

**Rules:**
- Handshake returns `protocol_version` (e.g., `"1.0"`)
- Extensions declare `supported_methods` in handshake
- M31A rejects incompatible `protocol_version`
- Major protocol version = breaking changes (method signatures, new required methods)

---

## Deprecation Policy

| Stage | Timeline | Action |
|-------|----------|--------|
| **Announce** | 2 minor versions before removal | Add deprecation notice to docs, emit warning logs |
| **Deprecate** | 1 minor version before removal | Mark deprecated in code, continue working |
| **Remove** | After 2 minor versions | Remove code, update docs, MAJOR bump |

**Example:**
- v1.0: Feature X added
- v1.1: Feature X deprecated (warning in logs, docs updated)
- v1.2: Feature X still works, marked deprecated
- v2.0: Feature X removed (MAJOR)

---

## Examples

| Change | Version Bump | Reason |
|--------|--------------|--------|
| Add `CustomPalettes` to `TemplateConfig` | MINOR | New optional field |
| Remove `MaxIterations` from `UIConfig` | MAJOR | Breaking config change |
| Fix race in `Dispatcher` | PATCH | Bug fix |
| Add `MethodHookPostPhase` to protocol | MAJOR | Breaking protocol change |
| Update `golangci-lint` version | PATCH | Dependency update |
| New built-in `CodeSearch` tool | MINOR | New feature |

---

## Decision Flowchart

```
Is it a bug fix only?
  ├─ Yes → PATCH
  └─ No
      ├─ Does it break extensions/configs/CLI?
      │   ├─ Yes → MAJOR
      │   └─ No
      │       ├─ Is it a new feature/extension point?
      │       │   ├─ Yes → MINOR
      │       │   └─ No → PATCH (internal refactor/cleanup)
```

---

## Version in Code

```go
// main.go
var Version = "1.2.3"  // Set by GoReleaser at release
```

- Never hardcode version in code
- GoReleaser injects version at build time via `-ldflags`
- Development builds show `dev` or commit hash

---

## References

- [SemVer 2.0.0](https://semver.org/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [M31A Extension Protocol](docs/extensions/api-reference.md)