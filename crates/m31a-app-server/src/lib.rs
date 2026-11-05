//! `m31a-app-server`: JSON-RPC daemon, connection multiplexer, turn governor, and client protocol engine.
//!
//! Provides the sovereign background service powering M31A user interfaces (CLI, TUI, IDE extensions).
//! Coordinates persistent threads, client sessions, event broadcasting, and execution admission.

pub mod client;
pub mod connection_multiplexer;
pub mod daemon_lifecycle;
pub mod http_server;
pub mod protocol;
pub mod protocol_schema;
pub mod relay;
pub mod resource_validator;
pub mod test_client;
pub mod turn_admission;
pub mod web_assets;

pub use client::AppServerClient;
pub use connection_multiplexer::{ClientKind, ClientMetadata, ConnectionMultiplexer};
pub use daemon_lifecycle::{
    DaemonLifecycleCoordinator, DaemonShutdownReport, DaemonShutdownTrigger,
    INTERRUPTED_TURN_PREFIX, LoadedThreadsSnapshot, ThreadRecoveryReport,
    setup_control_socket_rendezvous,
};
pub use http_server::{HttpServerConfig, run_http_server, run_http_server_with_listener};
pub use protocol::{AppServer, JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
pub use protocol_schema::{generate_protocol_schema, write_protocol_schema};
pub use relay::stdio_to_uds;
pub use resource_validator::{AuditEvent, ResourceValidator, RpcValidationError};
pub use test_client::{
    AppServerTestHarness, CommandApprovalDecision, FileChangeApprovalDecision,
    LocalAppServerSession,
};
pub use turn_admission::{
    TurnAdmissionConfig, TurnAdmissionController, TurnAdmissionDecision, TurnAdmissionRequest,
    TurnPriority,
};
