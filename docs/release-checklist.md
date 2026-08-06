# Release Checklist

**Version:** `vX.Y.Z` (fill in before starting)

## Pre-Release Verification

- [ ] **All CI checks pass on master**
  - [ ] Lint (golangci-lint, gofmt)
  - [ ] Test (race detector, coverage)
  - [ ] Security (govulncheck)
  - [ ] Build (all 6 targets: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64)
  - [ ] Benchmark (no regressions >50%)

- [ ] **Benchmark CI shows no regressions**
  - [ ] Latest benchmark run on master passed
  - [ ] No `p=0.0` regressions in benchstat output

- [ ] **Nightly workflow passed recently**
  - [ ] Full test suite (-race) passed
  - [ ] Extension compat tests passed (custom-linter, ollama-provider, pre-commit-hook)
  - [ ] Security scan (govulncheck) clean

- [ ] **Changelog generated**
  - [ ] Conventional commits since last release
  - [ ] `git log --oneline --grep="^(feat|fix|perf|refactor|docs|test|chore)" v<last>..HEAD`
  - [ ] Grouped by type (feat, fix, perf, refactor, docs, test, chore)

- [ ] **Version bumped per semantic versioning**
  - [ ] MAJOR: Breaking API changes (pkg/extensions, CLI, config schema)
  - [ ] MINOR: New features, new extension points
  - [ ] PATCH: Bug fixes, docs, CI, internal refactors

- [ ] **GoReleaser dry-run succeeds**
  ```bash
  make release-dry
  # Should complete without errors
  ```

- [ ] **Cross-platform builds verified**
  - [ ] linux/amd64
  - [ ] linux/arm64
  - [ ] darwin/amd64
  - [ ] darwin/arm64
  - [ ] windows/amd64
  - [ ] (windows/arm64 excluded - unsupported)

- [ ] **Extension compat tests pass**
  - [ ] custom-linter builds and runs
  - [ ] ollama-provider builds and runs
  - [ ] pre-commit-hook builds and runs

- [ ] **Documentation updated**
  - [ ] CHANGELOG.md reflects new version
  - [ ] README.md version badges updated
  - [ ] API reference (if extensions API changed)
  - [ ] Migration guide (if breaking changes)

- [ ] **Migration guide for breaking changes**
  - [ ] Document what breaks
  - [ ] Provide migration steps
  - [ ] Deprecation timeline (2 minor versions)

## Release Execution

- [ ] **Tag and push release**
  ```bash
  git tag vX.Y.Z
  git push origin vX.Y.Z
  ```

- [ ] **GoReleaser runs**
  - [ ] Creates GitHub Release
  - [ ] Uploads 6 binaries + packages (tar.gz, deb, rpm, apk, archlinux, scoop)
  - [ ] Generates SHA256 checksums
  - [ ] Generates changelog from conventional commits

- [ ] **Verify release artifacts**
  - [ ] All 6 binaries downloadable
  - [ ] Checksums match
  - [ ] Packages install correctly (deb, rpm, etc.)

- [ ] **Announce release**
  - [ ] Post changelog to Discord/Slack
  - [ ] Tweet/social media (if applicable)
  - [ ] Update docs badges

## Post-Release

- [ ] **Monitor for issues** (24-48 hours)
- [ ] **Update Homebrew/Scoop** (if applicable)
- [ ] **Close milestone** on GitHub
- [ ] **Plan next release** (create issues for next cycle)

---

**Sign-off:** _________________ **Date:** _________________