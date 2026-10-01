//! Cross-platform contract test suite.
//!
//! This test file includes all platform capability tests organized by
//! common semantics and platform-specific implementations.

#[path = "platform/common/capabilities.rs"]
mod platform_common;

#[path = "platform/linux/mod.rs"]
mod platform_linux;

#[path = "platform/macos/mod.rs"]
mod platform_macos;

#[path = "platform/windows/mod.rs"]
mod platform_windows;
