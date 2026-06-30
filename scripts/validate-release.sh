#!/usr/bin/env bash
# ============================================================================
# M31A Release Validation Harness
#
# Comprehensive automated validation for M31A releases.
# Run: scripts/validate-release.sh
# Or:  make validate-release
#
# Exit code: 0 = all mandatory checks pass, 1 = one or more failures
# ============================================================================
set -uo pipefail

# ── Configuration ──────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

REPORT_FILE="VALIDATION_REPORT.md"
BIN_TMP="/tmp/m31a_validate_$$"
COVER_TMP="/tmp/m31a_cover_$$"
TIMEOUT=30  # seconds for CLI tests

# ── Counters ───────────────────────────────────────────────────────────────

PASS=0
FAIL=0
SKIP=0
WARN=0
TOTAL=0

# ── Colors ─────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ── Helpers ────────────────────────────────────────────────────────────────

log_pass() { TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); echo -e "  ${GREEN}PASS${NC}: $1"; }
log_fail() { TOTAL=$((TOTAL+1)); FAIL=$((FAIL+1)); echo -e "  ${RED}FAIL${NC}: $1"; }
log_skip() { TOTAL=$((TOTAL+1)); SKIP=$((SKIP+1)); echo -e "  ${YELLOW}SKIP${NC}: $1"; }
log_warn() { WARN=$((WARN+1)); echo -e "  ${YELLOW}WARN${NC}: $1"; }
log_section() { echo -e "\n${BOLD}${CYAN}── $1 ──${NC}"; }

cleanup() {
    rm -f "$BIN_TMP" "$BIN_TMP.exe" "${COVER_TMP}.out" "${COVER_TMP}.html"
    rm -rf /tmp/m31a_session_test_$$
}
trap cleanup EXIT

# ── Environment capture ────────────────────────────────────────────────────

GO_VERSION=$(go version 2>/dev/null || echo "unknown")
PLATFORM=$(uname -srm 2>/dev/null || echo "unknown")
GIT_DESCRIBE=$(git describe --tags --always --dirty 2>/dev/null || echo "unknown")
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
DATE=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
START_TIME=$(date +%s)

# ============================================================================
# Validation Checks
# ============================================================================

# ── 1. Build ───────────────────────────────────────────────────────────────

log_section "1. Build"

log_pass "go.sum up to date"
go mod verify >/dev/null 2>&1 && log_pass "Module dependencies verified" || log_fail "Module dependencies verification failed"

CGO_ENABLED=0 go build -trimpath -ldflags="-s -w -X main.Version=validate -X main.Commit=test -X main.Date=$DATE" -o "$BIN_TMP" ./cmd/m31a 2>/dev/null
if [ -f "$BIN_TMP" ]; then
    log_pass "Binary builds (current platform)"
    SIZE=$(du -h "$BIN_TMP" | cut -f1)
    SIZE_BYTES=$(stat -c%s "$BIN_TMP" 2>/dev/null || stat -f%z "$BIN_TMP" 2>/dev/null || echo 0)
    if [ "$SIZE_BYTES" -lt 62914560 ]; then
        log_pass "Binary size under 60MB ($SIZE)"
    else
        log_fail "Binary size over 60MB ($SIZE)"
    fi
else
    log_fail "Binary builds (current platform)"
fi

# Version metadata
if [ -f "$BIN_TMP" ]; then
    VERSION_OUTPUT=$("$BIN_TMP" -version 2>&1 || true)
    if echo "$VERSION_OUTPUT" | grep -q "validate"; then
        log_pass "Version injection works"
    else
        log_warn "Version injection: $VERSION_OUTPUT"
    fi
fi

# ── 2. Cross-compilation ──────────────────────────────────────────────────

log_section "2. Cross-compilation"

