# Phase 5 — Session State & Configuration: Validation

## Test Coverage Map

### Plan 05-01 — Keychain (`pkg/keychain/`)

| Test Function | Task | Requirement | File |
|---|---|---|---|
| TestKeychain_SetGet | 3 | P5.2 | keychain_test.go |
| TestKeychain_SetDeleteGet | 3 | P5.2 | keychain_test.go |
| TestKeychain_GetNotFound | 3 | P5.2 | keychain_test.go |
| TestKeychain_DeleteNotFound | 3 | P5.2 | keychain_test.go |
| TestKeychain_ServiceNameFormat | 3 | P5.2 | keychain_test.go |
| TestKeychain_EmptyService | 3 | P5.2 | keychain_test.go |
| TestKeychain_InterfaceCompliance | 3 | P5.2 | keychain_test.go |
| TestKeychain_NewReturnsInterface | 3 | P5.2 | keychain_test.go |

**Validation command:** `go test -count=1 -v ./pkg/keychain/`

### Plan 05-02 — Session CRUD (`pkg/session/`)

| Test Function | Task | Requirement | File |
|---|---|---|---|
| TestSession_NewAndLoad | 3 | P5.3 | manager_test.go |
| TestSession_IDLength | 3 | P5.3 | manager_test.go |
| TestSession_NewSetsTimestamps | 3 | P5.3 | manager_test.go |
| TestSession_LoadMissing | 3 | P5.3 | manager_test.go |
| TestSession_LoadCorruptJSON | 3 | P5.3 | manager_test.go |
| TestSession_LoadMissingMessages | 3 | P5.3 | manager_test.go |
| TestSession_ListSessions | 3 | P5.3 | manager_test.go |
| TestSession_ListSessionsEmpty | 3 | P5.3 | manager_test.go |
| TestSession_DeleteSession | 3 | P5.3 | manager_test.go |
| TestSession_ArchiveSession | 3 | P5.3 | manager_test.go |
| TestSession_ArchiveAndList | 3 | P5.3 | manager_test.go |
| TestSession_SaveSession | 3 | P5.3 | manager_test.go |
| TestSession_NewSessionCollision | 3 | P5.3 | manager_test.go |

**Validation command:** `go test -count=1 -v ./pkg/session/ -run TestSession`

### Plan 05-03 — Planning Files + Checkpoint (`pkg/session/`)

| Test Function | Task | Requirement | File |
|---|---|---|---|
| TestPlanning_SaveAndLoadProject | 3 | P5.4 | planning_test.go |
| TestPlanning_LoadProjectMissing | 3 | P5.4 | planning_test.go |
| TestPlanning_SaveAndLoadTasks | 3 | P5.4 | planning_test.go |
| TestPlanning_LoadTasksMissing | 3 | P5.4 | planning_test.go |
| TestPlanning_SaveAndLoadState | 3 | P5.4 | planning_test.go |
| TestPlanning_LoadStateMissing | 3 | P5.4 | planning_test.go |
| TestPlanning_ToleratesExtraWhitespace | 3 | P5.4 | planning_test.go |
| TestPlanning_ToleratesMissingSections | 3 | P5.4 | planning_test.go |
| TestPlanning_TasksWithDeps | 3 | P5.4 | planning_test.go |
| TestPlanning_StateTimestampFormat | 3 | P5.4 | planning_test.go |
| TestCheckpoint_SaveAndLoad | 3 | P5.5 | checkpoint_test.go |
| TestCheckpoint_MaxRetention | 3 | P5.5 | checkpoint_test.go |
| TestCheckpoint_Latest | 3 | P5.5 | checkpoint_test.go |
| TestCheckpoint_EmptyState | 3 | P5.5 | checkpoint_test.go |

**Validation command:** `go test -count=1 -v ./pkg/session/ -run 'TestPlanning|TestCheckpoint'`

### Plan 05-04 — Config + Token Estimation (`internal/config/`, `internal/tokens/`)

