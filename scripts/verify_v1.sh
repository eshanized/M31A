#!/usr/bin/env bash
# verify_v1.sh — Acceptance criteria verification for M31A v1.0.0
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

PASS=0
FAIL=0
TOTAL=0

check() {
    local desc="$1"
    shift
    TOTAL=$((TOTAL + 1))
    if "$@" >/dev/null 2>&1; then
        PASS=$((PASS + 1))
        echo "  PASS: $desc"
    else
        FAIL=$((FAIL + 1))
        echo "  FAIL: $desc"
    fi
}

echo "=== M31A v1.0.0 Acceptance Criteria ==="
echo ""

# --- Build & Test ---
echo "-- Build & Test --"
check "Binary builds cleanly" bash -c 'CGO_ENABLED=0 go build -o /tmp/m31a_verify ./cmd/m31a'
check "go vet passes" go vet ./...
check "All tests pass" go test -race ./...

# Coverage check — run first, then parse
go test -race -cover -coverprofile=/tmp/verify_coverage.out ./... >/dev/null 2>&1
COVERAGE=$(go tool cover -func=/tmp/verify_coverage.out 2>/dev/null | grep "total:" | awk '{print $NF}' | sed 's/%//')
TOTAL=$((TOTAL + 1))
if [ -n "$COVERAGE" ] && [ "$(echo "$COVERAGE >= 75" | bc)" -eq 1 ] 2>/dev/null; then
    PASS=$((PASS + 1))
    echo "  PASS: Coverage >= 75% (current: ${COVERAGE}%)"
else
    FAIL=$((FAIL + 1))
    echo "  FAIL: Coverage >= 75% (current: ${COVERAGE:-unknown}%)"
fi

# Critical package coverage
go test -race -cover ./... > /tmp/verify_pkg_cov.txt 2>&1
for pkg in pkg/taskrunner pkg/bisect pkg/rollback; do
    PKG_COV=$(grep "github.com/eshanized/M31A/$pkg" /tmp/verify_pkg_cov.txt | sed 's/.*coverage: \([0-9.]*\).*/\1/')
    TOTAL=$((TOTAL + 1))
    if [ -n "$PKG_COV" ] && [ "$(echo "$PKG_COV >= 90" | bc)" -eq 1 ] 2>/dev/null; then
        PASS=$((PASS + 1))
        echo "  PASS: $pkg coverage >= 90% (current: ${PKG_COV}%)"
    else
        FAIL=$((FAIL + 1))
        echo "  FAIL: $pkg coverage >= 90% (current: ${PKG_COV:-unknown}%)"
    fi
done

# --- Cross-compile ---
echo ""
echo "-- Cross-Platform Build --"
for target in "linux/amd64" "linux/arm64" "darwin/amd64" "darwin/arm64" "windows/amd64"; do
    OS="${target%%/*}"
    ARCH="${target##*/}"
    check "Cross-compile $OS/$ARCH" env GOOS="$OS" GOARCH="$ARCH" CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
done

# --- Documentation ---
echo ""
echo "-- Documentation --"
check "README.md exists" test -f README.md
check "CONTRIBUTING.md exists" test -f CONTRIBUTING.md
check "CHANGELOG.md exists" test -f CHANGELOG.md
check "docs/SLASH_COMMANDS.md exists" test -f docs/SLASH_COMMANDS.md
check "docs/CONFIG.md exists" test -f docs/CONFIG.md
check "docs/ARCHITECTURE.md exists" test -f docs/ARCHITECTURE.md

# --- Release Files ---
echo ""
echo "-- Release Pipeline --"
check ".goreleaser.yaml exists" test -f .goreleaser.yaml
check "install.sh exists" test -f install.sh
check "install.sh is executable" test -x install.sh
check "--help flag works" /tmp/m31a_verify -help
check "--version flag works" /tmp/m31a_verify -version

