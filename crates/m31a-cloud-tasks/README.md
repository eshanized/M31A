# `m31a-cloud-tasks`

gRPC client for remote agent fleet dispatch and scalable cloud worker execution in **M31A**.

---

## Overview

`m31a-cloud-tasks` provides client and protocol definitions for delegating compute-intensive agent workloads, large compilation matrices, long test suites, and parallel benchmarks to remote cloud runtimes over gRPC/Tonic with TLS.

---

## User-Facing Capabilities

- **Remote Task Offloading (`m31a cloud`)**: Enables offloading heavy builds, speculative candidate evaluations, and integration test executions to dedicated remote execution clusters.
- **Agent Fleet Spawning**: Allows local sessions to orchestrate remote subagents seamlessly.

---

## Developer Guide & Architecture

### Protocol & Key Modules

- [`proto`](build.rs): Compiles `proto/cloud_tasks.proto` via `tonic-build` into strongly typed RPC stubs (`m31a.cloud.tasks.v1`).
- `CloudTasksClient`: Handles TLS endpoint configuration, token authentication, `ExecuteTaskRequest`, and `SpawnAgentRequest`.

### Programmatic Usage

```rust
use m31a_cloud_tasks::CloudTasksClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = CloudTasksClient::connect("https://cloud.m31a.io:8443").await?;
    let capabilities = client.get_capabilities().await?;
    println!("Remote fleet available workers: {}", capabilities.available_workers);
    Ok(())
}
```

---

## Testing

```bash
cargo test -p m31a-cloud-tasks
```
