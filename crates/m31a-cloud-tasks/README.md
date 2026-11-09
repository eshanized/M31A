# `m31a-cloud-tasks`

> gRPC-based remote task execution client, distributed agent fleet dispatch, and cloud workload offloading for M31A.

---

## Why Does This Crate Exist?

While local execution in Linux Bubblewrap sandboxes works brilliantly for normal projects, certain engineering workloads exceed local laptop resources:
- Running heavy End-to-End integration test matrices across 10 distinct database containers.
- Compiling massive monorepos requiring 64+ vCPUs and 128 GB of RAM.
- Distributing speculative solution candidate benchmarks across a cluster of dedicated compute nodes.

`m31a-cloud-tasks` enables M31A to offload heavy compilation, testing, and agent execution to remote cloud compute fleets over authenticated gRPC.

---

## Core Concepts & How It Works

### 1. High-Performance gRPC Transport (Tonic & Protobuf)
Uses typed Protocol Buffers (`m31a.cloud.tasks.v1`) to stream task requests, tool inputs, and console outputs with low latency and binary compression.

### 2. Enterprise Authentication & TLS
Supports multiple enterprise security schemes:
- Mutual TLS (mTLS) with custom CA root certificates.
- API Key authentication.
- Cloud Workload Identity Federation (GCP/AWS/Azure).
- OAuth2 Bearer tokens.

### 3. Remote Capability Discovery
Before dispatching a task, the client queries `GetCapabilities` on the remote worker node to verify available CPU cores, memory headroom, GPU acceleration, and installed toolchains.

---

## Key Types & Structs

| Struct / Enum | Description |
| :--- | :--- |
| `CloudTasksClient` | Asynchronous Tonic gRPC client communicating with remote fleet endpoints. |
| `CloudTasksConfig` | Endpoint URL, TLS certificates, timeouts, and authentication settings. |
| `CloudAuth` | Auth schemes: `ApiKey`, `WorkloadIdentity`, `OAuth2`, or `None`. |
| `ExecuteTaskRequest` | Serialized command, environment variables, working directory, and timeout. |

---

## Practical Usage Example

```rust
use m31a_cloud_tasks::{CloudAuth, CloudTasksConfig};

fn main() -> anyhow::Result<()> {
    let config = CloudTasksConfig {
        endpoint: "https://tasks.fleet.internal:8443".into(),
        auth: CloudAuth::ApiKey {
            key: "fleet-token-xyz...".into(),
        },
        tls_cert_path: None,
        tls_key_path: None,
        ca_cert_path: None,
        timeout_seconds: 300,
        max_retries: 3,
    };

    config.validate()?;
    println!("Cloud Tasks client configured for: {}", config.endpoint);
    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-cloud-tasks
```
