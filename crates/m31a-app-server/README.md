# `m31a-app-server`

JSON-RPC 2.0 daemon, connection multiplexer, turn governor, and client protocol engine for **M31A**.

---

## Overview

`m31a-app-server` implements the sovereign background daemon that exposes M31A's runtime to external clients, including IDE extensions (VS Code, Cursor), CLI wrappers, web interfaces, and official SDKs (Python and TypeScript). It manages child process stdio streams, Unix Domain Sockets, and HTTP/SSE endpoints with turn admission control and event multiplexing.

---

## User-Facing Capabilities

- **Background Daemon (`m31a daemon start`)**: Runs M31A as a persistent system daemon, enabling instant CLI command execution and multi-client connection sharing.
- **IDE Integration**: Powers VS Code and Cursor integrations over JSON-RPC 2.0 stdio or Unix Domain Sockets.
- **Session Recovery & Reconnection**: Clients can disconnect and reconnect without interrupting ongoing model generation or background builds.

---

## Developer Guide & Architecture

### Key Modules

- [`protocol`](src/protocol.rs): JSON-RPC 2.0 request, response, error, and notification definitions covering 40+ methods (`thread/*`, `turn/*`, `models/*`, `checkpoint/*`, `board/*`, `locks/*`).
- [`connection_multiplexer`](src/connection_multiplexer.rs): Multiplexes events across multiple active client connections (`ClientKind`, `ClientMetadata`).
- [`turn_admission`](src/turn_admission.rs): Manages execution queues, concurrency gates, and steering tokens.
- [`daemon_lifecycle`](src/daemon_lifecycle.rs): Handles socket rendezvous, PID files, graceful teardown (`DaemonShutdownReport`), and crash recovery.
- [`auth`](src/auth.rs): Token-based authentication, scope verification (`AuthScope`), and client identity validation.
- [`http_server`](src/http_server.rs): HTTP POST JSON-RPC handler and Server-Sent Events (SSE) notification broadcaster.

### Programmatic Usage

```rust
use m31a_app_server::client::AppServerClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = AppServerClient::connect_uds("~/.m31a/app-server-control/app-server-control.sock").await?;
    let diagnostics = client.get_diagnostics().await?;
    println!("Connected to M31A daemon v{}", diagnostics.version);
    Ok(())
}
```

---

## Testing

```bash
cargo test -p m31a-app-server
```