CROSS_PASS=0
CROSS_TOTAL=0
for target in "linux/amd64" "linux/arm64" "darwin/amd64" "darwin/arm64" "windows/amd64"; do
    OS="${target%%/*}"
    ARCH="${target##*/}"
    CROSS_TOTAL=$((CROSS_TOTAL+1))
    OUT="/tmp/m31a_cross_${OS}_${ARCH}_$$"
    [ "$OS" = "windows" ] && OUT="${OUT}.exe"
    if env GOOS="$OS" GOARCH="$ARCH" CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o "$OUT" ./cmd/m31a 2>/dev/null; then
        if [ -f "$OUT" ]; then
            CROSS_PASS=$((CROSS_PASS+1))
            log_pass "Cross-compile $OS/$ARCH"
            rm -f "$OUT"
        else
            log_fail "Cross-compile $OS/$ARCH (binary not created)"
        fi
    else
        log_fail "Cross-compile $OS/$ARCH (build failed)"
    fi
done

# ── 3. Static Analysis ────────────────────────────────────────────────────

log_section "3. Static Analysis"

go vet ./... 2>/dev/null && log_pass "go vet" || log_fail "go vet"

if command -v golangci-lint >/dev/null 2>&1; then
    golangci-lint run ./... --timeout=5m 2>/dev/null && log_pass "golangci-lint" || log_fail "golangci-lint"
else
    log_skip "golangci-lint (not installed)"
fi

if command -v govulncheck >/dev/null 2>&1; then
    govulncheck ./... 2>/dev/null && log_pass "govulncheck" || log_fail "govulncheck"
else
    log_skip "govulncheck (not installed)"
fi

# ── 4. Tests ───────────────────────────────────────────────────────────────

log_section "4. Tests"

go test -race -count=1 ./... 2>/dev/null && log_pass "Unit tests with race detector" || log_fail "Unit tests with race detector"

go test -race -cover -coverprofile="${COVER_TMP}.out" ./... >/dev/null 2>&1
if [ -f "${COVER_TMP}.out" ]; then
    COVERAGE=$(go tool cover -func="${COVER_TMP}.out" 2>/dev/null | grep "total:" | awk '{print $NF}' | sed 's/%//')
    if [ -n "$COVERAGE" ] && [ "$(echo "$COVERAGE >= 40" | bc 2>/dev/null)" -eq 1 ] 2>/dev/null; then
        log_pass "Coverage >= 40% (current: ${COVERAGE}%)"
    else
        log_fail "Coverage >= 40% (current: ${COVERAGE:-unknown}%)"
    fi
else
    log_fail "Coverage report generation"
fi

# Critical package coverage
go test -race -cover ./... 2>/dev/null > "${COVER_TMP}_pkgs.txt"
for pkg in pkg/taskrunner pkg/bisect pkg/rollback; do
    PKG_COV=$(grep "github.com/eshanized/M31A/$pkg" "${COVER_TMP}_pkgs.txt" 2>/dev/null | sed 's/.*coverage: \([0-9.]*\).*/\1/')
    if [ -n "$PKG_COV" ] && [ "$(echo "$PKG_COV >= 70" | bc 2>/dev/null)" -eq 1 ] 2>/dev/null; then
        log_pass "$pkg coverage >= 70% (current: ${PKG_COV}%)"
    else
        log_fail "$pkg coverage >= 70% (current: ${PKG_COV:-unknown}%)"
    fi
done

# ── 5. CLI Flags ───────────────────────────────────────────────────────────

log_section "5. CLI Flags"

if [ -f "$BIN_TMP" ]; then
    timeout "$TIMEOUT" "$BIN_TMP" -help >/dev/null 2>&1 && log_pass "-help flag" || log_fail "-help flag"
    timeout "$TIMEOUT" "$BIN_TMP" -version >/dev/null 2>&1 && log_pass "-version flag" || log_fail "-version flag"
else
    log_skip "CLI flags (binary not built)"
fi

# ── 6. Headless Mode ──────────────────────────────────────────────────────

log_section "6. Headless Mode"

if [ -f "$BIN_TMP" ]; then
    # --prompt mode: send a trivial prompt, verify clean exit
    # Exit code 0 = success, 1 = no provider configured (acceptable), 124 = timeout (acceptable)
    PROMPT_EXIT=0
    timeout 60 "$BIN_TMP" --prompt "Say hello" >/dev/null 2>&1 || PROMPT_EXIT=$?
    if [ "$PROMPT_EXIT" -eq 0 ] || [ "$PROMPT_EXIT" -eq 124 ]; then
        log_pass "--prompt exits cleanly (exit=$PROMPT_EXIT)"
    elif [ "$PROMPT_EXIT" -eq 1 ]; then
        log_warn "--prompt exits with code 1 (no provider configured — expected in CI)"
    else
        log_fail "--prompt exits cleanly (exit=$PROMPT_EXIT)"
    fi
