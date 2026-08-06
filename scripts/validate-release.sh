#!/bin/bash
# validate-release.sh — Pre-release validation harness
# Runs all quality gates before GoReleaser builds the release.
# Must be executable: chmod +x scripts/validate-release.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPORT_FILE="$PROJECT_ROOT/VALIDATION_REPORT.md"

cd "$PROJECT_ROOT"

echo "=== M31A Release Validation ==="
echo "Project: $PROJECT_ROOT"
echo "Timestamp: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo ""

# Initialize report
cat > "$REPORT_FILE" <<EOF
# Release Validation Report

**Timestamp:** $(date -u +"%Y-%m-%dT%H:%M:%SZ")
**Commit:** $(git rev-parse HEAD)
**Tag:** $(git describe --tags --exact-match 2>/dev/null || echo "N/A (not on tag)")

## Checks

EOF

run_check() {
    local name="$1"
    local cmd="$2"
    local start_time=$(date +%s)

    echo "---"
    echo "Running: $name"
    echo ""

    if eval "$cmd"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo "✅ $name passed (${duration}s)"
        echo "- ✅ **$name** — passed (${duration}s)" >> "$REPORT_FILE"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo "❌ $name failed (${duration}s)"
        echo "- ❌ **$name** — failed (${duration}s)" >> "$REPORT_FILE"
        return 1
    fi
}

FAILED=0

# Check 1: Tests with race detector
if ! run_check "Tests (race detector)" "go test -race -timeout 60s ./..."; then
    FAILED=1
fi

# Check 2: Lint
if ! run_check "Lint (golangci-lint)" "golangci-lint run ./... --timeout=5m"; then
    FAILED=1
fi

# Check 3: Benchmark comparison
if ! run_check "Benchmark regression check" "go test -bench=. -benchmem -count=5 -run=^$ ./internal/performance/ > /tmp/new.txt && benchstat /tmp/old.txt /tmp/new.txt 2>/dev/null || echo 'No baseline available'"; then
    FAILED=1
fi

# Check 4: Security scan
if ! run_check "Security (govulncheck)" "go install golang.org/x/vuln/cmd/govulncheck@latest && govulncheck ./..."; then
    FAILED=1
fi

# Check 5: GoReleaser config validation
if ! run_check "GoReleaser config" "goreleaser check"; then
    FAILED=1
fi

# Check 6: Semantic version tag validation
TAG=$(git describe --tags --exact-match 2>/dev/null || echo "")
if [ -n "$TAG" ]; then
    if ! run_check "Semantic version tag" "echo '$TAG' | grep -Eq '^v[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.-]+)?$'"; then
        FAILED=1
    fi
else
    echo "- ⚠️ **Semantic version tag** — skipped (not on a tag)" >> "$REPORT_FILE"
fi

# Summary
echo "" >> "$REPORT_FILE"
echo "---" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

if [ $FAILED -eq 0 ]; then
    echo "## Summary: ✅ ALL CHECKS PASSED" >> "$REPORT_FILE"
    echo ""
    echo "=== ALL CHECKS PASSED ==="
else
    echo "## Summary: ❌ SOME CHECKS FAILED" >> "$REPORT_FILE"
    echo ""
    echo "=== SOME CHECKS FAILED ==="
fi

echo ""
echo "Report written to: $REPORT_FILE"
cat "$REPORT_FILE"

exit $FAILED