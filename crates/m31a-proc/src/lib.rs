//! `m31a-proc`: Background process supervisor and asynchronous execution manager.
//!
//! Spawns, monitors, and terminates background worker processes, managing streaming I/O buffers,
//! process groups, exit statuses, and resource consumption.

pub mod buffer;
pub mod error;
pub mod manager;
pub mod process;

pub use buffer::{CircularOutputBuffer, OutputStream, ProcessLine};
pub use error::ProcessError;
pub use manager::{ProcessManager, ProcessManagerConfig};
pub use process::{ManagedProcess, ProcessInfo, ProcessOutput, ProcessStatus, ProcessStopResult};
