# Pull Request Template

## Description
<!-- What does this PR do? Why? -->

## Related Issue
<!-- Fixes #XXX or Related to #XXX -->

## Type of Change
- [ ] feat: New feature
- [ ] fix: Bug fix
- [ ] docs: Documentation only
- [ ] test: Test-only changes
- [ ] refactor: Code cleanup, no behavior change
- [ ] chore: Config, tooling, dependencies
- [ ] perf: Performance improvement

## Testing
<!-- How did you test this? -->
- [ ] Unit tests: `make test` passes
- [ ] Integration tests: `make test` covers new code
- [ ] Manual testing: commands run
  - Commands run:
  - Expected output:
  - Actual output:

## Checklist
- [ ] `make check` passes locally
- [ ] Tests added/updated for new functionality
- [ ] Documentation updated (if user-facing)
- [ ] Changelog entry added (if applicable)
- [ ] No breaking changes (or migration guide provided)
- [ ] Conventional commit messages used
- [ ] No emojis in code or docs
- [ ] Error wrapping with `fmt.Errorf("%w", err)` used

## Additional Context
<!-- Any other information, configuration, or data that might be necessary to review this PR. -->