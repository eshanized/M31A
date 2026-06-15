---
name: Bug report
about: Something in M31 Autonomous isn't working the way it should.
title: "[bug] "
labels: bug
assignees: ''
---

### Describe the bug

A clear and concise description of what the bug is.

### To reproduce

Steps to reproduce the behavior:

1. Run `m31a` with args `...`
2. Navigate to screen `...`
3. Type command `...`
4. See error

### Expected behavior

What you expected to happen.

### Actual behavior

What actually happened. Include the exact output, error message, or stack trace.

### Environment

- **M31 Autonomous version**: `m31a --version` (or commit SHA if built from source)
- **OS**: [e.g. Ubuntu 24.04, macOS 14.5, Windows 11 / WSL2]
- **Architecture**: [e.g. amd64, arm64]
- **Go version** (if built from source): `go version`
- **Terminal emulator**: [e.g. kitty, alacritty, iTerm2, Windows Terminal]
- **$TERM**: `echo $TERM`
- **Shell**: [e.g. bash 5.2, zsh 5.9, fish 3.7]

### Provider and model

- **Provider**: [openrouter | zen]
- **Model**: [e.g. anthropic/claude-3.5-sonnet]
- **Did auto-fallback trigger?**: [yes | no | unsure]

### Logs

If relevant, paste the contents of `~/.m31a/logs/<session>.log` around the time of the failure. Redact API keys and session content.

<details>
<summary>Log excerpt</summary>

```
paste here
```

</details>

### Screenshots

If the bug is visual (TUI glitch, broken screen, wrong layout), drag a screenshot or screen recording here.

### Additional context

Anything else that might help — related issues, bisect output (`/rollback`), or a minimal repo that reproduces it.
