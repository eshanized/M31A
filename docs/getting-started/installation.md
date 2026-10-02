# Installation Guide

M31A is distributed as standalone, statically-linked (or minimal glibc) native binaries for Linux, macOS, and Windows. No external runtimes (such as Python, Node.js, or GPU stacks) are required.

For downloads, documentation, and release announcements, visit the official website at [https://m31a.tonmoyinfrastructure.org/](https://m31a.tonmoyinfrastructure.org/).

---

## 1. Quick Install Script (Linux & macOS)

To install the latest release automatically to `~/.local/bin` (or `/usr/local/bin` if root):

```bash
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.sh | bash
```

To install on Windows via PowerShell:

```powershell
irm https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.ps1 | iex
```

Verify your installation:

```bash
m31a --version     # production channel: m31a X.Y.Z
m31a doctor       # includes Deployment: channel/version/target/build
```

### Development vs production channels

M31A ships one core runtime as two isolated installations (see
[`docs/DEPLOYMENT.md`](../DEPLOYMENT.md)):

- **Production** (`m31a`, default): stable releases, stable update channel,
  `~/.config/m31a/` state.
- **Development** (`m31a-dev`): nightly/branch builds, diagnostics and
  experimental features, isolated `~/.config/m31a-dev/` state.
  Both install side-by-side without sharing mutable state.

```bash
# Production
cargo build --release
./scripts/install-local.sh --channel production      # installs `m31a`

# Development (side-by-side safe)
cargo build --release --features development
./scripts/install-local.sh --channel development     # installs `m31a-dev`

m31a-dev --version   # m31a-dev X.Y.Z-dev+<build>
m31a-dev doctor      # Channel: development
```

---

## 2. Pre-Built Release Binaries

You can manually download pre-built release archives from the [GitHub Releases page](https://github.com/eshanized/M31A/releases):

| Platform | Target Architecture | Archive Format |
|:---|:---|:---|
| **Linux** (x86_64) | `x86_64-unknown-linux-gnu` | `m31a-linux-x64.tar.gz` |
| **Linux** (ARM64) | `aarch64-unknown-linux-gnu` | `m31a-linux-arm64.tar.gz` |
| **macOS** (Intel) | `x86_64-apple-darwin` | `m31a-darwin-x64.tar.gz` |
| **macOS** (Apple Silicon) | `aarch64-apple-darwin` | `m31a-darwin-arm64.tar.gz` |
| **Windows** (x86_64) | `x86_64-pc-windows-msvc` | `m31a-windows-x64.zip` |
| **Windows** (ARM64) | `aarch64-pc-windows-msvc` | `m31a-windows-arm64.zip` |

> **Platform qualification:** only Linux x86_64 is verified and SUPPORTED
> (see `docs/PLATFORM-SUPPORT.md`). Other archives are published compile-only
> and are NOT qualified until native runtime evidence exists.

### Verifying Checksums

Every release archive includes an accompanying SHA-256 checksum file (`<archive>.sha256`).

On Linux / macOS:
```bash
sha256sum -c m31a-linux-x64.tar.gz.sha256
```

On Windows:
```powershell
Get-FileHash m31a-windows-x64.zip -Algorithm SHA256
```

---

## 3. Building From Source (Cargo)

If you prefer building from source, ensure you have the stable Rust toolchain (1.85+) installed:

```bash
# Clone the repository
git clone https://github.com/eshanized/M31A.git
cd M31A

# Build optimized release binary
cargo build --release

# Install locally
cargo install --path .
```

The resulting binary will be placed in `$HOME/.cargo/bin/m31a`. Ensure `$HOME/.cargo/bin` is in your `$PATH`.

---

## 4. System Diagnostics

Run the built-in diagnostic tool to verify host capabilities and environment readiness:

```bash
m31a doctor
```

This verifies:
- Available sandbox confinement backends (`cgroups v2`, POSIX `rlimits`, Windows Job Objects)
- SQLite storage permissions
- PTY allocation capabilities
- Configured LLM provider API credentials