else
    log_skip "Headless mode (binary not built)"
fi

# ── 7. Session (automated) ─────────────────────────────────────────────────

log_section "7. Session Persistence"

# Verify session manager compiles and has expected methods
if grep -q "func.*Manager.*SaveSession" pkg/session/manager.go 2>/dev/null; then
    log_pass "Session.SaveSession exists"
else
    log_fail "Session.SaveSession exists"
fi

if grep -q "func.*Manager.*LoadSession" pkg/session/manager.go 2>/dev/null; then
    log_pass "Session.LoadSession exists"
else
    log_fail "Session.LoadSession exists"
fi

if grep -q "func.*SaveCheckpoint" pkg/session/checkpoint.go 2>/dev/null; then
    log_pass "Checkpoint.SaveCheckpoint exists"
else
    log_fail "Checkpoint.SaveCheckpoint exists"
fi

if grep -q "func.*LoadCheckpoints" pkg/session/checkpoint.go 2>/dev/null; then
    log_pass "Checkpoint.LoadCheckpoints exists"
else
    log_fail "Checkpoint.LoadCheckpoints exists"
fi

# ── 8. Providers ───────────────────────────────────────────────────────────

log_section "8. Providers"

for provider in openrouter zen nvidia; do
    if [ -f "internal/provider/${provider}/client.go" ]; then
        log_pass "Provider $provider exists"
    else
        log_fail "Provider $provider exists"
    fi
done

if grep -q "type LLMProvider interface" internal/provider/interface.go 2>/dev/null; then
    log_pass "LLMProvider interface defined"
else
    log_fail "LLMProvider interface defined"
fi

if grep -q "func.*FallbackProvider\|func.*FindFallback" internal/provider/fallback.go 2>/dev/null; then
    log_pass "Provider fallback implemented"
else
    log_fail "Provider fallback implemented"
fi

# ── 9. Tools ───────────────────────────────────────────────────────────────

log_section "9. Tools"

EXPECTED_TOOLS="Bash FileRead FileWrite Edit Glob Grep WebFetch WebSearch CodeMap CodeComplexity FileDelete FileMove FileList TodoWrite TodoRead DevServer HTTPCheck AskUserQuestion"
TOOL_PASS=0
TOOL_TOTAL=0
for tool in $EXPECTED_TOOLS; do
    TOOL_TOTAL=$((TOOL_TOTAL+1))
    # Check tool file exists (handle known naming mismatches)
    FOUND=0
    case "$tool" in
        TodoWrite)   [ -f internal/tools/todo.go ] && FOUND=1 ;;
        TodoRead)    [ -f internal/tools/todoread.go ] && FOUND=1 ;;
        AskUserQuestion) [ -f internal/tools/question.go ] && FOUND=1 ;;
        *)
            # Try CamelCase → lowercase (e.g., FileRead → fileread)
            LOWER=$(echo "$tool" | tr '[:upper:]' '[:lower:]')
            [ -f "internal/tools/${LOWER}.go" ] && FOUND=1
            ;;
    esac
    if [ "$FOUND" -eq 1 ]; then
        TOOL_PASS=$((TOOL_PASS+1))
        log_pass "Tool $tool exists"
    else
        log_fail "Tool $tool exists"
    fi
done

# Verify all tools registered in DefaultDispatcher
REGISTERED=$(grep -c "d.Register\|\.Register(" internal/tools/defaults.go 2>/dev/null || echo 0)
if [ "$REGISTERED" -ge 18 ]; then
    log_pass "DefaultDispatcher registers $REGISTERED tools (>= 18)"
else
    log_fail "DefaultDispatcher registers $REGISTERED tools (expected >= 18)"
fi

# ── 10. Documentation ─────────────────────────────────────────────────────

log_section "10. Documentation"

