#!/usr/bin/env bash
# Configuration authority static guard (CI).
#
# Fails when production code reintroduces hidden configuration authorities:
# bare operational `unwrap_or(<literal>)` fallbacks, or scattered
# production model/provider/endpoint literals outside the canonical
# authority. Scope-aware: test modules, fixtures, retired-descriptor
# compat, curated capability metadata, and explicitly classified
# (ALGORITHMIC / IMMUTABLE / COMPATIBILITY) sites are exempt.
#
# The same rules are enforced in-process by
# tests/config_architecture_enforcement.rs; this script gives fast CI
# feedback without compiling.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
report() { echo "CONFIG-AUTHORITY VIOLATION: $1"; fail=1; }

# Precompute unit-test module boundaries: lines at/after `mod tests` in
# files containing #[cfg(test)] are normative fixtures, never production
# authorities.
declare -A TEST_TAIL_START=()
while IFS= read -r f; do
  if rg -q '#\[cfg\(test\)\]' "$f"; then
    start=$(rg -n '^\s*mod tests' "$f" | head -n1 | cut -d: -f1 || true)
    if [[ -n "${start:-}" ]]; then TEST_TAIL_START["$f"]="$start"; fi
  fi
done < <(rg -l --glob '*.rs' '#\[cfg\(test\)\]' src/ || true)

in_test_tail() {
  local file="$1" lineno="$2"
  local start="${TEST_TAIL_START[$file]:-}"
  [[ -n "$start" && "$lineno" -ge "$start" ]]
}

is_comment_line() {
  local file="$1" lineno="$2"
  local text
  text=$(sed -n "${lineno}p" "$file" | sed 's/^[[:space:]]*//')
  case "$text" in
    "//"*) return 0;;
  esac
  return 1
}

# ── 1. Bare operational numeric fallbacks ───────────────────────────────
# Literals that look like timeouts/limits/concurrency restated inline.
# Every match must either reference a named constant or carry a
# classification marker (see tests/config_architecture_enforcement.rs).
while IFS= read -r hit; do
  file="${hit%%:*}"; rest="${hit#*:}"; lineno="${rest%%:*}"; line="${rest#*:}"
  if in_test_tail "$file" "$lineno"; then continue; fi
  if is_comment_line "$file" "$lineno"; then continue; fi
  # Exempt: test modules, fixtures, canonical authority itself, schema
  # defaults (the authority's serde surface), retired-descriptor compat.
  case "$file" in
    *testing/*|*test_*) continue;;
  esac
  case "$line" in
    *ALGORITHMIC*|*IMMUTABLE*|*COMPATIBILITY*|*"Normative Artifact"*|*canonical::*|*canonical\ defaults*|*single\ authority*|*Single\ authority*|*"test-only"*|*"test fixture"*) continue;;
  esac
  case "$file" in
    src/config/canonical.rs|src/config/schema.rs) continue;;
  esac
  if [[ "$file" == "src/config/provider_registry.rs" ]] && [[ "$line" == *"is_production_supported: false"* ]]; then continue; fi
  if [[ "$file" == "src/tui/screens/wizard.rs" ]] && [[ "$line" == *"UNAVAILABLE"* ]]; then continue; fi
  if [[ "$file" == "src/model/provider/nvidia_metadata.rs" ]]; then continue; fi
  if [[ "$file" == "src/model/catalog.rs" ]] && [[ "$line" == *"CANONICAL_REAL_MODEL"* ]]; then continue; fi
  report "$hit"
done < <(rg -n 'unwrap_or\((25|30|50|60|100|300|600|1000|1024|4096|65536|1048576)\)' src/ || true)

# ── 2. Scattered production model/endpoint literals ─────────────────────
while IFS= read -r hit; do
  file="${hit%%:*}"; rest="${hit#*:}"; lineno="${rest%%:*}"; line="${rest#*:}"
  if in_test_tail "$file" "$lineno"; then continue; fi
  if is_comment_line "$file" "$lineno"; then continue; fi
  case "$file" in
    *testing/*|*test_*) continue;;
  esac
  case "$line" in
    *ALGORITHMIC*|*IMMUTABLE*|*COMPATIBILITY*|*"Normative Artifact"*|*canonical::*|*"test-only"*) continue;;
  esac
  case "$file" in
    src/config/canonical.rs|src/config/schema.rs) continue;;
  esac
  if [[ "$file" == "src/config/provider_registry.rs" ]]; then continue; fi
  if [[ "$file" == "src/model/provider/nvidia_metadata.rs" ]]; then continue; fi
  if [[ "$file" == "src/model/catalog.rs" ]] && [[ "$line" == *"CANONICAL_REAL_MODEL"* ]]; then continue; fi
  if [[ "$file" == "src/tui/screens/wizard.rs" ]] && [[ "$line" == *"UNAVAILABLE"* ]]; then continue; fi
  if [[ "$file" == "src/model/provider/endpoint.rs" ]] && [[ "$line" == *"CANONICAL_NVIDIA_BASE_URL"* ]]; then continue; fi
  # Unit-test tails are exempt (handled in-process); script-level: skip
  # lines that are clearly inside #[cfg(test)] tails is approximate, so
  # only flag non-test-looking production lines here.
  report "$hit"
done < <(rg -n 'integrate\.api\.nvidia\.com|api\.openai\.com|api\.anthropic\.com|generativelanguage\.googleapis\.com|gpt-4o|claude-3-5-sonnet|gemini-1\.5-pro|llama3:latest|nemotron-3-ultra' src/ || true)

# ── 3. Canonical consumer pins ──────────────────────────────────────────
for pin in \
  "src/tools/process/mod.rs:DEFAULT_TOOL_TIMEOUT_SECS" \
  "src/controller/dependencies.rs:DEFAULT_RUNTIME_CONCURRENCY" \
  "src/tui/screens/wizard.rs:DEFAULT_RUNTIME_CONCURRENCY" \
  "src/planning/service.rs:DEFAULT_RUNTIME_TIMEOUT_SECS" \
  "src/config/schema.rs:DEFAULT_AGENT_MAX_TOKENS" \
  "src/workflow/genesis/intake.rs:DEFAULT_MAX_DISCOVERY_TURNS" \
  "src/repo/query.rs:DEFAULT_QUERY_MAX_RESULTS" \
  ; do
  file="${pin%%:*}"; marker="${pin#*:}"
  if ! rg -q "$marker" "$file"; then
    report "$file no longer consumes canonical $marker"
  fi
done

if [[ "$fail" -ne 0 ]]; then
  echo "config-authority guard FAILED (see violations above)"
  exit 1
fi
echo "config-authority guard OK"
