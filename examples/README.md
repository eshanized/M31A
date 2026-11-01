# M31A Examples

This directory contains standalone examples and reference architectures demonstrating the integration and capabilities of **M31A**.

---

## Directory Overview

| Example | Description | Language / Technology |
| :--- | :--- | :--- |
| [`distributed_task_mesh`](./distributed_task_mesh) | Distributed multi-agent task orchestration mesh featuring distributed WAL replication, cluster heartbeats, metrics aggregation, and parallel execution. | Rust (Tokio, Serde) |

---

## Running the Examples

### Distributed Task Mesh

The `distributed_task_mesh` crate provides a complete reference architecture showing how multiple M31A agents can coordinate across an event-driven cluster with write-ahead logs (WAL) and consensus scheduling:

```bash
# Run unit and integration tests for distributed task mesh
cargo test -p distributed_task_mesh

# Build in release mode
cargo build --release -p distributed_task_mesh
```

---

## Additional SDK Examples

For client library examples interacting with M31A via JSON-RPC over Unix Domain Sockets or stdio:
- **Python SDK Examples**: See [`sdk/python/examples/`](file:///home/snigdha/Desktop/org.m31a/sdk/python/examples) (`sync_quickstart.py`, `interactive_stream.py`, `checkpoint_rollback.py`).
- **TypeScript SDK Examples**: See [`sdk/typescript/examples/`](file:///home/snigdha/Desktop/org.m31a/sdk/typescript/examples) (`interactive_stream.ts`, `approval_agent.ts`, `board_pubsub.ts`).
