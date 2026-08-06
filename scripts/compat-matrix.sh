#!/bin/bash
# compat-matrix.sh — Compatibility matrix for curated OSS projects
# Clones and builds real-world projects to track M31A compatibility.
# Must be executable: chmod +x scripts/compat-matrix.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Default output path
OUTPUT_FILE="${1:-compat-results.json}"

# Handle --output flag
if [[ "$1" == "--output" ]]; then
    OUTPUT_FILE="$2"
fi

cd "$PROJECT_ROOT"

# Curated projects: associative array of name -> "url|language|size|repo_type"
# Size: small, medium, large
# Repo type: mono, poly
declare -A PROJECTS=(
    ["kubernetes/kubernetes"]="https://github.com/kubernetes/kubernetes.git|Go|large|mono"
    ["golang/go"]="https://github.com/golang/go.git|Go|large|mono"
    ["hashicorp/terraform"]="https://github.com/hashicorp/terraform.git|Go|medium|poly"
    ["prometheus/prometheus"]="https://github.com/prometheus/prometheus.git|Go|medium|poly"
    ["docker/compose"]="https://github.com/docker/compose.git|Go|medium|poly"
    ["etcd-io/etcd"]="https://github.com/etcd-io/etcd.git|Go|medium|poly"
    ["microsoft/vscode"]="https://github.com/microsoft/vscode.git|TypeScript|large|poly"
    ["facebook/react"]="https://github.com/facebook/react.git|TypeScript|large|poly"
    ["rust-lang/rust"]="https://github.com/rust-lang/rust.git|Rust|large|mono"
    ["python/cpython"]="https://github.com/python/cpython.git|Python|large|mono"
    ["astral-sh/ruff"]="https://github.com/astral-sh/ruff.git|Rust|medium|poly"
    ["charmbracelet/bubbletea"]="https://github.com/charmbracelet/bubbletea.git|Go|small|poly"
    ["golang/lint"]="https://github.com/golang/lint.git|Go|small|poly"
    ["moby/moby"]="https://github.com/moby/moby.git|Go|large|mono"
    ["hashicorp/consul"]="https://github.com/hashicorp/consul.git|Go|medium|poly"
)

# Cleanup function
cleanup() {
    if [ -n "${TEMP_DIR:-}" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT INT TERM

TEMP_DIR=$(mktemp -d -t compat-matrix-XXXXXX)

echo "=== M31A Compatibility Matrix ==="
echo "Temp directory: $TEMP_DIR"
echo "Output file: $OUTPUT_FILE"
echo ""

# Initialize results
TOTAL=0
PASSED=0
FAILED=0

declare -A PER_CATEGORY_TOTAL
declare -A PER_CATEGORY_PASSED

JSON_ARRAY=""

for project in "${!PROJECTS[@]}"; do
    IFS='|' read -r url language size repo_type <<< "${PROJECTS[$project]}"

    echo "---"
    echo "Testing: $project ($language, $size, $repo_type)"
    echo "URL: $url"

    TOTAL=$((TOTAL + 1))
    PER_CATEGORY_TOTAL[$language]=$((${PER_CATEGORY_TOTAL[$language]:-0} + 1))

    PROJECT_DIR="$TEMP_DIR/$project"
    STATUS="fail"
    ERROR_MSG=""

    # Clone
    if git clone --depth 1 --quiet "$url" "$PROJECT_DIR" 2>/dev/null; then
        echo "  Cloned successfully"

        # Build based on language
        cd "$PROJECT_DIR"
        BUILD_SUCCESS=false

        case "$language" in
            "Go")
                # Try standard Go build
                if CGO_ENABLED=0 go build ./... 2>&1 | tail -5; then
                    BUILD_SUCCESS=true
                fi
                ;;
            "TypeScript")
                # Check for package.json and try npm build
                if [ -f package.json ]; then
                    if command -v npm >/dev/null 2>&1; then
                        if npm ci --silent 2>/dev/null && npm run build 2>&1 | tail -5; then
                            BUILD_SUCCESS=true
                        fi
                    fi
                fi
                ;;
            "Rust")
                # Check for Cargo.toml and try cargo build
                if [ -f Cargo.toml ]; then
                    if command -v cargo >/dev/null 2>&1; then
                        if cargo build --release 2>&1 | tail -5; then
                            BUILD_SUCCESS=true
                        fi
                    fi
                fi
                ;;
            "Python")
                # Python projects - just check if they can be imported
                if [ -f setup.py ] || [ -f pyproject.toml ] || [ -f requirements.txt ]; then
                    if python3 -m py_compile $(find . -name "*.py" -not -path "./.git/*" | head -5) 2>&1 | tail -5; then
                        BUILD_SUCCESS=true
                    fi
                fi
                ;;
        esac

        if [ "$BUILD_SUCCESS" = true ]; then
            STATUS="pass"
            PASSED=$((PASSED + 1))
            PER_CATEGORY_PASSED[$language]=$((${PER_CATEGORY_PASSED[$language]:-0} + 1))
            echo "  ✅ Build passed"
        else
            STATUS="fail"
            FAILED=$((FAILED + 1))
            ERROR_MSG="Build failed"
            echo "  ❌ Build failed"
        fi
    else
        STATUS="fail"
        FAILED=$((FAILED + 1))
        ERROR_MSG="Clone failed"
        echo "  ❌ Clone failed"
    fi

    # Add to JSON array
    JSON_ARRAY="${JSON_ARRAY},\n    {\n      \"name\": \"$project\",\n      \"language\": \"$language\",\n      \"size\": \"$size\",\n      \"repo_type\": \"$repo_type\",\n      \"status\": \"$STATUS\",\n      \"error\": \"$ERROR_MSG\"\n    }"

    cd "$PROJECT_ROOT"