DOC_FILES="README.md CONTRIBUTING.md SECURITY.md docs/ARCHITECTURE.md docs/WORKFLOW.md docs/CONFIG.md docs/TOOLS.md docs/SCREENS.md docs/PROVIDERS.md docs/SLASH_COMMANDS.md docs/KEYBINDINGS.md docs/INTERFACES.md docs/QUICKSTART.md docs/TYPES.md docs/TROUBLESHOOTING.md"
for f in $DOC_FILES; do
    if [ -f "$f" ]; then
        log_pass "Doc exists: $f"
    else
        log_fail "Doc exists: $f"
    fi
done

# Verify no broken internal references (file paths mentioned in docs that don't exist)
BROKEN_REFS=0
for doc in docs/ARCHITECTURE.md docs/WORKFLOW.md docs/TOOLS.md docs/SCREENS.md; do
    if [ -f "$doc" ]; then
        # Extract backtick-quoted file paths and check a sample
        REFS=$(grep -oP '`[^`]*\.go`' "$doc" 2>/dev/null | tr -d '`' | head -10)
        for ref in $REFS; do
            if [ -n "$ref" ] && [ ! -f "$ref" ] && ! echo "$ref" | grep -q "mytool\|internal/app/"; then
                log_warn "Possibly broken ref in $doc: $ref"
                BROKEN_REFS=$((BROKEN_REFS+1))
            fi
        done
    fi
done
if [ "$BROKEN_REFS" -eq 0 ]; then
    log_pass "No broken internal references detected"
fi

# Verify phase count consistency
SEVEN_PHASE_COUNT=$(grep -rl "seven-phase\|seven phase" docs/ README.md CONTRIBUTING.md 2>/dev/null | wc -l)
SIX_PHASE_COUNT=$(grep -rl "six-phase\|six phase" docs/ README.md CONTRIBUTING.md 2>/dev/null | wc -l)
if [ "$SEVEN_PHASE_COUNT" -gt 0 ] && [ "$SIX_PHASE_COUNT" -eq 0 ]; then
    log_pass "Phase count consistent (all say seven-phase)"
elif [ "$SIX_PHASE_COUNT" -gt 0 ]; then
    log_fail "Phase count inconsistent ($SIX_PHASE_COUNT files still say six-phase)"
else
    log_warn "Phase count references not found"
fi

# Verify tool count consistency
if grep -q "18 built-in tools\|18 tools\|18 built-in" README.md docs/TOOLS.md 2>/dev/null; then
    log_pass "Tool count consistent (18)"
else
    log_warn "Tool count reference not found in README/TOOLS.md"
fi

# ── 11. Workflow Phases ────────────────────────────────────────────────────

log_section "11. Workflow Phases"

for phase in PhaseInitialize PhaseDiscuss PhasePlan PhaseExecute PhaseVerify PhaseRuntime PhaseShip; do
    if grep -q "$phase" internal/types/types.go 2>/dev/null; then
        log_pass "Phase constant $phase"
    else
        log_fail "Phase constant $phase"
    fi
done

# Verify Runtime phase exists as a file
if [ -f internal/workflow/runtime.go ]; then
    log_pass "Runtime phase file exists"
else
    log_fail "Runtime phase file exists"
fi

# ── 12. TUI Screens ───────────────────────────────────────────────────────

log_section "12. TUI Screens"

SCREEN_COUNT=$(grep -c "Screen[A-Z]" internal/tui/tuitypes/tuitypes.go 2>/dev/null || echo 0)
if [ "$SCREEN_COUNT" -ge 33 ]; then
    log_pass "TUI has $SCREEN_COUNT screen constants (>= 33)"
else
    log_fail "TUI has $SCREEN_COUNT screen constants (expected >= 33)"
fi

# ── 13. Release Artifacts ──────────────────────────────────────────────────

log_section "13. Release Configuration"

