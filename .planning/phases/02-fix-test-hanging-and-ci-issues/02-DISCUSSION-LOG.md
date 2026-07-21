# Phase 2: Fix Test Hanging and CI Issues - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-21
**Phase:** 2-Fix Test Hanging and CI Issues
**Areas discussed:** Test hanging investigation approach, CI pipeline issues investigation, Fix strategy for hanging tests, CI pipeline fixes approach

---

## Test hanging investigation approach

### How should we identify which tests are hanging?

| Option | Description | Selected |
|--------|-------------|----------|
| Verbose output + timeouts (Recommended) | Run tests with -v flag to see which test is hanging, add timeouts to individual tests, use -race flag | ✓ |
| Add logging infrastructure | Add structured logging to test setup/teardown, trace goroutine leaks, monitor resource usage | |
| Leverage Go test flags | Use Go's test framework features: -timeout flag, -count flag, -run flag to isolate hanging tests | |
| You decide | Let the agent decide the best approach based on initial investigation | |

**User's choice:** Verbose output + timeouts (Recommended)
**Notes:** None

### What timeout value should we use for tests?

| Option | Description | Selected |
|--------|-------------|----------|
| Short timeouts (30s) (Recommended) | Add -timeout 30s to test commands, fail tests that take too long | ✓ |
| Medium timeouts (60s) | Add -timeout 60s to test commands, allow longer-running tests | |
| Long timeouts (120s) | Add -timeout 120s to test commands, be lenient with test duration | |
| You decide | Let the agent decide based on test suite analysis | |

**User's choice:** Short timeouts (30s) (Recommended)
**Notes:** None

### How should we use the race detector during investigation?

| Option | Description | Selected |
|--------|-------------|----------|
| Always use race detector (Recommended) | Add -race flag to all test commands, catch race conditions that cause hangs | ✓ |
| Selective race detection | Use -race only for specific test packages that are suspected to have race conditions | |
| Double-run strategy | Run tests twice: once without -race, once with -race to catch intermittent issues | |
| You decide | Let the agent decide based on test suite analysis | |

**User's choice:** Always use race detector (Recommended)
**Notes:** None

### Which tests should we prioritize investigating?

| Option | Description | Selected |
|--------|-------------|----------|
| Focus on concurrent tests (Recommended) | Focus on tests that use goroutines, channels, sync primitives, or concurrent operations | ✓ |
| Systematic investigation | Investigate all tests systematically, starting with the most complex ones | |
| Start with recent failures | Start with tests that have failed in CI recently, then expand | |
| You decide | Let the agent decide based on initial analysis | |

**User's choice:** Focus on concurrent tests (Recommended)
**Notes:** None

---

## CI pipeline issues investigation

### What specific CI issues are we seeing?

| Option | Description | Selected |
|--------|-------------|----------|
| Environment differences (Recommended) | Tests pass locally but fail in CI due to environment differences | ✓ |
| Flaky tests | Tests fail intermittently in CI, sometimes pass, sometimes fail | |
| Timeout issues | Tests timeout in CI due to resource constraints or slow execution | |
| You decide | Let the agent investigate and categorize all CI issues | |

**User's choice:** Environment differences (Recommended)
**Notes:** None

### How should we investigate environment differences?

| Option | Description | Selected |
|--------|-------------|----------|
| Environment audit (Recommended) | Check Go version, dependencies, environment variables, file permissions | ✓ |
| Docker-based testing | Run tests in Docker container to match CI environment | |
| CI-aware tests | Add CI environment detection to tests, skip tests that require local resources | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Environment audit (Recommended)
**Notes:** None

### Which tests should we prioritize for environment difference investigation?

| Option | Description | Selected |
|--------|-------------|----------|
| Focus on resource-dependent tests (Recommended) | Focus on tests that use file system, network, or external services | ✓ |
| All failing tests | Investigate all tests that fail in CI, regardless of type | |
| Start with most dependent | Start with tests that are most likely to have environment dependencies | |
| You decide | Let the agent decide based on initial analysis | |

**User's choice:** Focus on resource-dependent tests (Recommended)
**Notes:** None

### How should we handle tests that depend on external resources?

| Option | Description | Selected |
|--------|-------------|----------|
| CI detection + skip (Recommended) | Use environment variables to detect CI, skip tests that require local resources | ✓ |
| Test fixtures + mocks | Create test fixtures and mocks to replace external dependencies | |
| Test containers | Use test containers to provide consistent environment | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** CI detection + skip (Recommended)
**Notes:** None