done

# Remove leading comma
JSON_ARRAY="${JSON_ARRAY#,\n}"

# Build per-category results
CATEGORY_JSON=""
for lang in "${!PER_CATEGORY_TOTAL[@]}"; do
    total=${PER_CATEGORY_TOTAL[$lang]}
    passed=${PER_CATEGORY_PASSED[$lang]:-0}
    rate=$(awk -v p="$passed" -v t="$total" 'BEGIN { if (t > 0) printf "%.1f", (p/t)*100; else print "0.0" }')
    CATEGORY_JSON="${CATEGORY_JSON},\n    {\n      \"language\": \"$lang\",\n      \"total\": $total,\n      \"passed\": $passed,\n      \"pass_rate\": $rate\n    }"
done
CATEGORY_JSON="${CATEGORY_JSON#,\n}"

# Generate final JSON
cat > "$OUTPUT_FILE" <<EOF
{
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "total": $TOTAL,
  "passed": $PASSED,
  "failed": $FAILED,
  "pass_rate": $(awk -v p="$PASSED" -v t="$TOTAL" 'BEGIN { if (t > 0) printf "%.1f", (p/t)*100; else print "0.0" }'),
  "per_project": [
$JSON_ARRAY
  ],
  "per_category": [
$CATEGORY_JSON
  ]
}
EOF

# Validate JSON
if command -v jq >/dev/null 2>&1; then
    if ! jq . "$OUTPUT_FILE" >/dev/null 2>&1; then
        echo "ERROR: Generated JSON is invalid"
        exit 1
    fi
elif command -v python3 >/dev/null 2>&1; then
    if ! python3 -c "import json; json.load(open('$OUTPUT_FILE'))" 2>/dev/null; then
        echo "ERROR: Generated JSON is invalid"
        exit 1
    fi
else
    echo "WARNING: No JSON validator (jq or python3) found, skipping validation"
fi

echo ""
echo "=== Compatibility Matrix Results ==="
echo "Total: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
PASS_RATE=$(awk -v p="$PASSED" -v t="$TOTAL" 'BEGIN { if (t > 0) printf "%.1f", (p/t)*100; else print "0.0" }')
echo "Pass Rate: $PASS_RATE%"
echo ""
echo "Per-category:"
for lang in "${!PER_CATEGORY_TOTAL[@]}"; do
    total=${PER_CATEGORY_TOTAL[$lang]}
    passed=${PER_CATEGORY_PASSED[$lang]:-0}
    rate=$(awk -v p="$passed" -v t="$total" 'BEGIN { if (t > 0) printf "%.1f", (p/t)*100; else print "0.0" }')
    echo "  $lang: $passed/$total ($rate%)"
done
echo ""
echo "Results written to: $OUTPUT_FILE"

# Exit with non-zero if pass rate below 80%
if (( $(awk -v r="$PASS_RATE" 'BEGIN { print (r < 80) }') )); then
    echo "⚠️  Pass rate below 80% ($PASS_RATE%)"
    exit 1
fi

echo "✅ Pass rate above 80%"
exit 0