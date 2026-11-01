# Getting Support for M31A

Welcome to M31A support! Whether you are experiencing an unexpected issue, need guidance on integrating M31A into your development workflow, or have questions about NVIDIA NIM models, this guide is here to help.

---

## 1. Quick Self-Diagnostics

Before opening an issue, run the built-in diagnostic health check:

```bash
m31a doctor
```

`m31a doctor` inspects:
- **NVIDIA NIM API Key**: Verifies presence and valid formatting of `NVIDIA_API_KEY`.
- **API Base Connectivity**: Tests reachability of `https://integrate.api.nvidia.com/v1`.
- **Sandbox Engine**: Confirms whether `bubblewrap` (`bwrap`) is installed and functional.
- **Git State**: Checks git repository root, branch state, and uncommitted changes.
- **SQLite Database**: Verifies accessibility of local database storage (`~/.m31a/`).
- **Daemon Socket**: Inspects whether the app-server daemon socket is responsive.

If an error is detected, `m31a doctor` provides immediate remediation steps.

---

## 2. Common Troubleshooting Steps

### "bubblewrap binary not found"
- **Linux**: Install `bubblewrap` via your package manager:
  - Ubuntu/Debian: `sudo apt install bubblewrap`
  - Fedora: `sudo dnf install bubblewrap`
  - Arch: `sudo pacman -S bubblewrap`
- **macOS / Non-Linux**: Run with `--sandbox read-only` or `--dangerously-bypass-approvals-and-sandbox` in trusted local environments.

### "NVIDIA_API_KEY environment variable is not set"
Export your key in your current shell or add it to `~/.bashrc` / `~/.zshrc`:
```bash
export NVIDIA_API_KEY="nvapi-..."
```
Alternatively, save it permanently in `~/.m31a/config.toml`:
```toml
api_key = "nvapi-..."
```

### Collecting Feedback Bundles for Issues
If an error occurs, you can automatically capture a redacted diagnostics archive using:
```bash
m31a feedback
```
This generates a ZIP archive containing sanitized logs, configuration settings, and crash traces suitable for attaching to a GitHub issue.

---

## 3. Support Channels

### GitHub Issues
- **Bug Reports**: If you found a bug or unexpected crash, open a [Bug Report](file:///home/snigdha/Desktop/org.m31a/.github/ISSUE_TEMPLATE/bug_report.yml).
- **Feature Requests**: If you want to propose a new command, model integration, or capability, open a [Feature Request](file:///home/snigdha/Desktop/org.m31a/.github/ISSUE_TEMPLATE/feature_request.yml).
- **Model Provider Requests**: If you'd like a new NVIDIA NIM model slug supported or optimized, open a [Model Provider Request](file:///home/snigdha/Desktop/org.m31a/.github/ISSUE_TEMPLATE/model_provider_request.yml).

### GitHub Discussions
Join our community on GitHub Discussions for:
- "How-to" questions and best practices
- Sharing custom skills and MCP configurations
- General architectural discussions and brainstorming

### Security Inquiries
For vulnerabilities or security boundary concerns, do **not** file public issues. Follow our [Security Policy](file:///home/snigdha/Desktop/org.m31a/SECURITY.md) and report privately to `security@tonmoyinfrastructure.org`.