---

## Fix strategy for hanging tests

### Should we fix individual tests or improve test infrastructure?

| Option | Description | Selected |
|--------|-------------|----------|
| Fix individual tests (Recommended) | Add proper cleanup, timeouts, and synchronization to individual hanging tests | ✓ |
| Improve test infrastructure | Add test utilities, helpers, and infrastructure to prevent hanging tests | |
| Both approaches | Fix individual tests and add infrastructure improvements | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Fix individual tests (Recommended)
**Notes:** None

### What should we add to hanging tests?

| Option | Description | Selected |
|--------|-------------|----------|
| Add timeouts (Recommended) | Add context.WithTimeout to tests that use goroutines or channels | ✓ |
| Add cleanup | Add proper cleanup with defer statements to ensure resources are released | |
| Add synchronization | Add synchronization with sync.WaitGroup or channels to coordinate goroutines | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Add timeouts (Recommended)
**Notes:** None

### What timeout value should we add to individual tests?

| Option | Description | Selected |
|--------|-------------|----------|
| 30 second timeout (Recommended) | Add 30 second timeout to match test suite timeout | ✓ |
| 10 second timeout | Add 10 second timeout for faster failure detection | |
| 60 second timeout | Add 60 second timeout for complex tests | |
| You decide | Let the agent decide based on test complexity | |

**User's choice:** 30 second timeout (Recommended)
**Notes:** None

### Should we add cleanup to prevent resource leaks?

| Option | Description | Selected |
|--------|-------------|----------|
| Add cleanup (Recommended) | Add cleanup with defer statements to ensure goroutines are stopped and resources released | ✓ |
| Add logging | Add logging to track goroutine lifecycle and resource usage | |
| Both cleanup and logging | Add both cleanup and logging for comprehensive debugging | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Add cleanup (Recommended)
**Notes:** None

---

## CI pipeline fixes approach

### Should we update the Makefile, CI workflow, or both?

| Option | Description | Selected |
|--------|-------------|----------|
| Update Makefile (Recommended) | Add -timeout flag to Makefile test targets, add -race flag consistently | |
| Update CI workflow | Update GitHub Actions workflow to add timeouts, environment detection, and better logging | |
| Both Makefile and CI | Update both Makefile and CI workflow for comprehensive fixes | ✓ |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Both Makefile and CI
**Notes:** None

### What should we add to the Makefile test targets?

| Option | Description | Selected |
|--------|-------------|----------|
| Add timeouts to Makefile (Recommended) | Add -timeout 30s to all test commands in Makefile | ✓ |
| Add race detection to Makefile | Add -race flag to all test commands in Makefile | |
| Add both flags | Add both -timeout and -race flags to all test commands | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Add timeouts to Makefile (Recommended)
**Notes:** None

### What should we add to the GitHub Actions workflow?

| Option | Description | Selected |
|--------|-------------|----------|
| Add job timeouts (Recommended) | Add timeout-minutes to GitHub Actions jobs to prevent infinite hangs | ✓ |
| Add CI detection | Add environment detection to skip tests that require local resources | |
| Improve logging | Add better logging and artifact collection for debugging failures | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Add job timeouts (Recommended)
**Notes:** None

### What timeout value should we add to GitHub Actions jobs?

| Option | Description | Selected |
|--------|-------------|----------|
| 10 minute timeout (Recommended) | Add timeout-minutes: 10 to test jobs in GitHub Actions | ✓ |
| 5 minute timeout | Add timeout-minutes: 5 to test jobs for faster failure detection | |
| 20 minute timeout | Add timeout-minutes: 20 to test jobs for complex test suites | |
| You decide | Let the agent decide based on test suite analysis | |

**User's choice:** 10 minute timeout (Recommended)
**Notes:** None

### Should we add CI detection to tests that depend on external resources?

| Option | Description | Selected |
|--------|-------------|----------|
| Add CI detection to tests (Recommended) | Add CI detection to tests that use file system or network resources | ✓ |
| Add skip logic | Add skip logic to tests that require local resources | |
| Both detection and skip | Add both CI detection and skip logic | |
| You decide | Let the agent decide based on initial investigation | |

**User's choice:** Add CI detection to tests (Recommended)
**Notes:** None

---

## the agent's Discretion

- Agent has flexibility in identifying which specific tests are hanging
- Agent can decide on specific cleanup strategies based on test implementation
- Agent can adjust timeout values if needed during implementation

## Deferred Ideas

None — discussion stayed within phase scope.
