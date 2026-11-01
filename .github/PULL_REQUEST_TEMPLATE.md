## Description

Please include a summary of the changes and the related issue/motivation. Include any architectural context necessary for reviewers.

Fixes #(issue)

---

## Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds capability)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Performance optimization
- [ ] Documentation update
- [ ] Refactoring / Code cleanup

---

## Crate / Subsystem Affected

- [ ] `crates/m31a-cli`
- [ ] `crates/m31a-tui`
- [ ] `crates/m31a-agent-engine`
- [ ] `crates/m31a-provider`
- [ ] `crates/m31a-sandbox` / `m31a-execpolicy` / `m31a-guardian`
- [ ] `crates/m31a-tools` / `m31a-patch`
- [ ] `crates/m31a-app-server`
- [ ] `sdk/python`
- [ ] `sdk/typescript`
- [ ] Other

---

## Checklist

- [ ] My code adheres to the style guidelines of this project (`cargo fmt --all`).
- [ ] I have run `cargo clippy --workspace --all-targets --all-features -- -D warnings` without errors.
- [ ] I have executed the full workspace test suite (`cargo test --workspace`) and all tests pass.
- [ ] I have added tests that prove my fix is effective or that my feature works.
- [ ] I have updated documentation or README files accordingly.
- [ ] My commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
- [ ] I have verified that no sensitive credentials or keys are checked in.