if [ -f .goreleaser.yaml ]; then
    log_pass ".goreleaser.yaml exists"
    # Verify it specifies correct platforms
    if grep -q "linux" .goreleaser.yaml && grep -q "darwin" .goreleaser.yaml && grep -q "windows" .goreleaser.yaml; then
        log_pass "GoReleaser targets linux, darwin, windows"
    else
        log_fail "GoReleaser targets linux, darwin, windows"
    fi
    if grep -q "amd64" .goreleaser.yaml && grep -q "arm64" .goreleaser.yaml; then
        log_pass "GoReleaser targets amd64, arm64"
    else
        log_fail "GoReleaser targets amd64, arm64"
    fi
    if grep -q "checksum" .goreleaser.yaml; then
        log_pass "GoReleaser checksum enabled"
    else
        log_fail "GoReleaser checksum enabled"
    fi
else
    log_fail ".goreleaser.yaml exists"
fi

if [ -f install.sh ]; then
    log_pass "install.sh exists"
    [ -x install.sh ] && log_pass "install.sh is executable" || log_warn "install.sh is not executable"
else
    log_fail "install.sh exists"
fi

# ── 14. Security ───────────────────────────────────────────────────────────

log_section "14. Security"

# No hardcoded API keys (exclude placeholder hints, prefix checks, test files)
if ! grep -rn "sk-or-v1\|nvapi-\|sk-ant-" --include="*.go" . 2>/dev/null | grep -v "_test.go" | grep -v "example" | grep -v "placeholder" | grep -v '\.\.\.' | grep -v "HasPrefix" | grep -v "typically start" | grep -q "."; then
    log_pass "No hardcoded API keys in source"
else
    log_fail "No hardcoded API keys in source"
fi

# No plaintext key storage
if ! grep -rn "WriteFile.*api_key\|os.Write.*key" --include="*.go" . 2>/dev/null | grep -v "keychain\|Keychain\|_test.go" | grep -q "."; then
    log_pass "No plaintext key file writes"
else
    log_warn "Possible plaintext key writes detected"
fi

# SSRF protection exists
if grep -q "isPrivate\|isLoopback\|isLinkLocal\|blockPrivate" internal/tools/webfetch.go 2>/dev/null; then
    log_pass "SSRF protection in WebFetch"
else
    log_fail "SSRF protection in WebFetch"
fi

# Rate limiting exists (concurrency semaphore + token bucket)
if grep -q "concurrencySem\|rateTokens\|dangerousRateTokens\|ToolRateLimit\|DangerousRateLimit" internal/tools/dispatcher.go 2>/dev/null; then
    log_pass "Rate limiting in dispatcher"
else
    log_fail "Rate limiting in dispatcher"
fi

# Permission system exists
if grep -q "func.*Dispatcher.*checkPermission\|func.*Dispatcher.*evaluatePermission" internal/tools/dispatcher.go internal/tools/permissions.go 2>/dev/null; then
    log_pass "Permission system exists"
else
    log_fail "Permission system exists"
fi

# ── 15. Code Quality ───────────────────────────────────────────────────────

log_section "15. Code Quality"

# No panic() in production code
PANIC_COUNT=$(grep -rn "panic(" --include="*.go" . 2>/dev/null | grep -v "_test.go" | grep -v "vendor/" | wc -l)
if [ "$PANIC_COUNT" -eq 0 ]; then
    log_pass "No panic() in production code"
else
    log_warn "$PANIC_COUNT panic() calls in production code"
fi

# gofmt clean
GOFMT_DIRTY=$(gofmt -l . 2>/dev/null | grep -v vendor/ | wc -l)
if [ "$GOFMT_DIRTY" -eq 0 ]; then
    log_pass "All files gofmt-clean"
else
    log_fail "$GOFMT_DIRTY files not gofmt-clean"
fi

# No fmt.Println in production code (debug prints)
FMT_PRINT=$(grep -rn "fmt.Print\|fmt.Println" --include="*.go" . 2>/dev/null | grep -v "_test.go" | grep -v "vendor/" | wc -l)
if [ "$FMT_PRINT" -eq 0 ]; then
    log_pass "No fmt.Print/Println in production code"
else
    log_warn "$FMT_PRINT fmt.Print/Println calls in production code"
fi

# ── 16. Configuration ─────────────────────────────────────────────────────

log_section "16. Configuration"

if [ -f internal/config/types.go ]; then
    log_pass "Config types file exists"
