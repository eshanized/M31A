# M31A Support Policy

**Last Updated:** 2026-08-06

---

## Versioning Policy

M31A follows [Semantic Versioning 2.0.0](https://semver.org/) (MAJOR.MINOR.PATCH).

| Version Component | When It Increments | Examples |
|-------------------|-------------------|----------|
| **MAJOR** (X.0.0) | Breaking changes that require user action | • Breaking changes to `pkg/extensions` interfaces (method removal, signature changes)<br>• Config schema changes (TOML key removal or rename)<br>• CLI flag removal or rename<br>• Removed extension points |
| **MINOR** (x.Y.0) | Backward-compatible new features | • New features with full backward compatibility<br>• New extension points (tools, providers, workflows)<br>• New config options (additive)<br>• New CLI flags (additive) |
| **PATCH** (x.y.Z) | Bug fixes and security fixes | • Bug fixes with no API changes<br>• Security fixes<br>• Documentation fixes<br>• Internal refactoring without behavior change |

### Conventional Commits and Version Bumps

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):

| Commit Prefix | Version Bump |
|---------------|--------------|
| `feat:` | MINOR (or MAJOR if breaking) |
| `fix:` | PATCH |
| `perf:` | PATCH |
| `refactor:` | PATCH (unless breaking) |
| `docs:` | PATCH (documentation only) |
| `test:` | PATCH |
| `chore:` | PATCH (maintenance) |
| `build:` | PATCH |
| `ci:` | PATCH |
| `feat!:` / `fix!:` / `BREAKING CHANGE:` | MAJOR |

Breaking changes **must** include `BREAKING CHANGE:` in the commit body or `!` after the type prefix.

---

## Support Windows

Per [Decision D-05](https://github.com/eshanized/M31A/blob/main/.planning/STATE.md), M31A provides rolling support windows:

| Version | Release Date | EOL Date | Support Level |
|---------|--------------|----------|---------------|
| Current (latest minor) | Release date | 6 months after next minor | **Full support** — bug fixes, security fixes, new features, documentation |
| Previous minor (N-1) | Release date of N-1 | 6 months after current release | **Security fixes only** — critical/high severity security patches |
| Major versions (N-1.0, N-2.0, etc.) | Release date | 12 months after major release | **Security fixes only** — critical/high severity security patches |

### Support Level Definitions

| Support Level | Bug Fixes | Security Fixes | New Features | Documentation |
|---------------|-----------|----------------|--------------|---------------|
| **Full Support** | ✅ | ✅ | ✅ | ✅ |
| **Security Only (6 months)** | ❌ | ✅ (critical/high) | ❌ | ✅ (security-related) |
| **Security Only (12 months)** | ❌ | ✅ (critical/high) | ❌ | ✅ (security-related) |
| **End of Life** | ❌ | ❌ | ❌ | ❌ |

### Example Timeline

```
v1.0.0  ──┬──► v1.1.0 ──┬──► v1.2.0 ──┬──► v2.0.0
          │             │             │
          │             │             ├─ Security only (12 months from v2.0.0)
          │             │             │
          │             │             ├─ Security only (6 months from v1.2.0)
          │             │             │
          │             ├─ Full support
          │             │
          ├─ Security only (6 months from v1.1.0)
```

---

## EOL Policy

- **EOL dates are published** in GitHub Releases (as release notes) and updated in this `SUPPORT.md` file.
- **6-month deprecation notice** before removing any public feature, config option, CLI flag, or extension point.
- **After EOL**: No patches (security or otherwise) will be provided. Users should upgrade to a supported version.
- **Exceptions**: Critical security vulnerabilities (CVSS ≥ 9.0) affecting widely deployed EOL versions may receive a one-time patch at the project maintainers' discretion.

### EOL Communication

1. EOL announced in release notes and `SUPPORT.md` at least 6 months in advance
2. Deprecation warnings added to affected features (see Deprecation Process)
3. Final release notes include migration guidance
4. GitHub Security Advisories remain available for historical reference

---

## Deprecation Process

When a feature, config option, CLI flag, or extension point is scheduled for removal:

### 1. Runtime Warnings
- Deprecated features emit warnings via `slog.Warn` when used
- Warning message includes: what is deprecated, since which version, planned removal version, migration guidance
- Example:
  ```go
  slog.Warn("deprecated",
      "feature", "OldConfigField",
      "since", "v1.2.0",
      "removal", "v2.0.0",
      "migration", "Use NewConfigField instead")
  ```

### 2. CHANGELOG Documentation
- Deprecation announced in `CHANGELOG.md` under a `Deprecated` section
- Entry includes: feature name, deprecation version, planned removal version (minimum 6 months), migration path
- Format:
  ```markdown
  ### Deprecated
  - **OldConfigField** (v1.2.0): Use `NewConfigField` instead. Removal planned for v2.0.0.
  ```

### 3. Config Field Marking
- Deprecated config fields marked in Go types with deprecation comment:
  ```go
  // Deprecated: Use NewConfigField instead. Removal planned for v2.0.0.
  OldConfigField string `toml:"old_field"`
  ```

### 4. CLI Flag Warnings
- Deprecated CLI flags produce warning when used:
  ```
  WARNING: --old-flag is deprecated since v1.2.0 and will be removed in v2.0.0.
  Use --new-flag instead.
  ```

### 5. Removal Timeline
- **Minimum 6 months** between deprecation and removal
- Removal occurs in the **next MAJOR version** after the deprecation period
- If deprecation occurs in v1.2.0, earliest removal is v2.0.0 (not v1.3.0)

---

## Security Reporting

### Private Disclosure

Report security vulnerabilities **privately** via [GitHub Security Advisories](https://github.com/eshanized/M31A/security/advisories/new) — **not** via public issues, discussions, or email.

### Response Timeline

| Severity | Acknowledgment | Assessment | Fix/Mitigation |
|----------|----------------|------------|----------------|
| **Critical** (CVSS ≥ 9.0) | ≤ 48 hours | ≤ 1 week | ≤ 2 weeks |
| **High** (CVSS 7.0–8.9) | ≤ 48 hours | ≤ 1 week | ≤ 2 weeks |
| **Medium** (CVSS 4.0–6.9) | ≤ 48 hours | ≤ 2 weeks | ≤ 1 month |
| **Low** (CVSS < 4.0) | ≤ 1 week | ≤ 1 month | Best effort |

### Process

1. **Submit** via GitHub Security Advisories with:
   - Description of the vulnerability
   - Steps to reproduce (if applicable)
   - Impact assessment
   - Suggested fix (if known)

2. **Acknowledgment** within 48 hours confirming receipt

3. **Assessment** within the timeline above:
   - Verify the vulnerability
   - Determine affected versions
   - Assign CVE if warranted

4. **Fix Development**:
   - Develop fix in private fork
   - Test against all supported versions
   - Prepare release notes (without vulnerability details until published)

5. **Coordinated Disclosure**:
   - Publish GitHub Security Advisory with CVE
   - Release patched versions for all supported versions
   - Update `SUPPORT.md` if EOL dates affected

### Reporter Credit

- Reporters credited in release notes and Security Advisory (unless they request anonymity)
- Credit format: "Reported by @username" or "Reported by [Name]"

---

## Related Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) — Contribution guidelines
- [CHANGELOG.md](CHANGELOG.md) — Release history and deprecations
- [AGENTS.md](AGENTS.md) — Project conventions and commit message format
- [GitHub Releases](https://github.com/eshanized/M31A/releases) — Version history and EOL announcements
- [GitHub Security Advisories](https://github.com/eshanized/M31A/security/advisories) — Security vulnerability reports