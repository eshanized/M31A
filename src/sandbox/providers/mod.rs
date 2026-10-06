//! Concrete sandbox providers (Linux bubblewrap, macOS Seatbelt, Windows Job Object, and fallback process isolation).

pub mod bubblewrap;
pub mod macos;
pub mod process;
pub mod windows;

pub use bubblewrap::BubblewrapSandboxProvider;
pub use macos::SeatbeltSandboxProvider;
pub use process::ProcessIsolationProvider;
pub use windows::WindowsSandboxProvider;
