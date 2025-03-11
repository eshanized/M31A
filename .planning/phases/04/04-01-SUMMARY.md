# Summary 04-01 — Glob Tool

## Files Created
- `internal/tools/glob.go` — Glob tool with `doublestar` recursive globbing (`**` support), sorted results with size/timestamp table, 1000-result cap with truncation, rg-based .gitignore-aware listing when both rg and .gitignore exist
- `internal/tools/glob_test.go` — 6 tests: simple pattern, recursive pattern, no matches, max results, invalid pattern, missing param

## Files Modified
- `go.mod` — added `github.com/bmatcuk/doublestar/v4 v4.10.0`
- `go.sum` — updated with doublestar checksums

## Deviations from Plan
- **rg integration**: Changed to only use rg when `.gitignore` file exists in workDir + rg is available. The `rg --files --glob` approach requires a git repo context; without `.gitignore`, doublestar is the correct choice for filesystem-level globbing.

## Verification
- `go build ./...` — passes
- `go vet ./...` — passes
- `go test -race -count=1 ./internal/tools/... -run TestGlob` — 6/6 pass
