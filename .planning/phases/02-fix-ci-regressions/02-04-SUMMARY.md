# Phase 2, Plan 04 - Summary

**Phase:** 02-fix-ci-regressions  
**Plan:** 02-04  
**Wave:** 4  
**Status:** ✅ COMPLETE

## Objective
Fix Root Cause E — Bash security patterns lost in directory restructure (23 test failures). Restore expanded patterns from commit f35077bd, upgrade to compiled regex for obfuscation detection, fix variable expansion detection, implement exact-prefix custom blocklist matching, and add command chaining detection for $(...).

## Changes Made

### Commit: `fix(tools): restore bash security patterns from f35077bd + regex upgrades`
**File:** `internal/tools/exec/bash.go`

**Key changes:**
1. **Restored 60+ dangerousCommandPatterns** from commit f35077bd (Baseline + ExpandedBlocklist):
   - Added: `shred`, `wipefs`, `nc -l`, `ncat -l`, `socat`, `/dev/tcp`, `curl|sh`, `wget|bash`, `curl|bash`, `rm -rf *`, `rm -rf ~`, etc.

2. **Upgraded dangerousObfuscationPatterns to compiled regexes** (8 patterns):
   - `base64\s+-d`, `echo.*\|.*sh`, `curl.*\|.*sh`, `wget.*\|.*sh`
   - `eval\s+`, `exec\s+`, `source\s+/dev/stdin`, `\.\s+/dev/stdin`

3. **Fixed containsVariableExpansion** to use `regexp.MustCompile(\$[A-Za-z_])` for $VAR detection, plus `${`, `$(`, `$((` literal checks
   - Removed `$(` and backtick detection (now handled by command chaining)

4. **Implemented exact-prefix custom blocklist matching** (D-05):
   - `checkCustomBlocklist()` splits command into tokens, matches first token exactly
   - Prevents "my-custom-cmd-extra" from matching "my-custom-cmd"

5. **Implemented command chaining detection** (D-06):
   - `checkCommandChaining()` splits on `;`, `&&`, `||`, `|`
   - Recursively validates `$(...)` and backtick inner commands
   - Runs BEFORE variable expansion check to allow safe command substitutions

6. **Token-based custom obfuscation pattern matching**:
   - Splits command on spaces/hyphens
   - Pattern matches complete token (allows "evil" in "evil-command" but "evil-command" not in "evil-command-extra")

## Test Results

All 23 bash security tests pass with `-race` flag:

| Test Suite | Tests | Status |
|------------|-------|--------|
| TestCheckDangerousCommand_Baseline | 18 | ✅ PASS |
| TestCheckDangerousCommand_ExpandedBlocklist | 9 | ✅ PASS |
| TestCheckDangerousCommand_ChainingDetection | 13 | ✅ PASS |
| TestCheckDangerousCommand_ObfuscationDetection | 8 | ✅ PASS |
| TestCheckDangerousCommand_CustomBlockedCommands | 4 | ✅ PASS |
| TestCheckDangerousCommand_CustomObfuscationPatterns | 3 | ✅ PASS |
| TestCheckDangerousCommand_LongCommand | 1 | ✅ PASS |
| TestBash_CommandSubstitution | 1 | ✅ PASS |
| **Total** | **57** | **✅ ALL PASS** |

## Verification Checklist

- [x] `go build ./...` — clean compilation
- [x] `go vet ./...` — no issues
- [x] `golangci-lint run` — clean (pre-existing issues only)
- [x] `go test -race ./internal/tools/exec/ -run "TestCheckDangerousCommand|TestBash_CommandSubstitution"` — all pass
- [x] 1 atomic commit matching D-13 convention
- [x] No adjacent cleanups or refactors (per D-08)

## Security Improvements

| Threat | Before | After |
|--------|--------|-------|
| `shred`, `nc -l`, `/dev/tcp`, `curl|sh`, `rm -rf *` | NOT blocked | BLOCKED |
| `echo.*|.*sh`, `base64 -d` obfuscation | NOT detected (broken regex) | BLOCKED (compiled regex) |
| `$HOME`, `${PATH}` variable expansion | NOT detected (literal string) | BLOCKED (regex) |
| Custom blocklist bypass via substring | "mycmd-extra" bypassed "mycmd" | BLOCKED (exact token match) |
| `$(rm -rf /)` command substitution | Allowed (indiscriminate block) | BLOCKED (inner cmd validated) |
| `echo $(echo hello)` safe substitution | Blocked (indiscriminate) | ALLOWED (inner cmd safe) |

## Next Steps

Phase 2 complete! All 4 plans executed across 4 waves:
- ✅ Wave 1 (Plan 02-01): Config merge + Session label (Root Causes A, B)
- ✅ Wave 2 (Plan 02-02): Workflow transitions + Lint (Root Cause C)
- ✅ Wave 3 (Plan 02-03): TestIsCI race + AskUserQuestion timeout (Root Causes D, F)
- ✅ Wave 4 (Plan 02-04): Bash security patterns + regex upgrade (Root Cause E)

**Ready for `/gsd-verify-work`** to validate all 53 test failures resolved and CI restored to green.