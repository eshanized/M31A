"""Rust toolchain definitions for cross-platform M31A Bazel compilation.

Configures toolchains for Linux GNU, Linux MUSL (static binaries),
macOS (x86_64, aarch64), and Windows targets.
"""

load("@rules_rust//rust:defs.bzl", "rust_toolchain")

# Linux GNU toolchain
rust_toolchain(
    name = "linux_gnu_toolchain",
    edition = "2024",
    target = "x86_64-unknown-linux-gnu",
    linker = "gcc",
)

# Linux musl toolchain (for static binaries)
rust_toolchain(
    name = "linux_musl_toolchain",
    edition = "2024",
    target = "x86_64-unknown-linux-musl",
    linker = "gcc",
    linker_flags = ["-static"],
)

# macOS x86_64
rust_toolchain(
    name = "macos_x86_64_toolchain",
    edition = "2024",
    target = "x86_64-apple-darwin",
)

# macOS ARM64
rust_toolchain(
    name = "macos_arm64_toolchain",
    edition = "2024",
    target = "aarch64-apple-darwin",
)

# Windows MSVC
rust_toolchain(
    name = "windows_msvc_toolchain",
    edition = "2024",
    target = "x86_64-pc-windows-msvc",
)

# Windows GNU (gnullvm)
rust_toolchain(
    name = "windows_gnullvm_toolchain",
    edition = "2024",
    target = "x86_64-pc-windows-gnullvm",
)