# Technical Concerns

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## Known Bugs (BUG-XXXX References)

The codebase contains references to tracked bugs (BUG-01 through BUG-29). These indicate areas where specific issues were identified and fixed, but the fixes may be fragile or have edge cases.

### Key Bug References

- **BUG-01:** Channel ordering issues in git status/numstat concurrent execution
- **BUG-04:** Multiple framework indicators in project detection
- **BUG-05:** Multiple lock files causing incorrect package manager detection
- **BUG-06, BUG-07, BUG-19:** Data races from value-type replacement in sync.Map
- **BUG-10:** Working tree clean state after AddAll in ship phase
- **BUG-12:** Infinite oscillation between workflow phases
- **BUG-15:** Numstat-based heuristic vs file classification
- **BUG-17:** Race condition in provider cache
- **BUG-18:** Silent message handling in config loader
- **BUG-29:** Token estimation may exceed model window limits

**Location:** Bug references scattered across codebase, primarily in:
- `internal/engine/workflow/classify_test.go`
- `internal/tools/exec/concurrency.go`
- `internal/engine/tokens/estimator.go`
- `internal/integrations/provider/cache.go`

## Security Concerns

### Hardcoded Secrets Detection

- **Location:** `internal/engine/workflow/ship_preflight.go`
- **Pattern:** Regex-based detection of hardcoded passwords, secrets, API keys
- **Issue:** Heuristic-based, may have false positives/negatives
- **Mitigation:** Security gate in plan phase (`PlanSecurityGate` config)

### Bash Command Sandboxing

- **Location:** `internal/tools/exec/bash_sandbox_*.go`
- **Pattern:** Platform-specific sandboxing (Linux, macOS, Windows)
- **Issue:** Defense-in-depth, not a security boundary
- **Concern:** New attack vectors may not be covered

### Prompt Injection

- **Location:** `internal/tools/subagent/loop.go`
- **Pattern:** CRITICAL SECURITY INSTRUCTIONS in subagent prompts
- **Issue:** LLM-based security, not cryptographically secure
- **Mitigation:** Multiple layers of defense

### API Key Storage

- **Location:** `pkg/keychain/`
- **Pattern:** OS keychain integration (macOS Keychain, Windows Credential Vault, Linux Secret Service)
- **Issue:** Platform-specific implementations may have vulnerabilities
- **Mitigation:** Never written to disk in plaintext

## Performance Concerns

### Token Estimation

- **Location:** `internal/engine/tokens/estimator.go`
- **Issue:** Token estimation may be inaccurate, leading to context window overflow
- **Bug Reference:** BUG-29
- **Mitigation:** Conservative estimates, but may reject valid requests

### Provider Cache Race Conditions

- **Location:** `internal/integrations/provider/cache.go`
- **Issue:** Race condition between cache refresh and waiters
- **Bug Reference:** BUG-17
- **Mitigation:** Mutex-guarded cache with typed assertions

### Concurrent Tool Execution

- **Location:** `internal/tools/exec/concurrency.go`
- **Issue:** Data races from value-type replacement in sync.Map
- **Bug References:** BUG-06, BUG-07, BUG-19
- **Mitigation:** Typed mutex-guarded maps, comma-ok type assertions

### UI Performance

- **Location:** `internal/ui/tui/components/starfield.go`, `sparkline.go`
- **Issue:** Large data sets can cause rendering lag
- **Mitigation:** Caps on data points (100 points max)

## Technical Debt

### Hardcoded Values

- **Location:** `internal/core/config/types.go`
- **Issue:** Some values are hardcoded instead of configurable
- **Example:** Default timeout of 10 seconds
- **Mitigation:** Configuration options exist but not all are exposed

### Platform-Specific Code

- **Location:** `internal/tools/exec/bash_sandbox_*.go`
- **Issue:** Four separate implementations for different platforms
- **Mitigation:** Build tags for platform-specific compilation

### Test Coverage Gaps

- **Location:** Various test files
- **Issue:** Some edge cases may not be covered
- **Mitigation:** 75% overall coverage target, 90% for critical paths

### Error Handling

- **Location:** Throughout codebase
- **Issue:** Some errors may be swallowed or not properly propagated
- **Mitigation:** Custom error types, but not exhaustive

## Fragile Areas

### Workflow Phase Transitions

- **Location:** `internal/engine/workflow/engine.go`
- **Issue:** Complex state machine with mutex protection
- **Concern:** Phase transitions may have edge cases
- **Mitigation:** `transitionMu` mutex, but still complex

### Provider Fallback Logic

- **Location:** `internal/integrations/provider/fallback.go`
- **Issue:** Multi-provider failover may have cascading failures
- **Concern:** Error handling across providers
- **Mitigation:** Configurable retry policies

### Git Integration

- **Location:** `internal/integrations/git/git.go`
- **Issue:** Git operations may fail in edge cases
- **Concern:** Concurrent git operations, repository state
- **Mitigation:** Error handling, but git is complex

### Context Window Management

- **Location:** `internal/engine/compaction/compaction.go`
- **Issue:** Message compaction may lose important context
- **Concern:** Balancing context size vs information loss
- **Mitigation:** Heuristic-based compaction

## Configuration Issues

### Security Gate

- **Location:** `internal/engine/workflow/plan.go`
- **Issue:** Security gate may be too strict or too permissive
- **Concern:** Balancing security vs usability
- **Mitigation:** Configurable via `PlanSecurityGate`

### Provider Configuration

- **Location:** `internal/core/config/types.go`
- **Issue:** Multiple providers with different configurations
- **Concern:** Configuration complexity
- **Mitigation:** Default values, but still complex

## Testing Concerns

### Integration Test Environment

- **Location:** `tests/testutil/integration/`
- **Issue:** Integration tests may not cover all scenarios
- **Concern:** External dependencies (APIs, git)
- **Mitigation:** Mock providers, but not exhaustive

### E2E Test Reliability

- **Location:** `tests/e2e/e2e_test.go`
- **Issue:** E2E tests may be flaky
- **Concern:** Network dependencies, timing issues
- **Mitigation:** Timeouts, retry logic

## Dependency Concerns

### Go Version

- **Location:** `go.mod`
- **Issue:** Requires Go 1.25.12 (cutting edge)
- **Concern:** Compatibility with older Go versions
- **Mitigation:** None, forward-only compatibility

### External Dependencies

- **Location:** `go.mod`
- **Issue:** Multiple external dependencies
- **Concern:** Supply chain security, maintenance burden
- **Mitigation:** Minimal dependencies, but still present

## Operational Concerns

### Logging

- **Location:** `internal/integrations/log/`
- **Issue:** Structured logging may be verbose
- **Concern:** Log management in production
- **Mitigation:** Configurable log levels

### Metrics

- **Location:** `internal/integrations/metrics/`
- **Issue:** Metrics collection overhead
- **Concern:** Performance impact
- **Mitigation:** Lightweight metrics

### Rollback Chain

- **Location:** `internal/engine/rollback/`
- **Issue:** Git rollback chain may become corrupted
- **Concern:** Data loss, corruption
- **Mitigation:** Validation, but not exhaustive

## Recommendations

### Short-Term

1. **Review BUG-XXXX references** for known issues
2. **Add more test coverage** for edge cases
3. **Validate security gate** configuration
4. **Monitor performance** in production

### Long-Term

1. **Refactor platform-specific code** to reduce duplication
2. **Improve error handling** consistency
3. **Add more configuration options** for hardcoded values
4. **Enhance monitoring** and alerting