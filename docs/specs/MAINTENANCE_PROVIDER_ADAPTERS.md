# Maintenance Provider Adapters Specification

The intake subsystem consumes maintenance signals from external and local sources, converting them into canonical `MaintenanceSignal` instances.

## 1. GitHub Signal Adapter (`GitHubSignalAdapter`)

Located in `crates/m31a-maintenance/src/intake/github.rs`.

### Supported Event Types
1. **`workflow_run`**:
   - Ingests CI failure events where `conclusion == "failure"`.
   - Downloads run logs via GitHub Actions API.
   - Parses compiler diagnostics (`error[E...]`, panic backtraces, failed test names).
   - Produces a `MaintenanceCategory::BrokenCi` signal.
2. **`issues`**:
   - Ingests issue opened/edited events with labels `bug`, `fix`, `maintenance`.
   - Analyzes issue body for stack traces, reproduction steps, and error logs.
   - Produces a `MaintenanceCategory::IssueBug` signal.
3. **`pull_request` (Dependabot / Renovate)**:
   - Ingests dependency update PR events.
   - Extracts package name, old version, new version, and semver delta.
   - Produces a `MaintenanceCategory::DependencyUpdate` signal.
4. **`security_advisory` / Dependabot Alerts**:
   - Ingests CVE and GHSA security advisories.
   - Preserves CVE identifier and safe version while sanitizing raw exploit descriptions.
   - Produces a `MaintenanceCategory::SecurityAdvisory` signal.

### Security Invariants
- Validates `X-Hub-Signature-256` using constant-time HMAC-SHA256 comparison.
- Rejects bodies exceeding 5MB.
- Rejects replayed timestamps differing by > 10 minutes.
- Sanitizes untrusted user text using `GitHubSignalAdapter::sanitize_untrusted_text`.

---

## 2. Local Repository Signal Source (`LocalRepositorySignalSource`)

Located in `crates/m31a-maintenance/src/intake/local.rs`.

Executes non-destructive AST and toolchain audits on the local workspace without modifying code:
1. **Toolchain Compiler Verification**:
   - Runs `AutonomousVerifier::verify_check` with a 45s timeout.
   - Extracts structured compiler diagnostics and emits `BrokenCi` signals on failures.
2. **Manifest Dependency Audit**:
   - Employs `m31a_ast::semver::ManifestDependencyAuditor` across `Cargo.toml` and `package.json`.
   - Detects wildcard dependencies (`*`), missing patch pins, and deprecated lockfiles.
   - Emits `DependencyUpdate` signals.
3. **Documentation Drift Audit**:
   - Employs `m31a_ast::docs::DocSyncAuditor`.
   - Validates code fences, public symbol doc sync, and broken links.
   - Emits `DocDrift` signals.
4. **Architectural Complexity & Coupling Smells**:
   - Employs `m31a_ast::complexity::ArchitecturalComplexityAnalyzer`.
   - Audits excessive cyclomatic complexity, circular dependencies, and file bloat.
   - Emits `ArchitectureDrift` signals.