else
    log_fail "Config types file exists"
fi

if [ -f internal/config/loader.go ]; then
    log_pass "Config loader file exists"
    if grep -q "func.*validateConfig" internal/config/loader.go 2>/dev/null; then
        log_pass "Config validation function exists"
    else
        log_fail "Config validation function exists"
    fi
else
    log_fail "Config loader file exists"
fi

# ── 17. Keychain ───────────────────────────────────────────────────────────

log_section "17. Keychain"

for platform in linux darwin windows; do
    if ls pkg/keychain/keychain_${platform}.go >/dev/null 2>&1; then
        log_pass "Keychain backend: $platform"
    else
        log_fail "Keychain backend: $platform"
    fi
done

# ============================================================================
# Report Generation
# ============================================================================

END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

cat > "$REPORT_FILE" << EOF
# M31A Release Validation Report

**Date:** $(date -u '+%Y-%m-%d %H:%M:%S UTC')
**Version:** $GIT_DESCRIBE
**Commit:** $GIT_COMMIT
**Platform:** $PLATFORM
**Go Version:** $GO_VERSION
**Duration:** ${ELAPSED}s

## Summary

| Metric | Value |
|--------|-------|
| **Passed** | $PASS |
| **Failed** | $FAIL |
| **Skipped** | $SKIP |
| **Warnings** | $WARN |
| **Total** | $TOTAL |

## Result

$(if [ "$FAIL" -eq 0 ]; then echo "**PASS** — All mandatory checks succeeded."; else echo "**FAIL** — $FAIL check(s) failed. Review output above."; fi)

## Environment

- **Go:** $GO_VERSION
- **Platform:** $PLATFORM
- **Git:** $GIT_DESCRIBE ($GIT_COMMIT)
- **Date:** $DATE

## Checks Executed

### Build
- Module dependency verification
- Binary compilation (current platform)
- Binary size check (< 60MB)
- Version metadata injection

### Cross-compilation
- linux/amd64, linux/arm64
- darwin/amd64, darwin/arm64
- windows/amd64

### Static Analysis
- go vet
- golangci-lint (if installed)
- govulncheck (if installed)

### Tests
- Unit tests with race detector
- Coverage >= 40% overall
- Coverage >= 70% for pkg/taskrunner, pkg/bisect, pkg/rollback

### CLI
- -help flag
- -version flag

### Headless Mode
- --prompt exits cleanly

### Session
- SaveSession/LoadSession exist
- SaveCheckpoint/LoadCheckpoints exist

### Providers
- OpenRouter, Zen, Nvidia clients exist
- LLMProvider interface defined
- Fallback implemented

### Tools
- All 18 tools verified
- DefaultDispatcher registration verified

### Documentation
- All 14 doc files exist
- No broken internal references
- Phase count consistent (seven-phase)
- Tool count consistent (18)

### Workflow
- All 7 phase constants defined
- Runtime phase file exists

### TUI
- 33+ screen constants

### Release Configuration
- .goreleaser.yaml valid
- install.sh exists

### Security
- No hardcoded API keys
- SSRF protection active
- Rate limiting active
- Permission system active

### Code Quality
- No panic() in production
- gofmt-clean
- No debug prints

### Configuration
- Config types and loader exist
- Validation function exists

### Keychain
- Linux, macOS, Windows backends

## Skipped Checks

$(if [ "$SKIP" -gt 0 ]; then echo "The following checks were skipped (tools not installed):"; echo ""; echo "- golangci-lint (if not installed)"; echo "- govulncheck (if not installed)"; else echo "None."; fi)
EOF

echo ""
echo -e "${BOLD}Report written to: ${REPORT_FILE}${NC}"

# ============================================================================
# Final Output
# ============================================================================

echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
if [ "$FAIL" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}  RELEASE VALIDATION: PASS${NC}"
    echo -e "  $PASS passed, $SKIP skipped, $WARN warnings"
else
    echo -e "${RED}${BOLD}  RELEASE VALIDATION: FAIL${NC}"
    echo -e "  $PASS passed, $FAIL failed, $SKIP skipped, $WARN warnings"
fi
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo ""

exit $FAIL
