//! Distributed Task Mesh Example.
//!
//! Demonstrates a distributed task scheduling and execution mesh built on top of M31A primitives,
//! featuring write-ahead logging (WAL), consensus clustering, scheduler queues, and TCP servers.

pub mod cluster;
pub mod lib_engine;
pub mod metrics;
pub mod scheduler;
pub mod server;
pub mod types;
pub mod wal;

pub use cluster::ClusterMembership;
pub use lib_engine::TaskMeshNode;
pub use metrics::{MetricsSnapshot, TaskMeshMetrics};
pub use scheduler::WorkStealingScheduler;
pub use server::TaskMeshServer;
pub use types::{MeshEvent, NodeId, Priority, TaskDefinition, TaskId, TaskStatus};
pub use wal::{WalRecord, WriteAheadLog};