| Test Function | Task | Requirement | File |
|---|---|---|---|
| TestConfig_DefaultConfig | 3 | P5.1 | loader_test.go |
| TestConfig_LoadTOMLParse | 3 | P5.1 | loader_test.go |
| TestConfig_LoadMissingFile | 3 | P5.1 | loader_test.go |
| TestConfig_LoadEnvThemeOverride | 3 | P5.1 | loader_test.go |
| TestConfig_LoadEnvModelOverride | 3 | P5.1 | loader_test.go |
| TestConfig_LoadConfigPathOverride | 3 | P5.1 | loader_test.go |
| TestConfig_SaveAtomic | 3 | P5.1 | loader_test.go |
| TestConfig_SaveEnvOverride | 3 | P5.1 | loader_test.go |
| TestConfig_KeyResolutionEnvVar | 3 | P5.1 | loader_test.go |
| TestConfig_KeyResolutionKeychain | 3 | P5.1 | loader_test.go |
| TestConfig_KeyResolutionConfigFile | 3 | P5.1 | loader_test.go |
| TestConfig_KeyResolutionOrder | 3 | P5.1 | loader_test.go |
| TestEstimator_NewWithKnownModel | 3 | P5.7 | estimator_test.go |
| TestEstimator_NewWithUnknownModel | 3 | P5.7 | estimator_test.go |
| TestEstimator_EstimateWithTokenizer | 3 | P5.7 | estimator_test.go |
| TestEstimator_EstimateFallback | 3 | P5.7 | estimator_test.go |
| TestEstimator_EstimateEmptyString | 3 | P5.7 | estimator_test.go |
| TestEstimator_EstimateMultibyte | 3 | P5.7 | estimator_test.go |
| TestEstimator_Calibrate | 3 | P5.7 | estimator_test.go |
| TestEstimator_CalibrateZeroEstimated | 3 | P5.7 | estimator_test.go |
| TestEstimator_CalibrateConvergence | 3 | P5.7 | estimator_test.go |
| TestEstimator_FormatUsage | 3 | P5.7 | estimator_test.go |
| TestEstimator_FormatUsageZeroTotal | 3 | P5.7 | estimator_test.go |
| TestEstimator_ContextWarningBannerBelowThreshold | 3 | P5.7 | estimator_test.go |
| TestEstimator_ContextWarningBannerAboveThreshold | 3 | P5.7 | estimator_test.go |
| TestEstimator_DefaultThreshold | 3 | P5.7 | estimator_test.go |

**Validation command:** `go test -count=1 -v ./internal/config/ && go test -count=1 -v ./internal/tokens/`

### Plan 05-05 — Settings + Resume Screens (`internal/tui/`)

| Test Function | Task | Requirement | File |
|---|---|---|---|
| TestSettings_InitialRender | 4 | P5.1 | settings_test.go |
| TestSettings_TabCycling | 4 | P5.1 | settings_test.go |
| TestSettings_TabContent | 4 | P5.1 | settings_test.go |
| TestSettings_APIMasking | 4 | P5.1 | settings_test.go |
| TestSettings_SaveAction | 4 | P5.1 | settings_test.go |
| TestSettings_DiscardOnEsc | 4 | P5.1 | settings_test.go |
| TestSettings_WindowSize | 4 | P5.1 | settings_test.go |
| TestResume_InitialRender | 4 | P5.6 | resume_test.go |
| TestResume_ListSessions | 4 | P5.6 | resume_test.go |
| TestResume_CorruptedBadge | 4 | P5.6 | resume_test.go |
| TestResume_EnterSelectsSession | 4 | P5.6 | resume_test.go |
| TestResume_NewSession | 4 | P5.6 | resume_test.go |
| TestResume_DeleteConfirmation | 4 | P5.6 | resume_test.go |
| TestResume_DeleteConfirmYes | 4 | P5.6 | resume_test.go |
| TestResume_DeleteConfirmNo | 4 | P5.6 | resume_test.go |
| TestResume_EscReturnsToREPL | 4 | P5.6 | resume_test.go |
| TestResume_WindowSize | 4 | P5.6 | resume_test.go |

**Validation command:** `go test -count=1 -v ./internal/tui/ -run 'TestSettings|TestResume'`

## Totals

| Plan | Tests | Requirements |
|---|---|---|
| 05-01 (Keychain) | 8 | P5.2 |
| 05-02 (Session CRUD) | 13 | P5.3 |
| 05-03 (Planning + Checkpoint) | 14 | P5.4, P5.5 |
| 05-04 (Config + Tokens) | 26 | P5.1, P5.7 |
| 05-05 (Screens) | 17 | P5.1, P5.6 |
| **Total** | **78** | **P5.1–P5.7** |

## Gate Commands

### Per-Task Verification (after each task commit)
```bash
go test -race -cover ./pkg/keychain/ ./pkg/session/ ./internal/config/ ./internal/tokens/ ./internal/tui/
```

### Per-Wave Verification
```bash
# Wave 1 (Plans 05-01, 05-02)
go test -race -cover ./pkg/keychain/ ./pkg/session/

# Wave 2 (Plans 05-03, 05-04) — must run after Wave 1
go test -race -cover ./pkg/keychain/ ./pkg/session/ ./internal/config/ ./internal/tokens/

# Wave 3 (Plan 05-05) — must run after Wave 2
go test -race -cover ./...
```

### Phase Gate (final)
```bash
CGO_ENABLED=0 go build -o m31a ./cmd/m31a && \
go vet ./... && \
go test -race -cover ./...
```

## Wave 0 Setup

Before any Wave 1 task commences:
- [x] Directory `.planning/phases/05/` exists
- [x] All 5 PLAN.md files written
- [x] VALIDATION.md written (this file)
- [ ] `go get` dependencies: `BurntSushi/toml`, `pkoukk/tiktoken-go`, `godbus/dbus/v5`
- [ ] `go mod tidy` passes
- [ ] Existing 319 tests still pass

## Sampling Continuity

| Task | Sampling Command |
|---|---|
| Every task commit | `go test -race -cover ./package/...` for the package under work |
| Every wave merge to main | `go test -race -cover ./...` |
| Phase gate | Full suite: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a && go vet ./... && go test -race -cover ./...` |
