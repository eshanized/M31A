//! `m31a-ebpf`: Kernel-level eBPF execution auditing and provenance tracing.
//!
//! Provides Linux kernel-level execution observation, process lineage correlation,
//! bounded ring buffer event transport, event loss tracking, and secret redaction.
//! Operates strictly as an observational audit layer without bypassing or altering
//! authoritative sandbox execution policies.

pub mod auditor;
pub mod benchmark;
pub mod correlator;
pub mod events;
pub mod probe;
pub mod redaction;
pub mod ring_buffer;

pub use auditor::{AuditQueryFilter, AuditSummaryReport, ExecutionAuditor};
pub use benchmark::{AuditBenchmarkReport, AuditBenchmarker};
pub use correlator::{ExecutionCorrelator, ExecutionProvenance};
pub use events::{AuditEventKind, AuditRecord};
pub use probe::{BpfCapabilityStatus, BpfProbe};
pub use redaction::SecretRedactor;
pub use ring_buffer::BoundedAuditRingBuffer;