# --- Architecture ---
echo ""
echo "-- Architecture --"
check "No CGO in build" bash -c 'CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a'
check "No telemetry imports" bash -c '! grep -r "telemetry\|analytics\|phone.home" --include="*.go" . | grep -v "planning\|roadmap\|\.md"'
check "No direct Anthropic/OpenAI imports" bash -c '! grep -rl "anthropic.com\|api.openai.com" --include="*.go" internal/ 2>/dev/null'
check "OpenRouter client exists" test -f internal/provider/openrouter/client.go
check "Zen client exists" test -f internal/provider/zen/client.go

# --- TUI ---
echo ""
echo "-- TUI Screens --"
check "REPL screen" grep -q "ScreenREPL" internal/tui/types.go
check "Settings screen" grep -q "ScreenSettings" internal/tui/types.go
check "Resume screen" grep -q "ScreenResume" internal/tui/types.go
check "Plan screen" grep -q "ScreenPlan" internal/tui/types.go
check "Execute screen" grep -q "ScreenExecute" internal/tui/types.go
check "Verify screen" grep -q "ScreenVerify" internal/tui/types.go
check "Ship screen" grep -q "ScreenShip" internal/tui/types.go
check "First-run screen" grep -q "ScreenFirstRun" internal/tui/types.go
check "Permission screen" grep -q "ScreenPermission" internal/tui/types.go
check "Model selector screen" grep -q "ScreenModelSelector" internal/tui/types.go

# --- Workflow ---
echo ""
echo "-- Workflow Engine --"
check "Initialize phase" grep -q "PhaseInitialize" internal/types/types.go
check "Discuss phase" grep -q "PhaseDiscuss" internal/types/types.go
check "Plan phase" grep -q "PhasePlan" internal/types/types.go
check "Execute phase" grep -q "PhaseExecute" internal/types/types.go
check "Verify phase" grep -q "PhaseVerify" internal/types/types.go
check "Ship phase" grep -q "PhaseShip" internal/types/types.go

# --- Tools ---
echo ""
echo "-- Core Tools --"
check "Bash tool" test -f internal/tools/bash.go
check "FileRead tool" test -f internal/tools/fileread.go
check "FileWrite tool" test -f internal/tools/filewrite.go
check "Glob tool" test -f internal/tools/glob.go
check "Grep tool" test -f internal/tools/grep.go

# --- Keychain ---
echo ""
echo "-- Keychain --"
check "Linux keychain" test -f pkg/keychain/keychain_linux.go
check "macOS keychain" test -f pkg/keychain/keychain_darwin.go
check "Windows keychain" test -f pkg/keychain/keychain_windows.go

# --- Error Handling ---
echo ""
echo "-- Error Handling --"
check "Sentinel errors defined" test -f internal/errors/errors.go
check "Graceful Ctrl+C handling" grep -q "ctrl+c" internal/tui/app.go
check "Offline mode" grep -q "offline mode" internal/tui/app.go
check "errors.Is() for fallback" grep -q "errors.Is" internal/tui/app.go

# --- Features ---
echo ""
echo "-- Signature Features --"
check "Arbitrage package" test -f pkg/arbitrage/arbitrage.go
check "Ledger package" test -f pkg/ledger/ledger.go
check "Rollback package" test -f pkg/rollback/rollback.go
check "AutoDream package" test -f pkg/autodream/autodream.go
check "Bisect package" test -f pkg/bisect/bisect.go
check "Task runner package" test -f pkg/taskrunner/runner.go
check "Session manager" test -f pkg/session/manager.go

# --- Slash Commands ---
echo ""
echo "-- Slash Commands --"
check "16+ commands registered" bash -c '[ "$(grep -c "r.Register" internal/tui/commands.go)" -ge 16 ]'

# --- Cleanup ---
rm -f /tmp/m31a_verify /tmp/verify_coverage.out

echo ""
echo "=== Results: $PASS/$TOTAL passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
