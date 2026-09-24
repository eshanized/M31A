//! Stream storage module
//!
//! Per PST-02, append-only files store high-volume session/event/transcript streams.

pub mod file_stream;

pub use file_stream::{FileStream, StreamStore};
