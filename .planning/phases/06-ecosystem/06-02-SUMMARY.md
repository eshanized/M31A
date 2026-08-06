---
phase: 06-ecosystem
plan: 02
subsystem: config-extensions
tags: [config, loader, workspace, project, json, merge]
key-files:
  modified:
    - internal/core/config/types.go
    - internal/core/config/loader.go
    - internal/core/config/config_validate.go
    - internal/core/config/merge.go
    - internal/core/config/loader_merge_test.go
tech-stack:
  patterns:
    - Multi-file config merge with defined map for bool handling
    - Walk-up directory discovery (max 3 levels)
    - Precedence: project (JSON) > workspace (TOML) > global (TOML) > env vars
key-decisions:
  - D-03: Multi-file layered config with ExtensionsConfig section
  - Workspace config: .m31a/workspace.toml (TOML), walk-up discovery
  - Project config: m31a.json (JSON for diff-friendly)
  - Env vars now highest precedence (after project JSON)
requirements-completed:
  - D-03
duration: 30 min
completed: "2026-08-06T12:00:00Z"
---

# Phase 06 Plan 02: Config Loader Workspace + Project JSON Layers — Summary

**One-liner:** Extended config loader with workspace (`.m31a/workspace.toml`) and project (`m31a.json`) layers implementing 4-layer precedence.

## Accomplishments

### 1. Workspace Config Layer (internal/core/config/loader.go)
- Added `WorkspaceConfigPath = ".m31a/workspace.toml"` constant
- `findWorkspaceConfig(cwd)` walks up from cwd (max 3 levels like `findProjectConfig`)
- Inserted between global TOML and env vars in load order
- Uses existing `mergeConfig` with `defined` map from `meta.Keys()`

### 2. Project Config as JSON (m31a.json)
- Changed `findProjectConfig()` to search for `m31a.json` instead of `m31a.toml`
- Uses `json.Unmarshal()` with `collectJSONKeys()` to build `defined` map
- `collectJSONKeys()` recursively traverses JSON object for nested paths
- Updated `LocalConfigPath()` → returns `m31a.json`
- Updated `SaveProject()` → `json.MarshalIndent()` for JSON output

### 3. Config Types (internal/core/config/types.go)
- Added `Extensions ExtensionsConfig` field to `Config` with TOML/JSON tags
- Added `ExtensionsConfig`, `ExternalToolConfig`, `ExternalProviderConfig`, `PhaseHookConfig` types
- All nested config structs have both `json` and `toml` tags

### 4. Config Validation (internal/core/config/config_validate.go)
- `validateExtensionsConfig()` validates all extension configs:
  - Tools: command required, timeout parsable
  - Providers: command required, timeout parsable
  - Hooks: command required, timeout ≤ 5min, phases/hook_types non-empty and valid
- Added "extensions" to `knownConfigKeys()`

### 5. Config Merge (internal/core/config/merge.go)
- Added `mergeExtensionsConfig()` called from `MergeConfig()`
- `mergeToolConfigs()`, `mergeProviderConfigs()`, `mergeHookConfigs()` for map merging
- Handles non-existent base maps, field-level merge with `defined` map

### 6. Tests (internal/core/config/loader_merge_test.go)
- Workspace layer tests: found/merged, overrides global, project overrides workspace
- Project JSON tests: loads/merges, precedence over workspace, validation works
- Env var tests: now highest precedence (after project JSON)
- Extensions validation tests: all 8 error cases pass

### 7. Load Order (internal/core/config/loader.go)
Corrected precedence order:
1. Defaults
2. Global TOML (~/.m31a/config.toml)
3. Workspace TOML (.m31a/workspace.toml, walk-up max 3 levels)
4. .env file
5. Project JSON (m31a.json)
6. Env vars (M31A_*) — **highest precedence**

## Verification

All checks pass:
- `go test -race ./internal/core/config/...` ✓ (8.4s)
- `go build ./...` ✓
- `go test ./...` — Core packages pass (pre-existing failures unrelated)

## Deviations from Plan

None — plan executed exactly as written.

## Impact

Enables multi-file layered configuration for the extension platform:
- Teams share workspace defaults via `.m31a/workspace.toml` (committed to repo)
- Projects override with `m31a.json` (diff-friendly JSON)
- Users have global defaults via `~/.m31a/config.toml`
- Env vars provide highest-precedence override for CI/deployment

All subsequent plans (03-06) use this config infrastructure.