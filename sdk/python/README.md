# M31A Python SDK (`m31a`)

Official Python client library for **M31A**, the high-performance autonomous agentic coding engine powered by NVIDIA NIM.

Provides strongly typed asynchronous and synchronous clients, multi-transport communication (Unix Domain Sockets, Subprocess Stdio, HTTP/SSE), turn streaming abstractions with DeepSeek R1 / Nemotron reasoning chunk inspection, automated tool approval & interactive question callbacks, snapshot checkpoints, path-based concurrency locks, and distributed message board pub-sub.

---

## Installation

```bash
pip install m31a
```

*(Zero external dependencies! Only standard library Python >= 3.10 required.)*

---

## Features

- **Asynchronous & Synchronous Clients**:
  - `M31AClient`: Native `asyncio` client with `async with` support.
  - `SyncM31AClient`: Blocking synchronous wrapper for scripts, CLI utilities, and Jupyter notebooks.
- **Multi-Transport Support**:
  - `UdsTransport`: Fast local IPC over Unix Domain Sockets (`~/.m31a/app-server-control/app-server-control.sock` or custom path).
  - `StdioTransport`: Directly launches and orchestrates `m31a app-server --stdio` as a subprocess.
  - `HttpTransport`: HTTP POST JSON-RPC 2.0 with Server-Sent Events (SSE) streaming for remote and cloud deployments.
  - `MockTransport`: In-memory transport for unit testing without a running server.
- **Full Protocol Parity**: All 40+ M31A App Server RPC endpoints (`thread/*`, `turn/*`, `checkpoint/*`, `locks/*`, `board/*`, `goal/*`, `inbox/*`, `claims/*`, `skills/*`, `models/*`).
- **Turn Streaming & Reasoning Introspection**:
  - `on_reasoning`: Real-time DeepSeek R1 / Nemotron thinking steps.
  - `on_content`: Real-time text chunks.
  - `on_tool_started`, `on_tool_finished`: Live tool execution monitoring.
  - `async for event in turn:`: Asynchronous event generator.
- **Safety & Policy Integration**:
  - Automated or interactive tool approvals (`on_approval` -> `"allow"`, `"deny"`, or `ApprovalDecision`).
  - Interactive multi-choice question answers (`on_question`).
- **Checkpoints & Session Recovery**:
  - Snapshot checkpoints with git commit attribution and modified file lists.
  - Rollback session and workspace state.

---

## Quickstart

### 1. Async Streaming Turn

```python
import asyncio
from m31a import M31AClient

async def main():
    async with M31AClient() as client:
        # Create a session thread
        thread = await client.create_thread(cwd=".")
        print(f"Session initialized: {thread.id}")

        # Run turn with live reasoning & content stream
        turn = await thread.run(
            "Audit performance bottlenecks and generate a flamegraph profile.",
            on_reasoning=lambda chunk: print(f"\033[35m{chunk}\033[0m", end="", flush=True),
            on_content=lambda chunk: print(chunk, end="", flush=True),
            on_approval=lambda req: "allow",
        )

        final_answer = await turn.result()
        print("\nTurn completed:\n", final_answer)

asyncio.run(main())
```

### 2. Synchronous Quickstart (Scripts & Notebooks)

```python
from m31a import SyncM31AClient

with SyncM31AClient() as client:
    diag = client.get_diagnostics()
    print("M31A Health:", diag.status, "Model:", diag.active_model)

    thread = client.create_thread()
    turn = thread.run(
        "Explain async vs sync execution models in Python.",
        on_content=lambda chunk: print(chunk, end="", flush=True),
    )

    print("\nResult:", turn.result())
```

### 3. Checkpoints & Safe Rollbacks

```python
async with M31AClient() as client:
    thread = await client.create_thread()

    # Create snapshot checkpoint before modifications
    checkpoint = await thread.create_checkpoint("Pre-Refactor Snapshot", ["m31a/"])

    # Run turn with safety approval
    turn = await thread.run("Apply patch to parser module.")
    await turn.result()

    # Roll back anytime if needed
    await thread.restore_checkpoint(checkpoint.id, revert_conversation=True)
```

### 4. Distributed Multi-Agent Message Board

```python
async with M31AClient() as client:
    # Post updates across agent mesh
    await client.post_to_board(
        body="Backend unit test suite passed.",
        channel="ci-cd",
        author="test-agent",
        payload={"tests_passed": 12, "failures": 0},
    )

    # Read posts
    posts = await client.list_board_posts("ci-cd", limit=10)
    for p in posts:
        print(f"[{p.author}] {p.body}")
```

---

## Transports Configuration

```python
# 1. Unix Domain Socket (Default)
client = M31AClient(socket_path="/path/to/custom.sock")

# 2. HTTP / Server-Sent Events (SSE)
client = M31AClient(url="http://127.0.0.1:4096")

# 3. Subprocess Stdio Transport
from m31a import StdioTransport

client = M31AClient(
    transport=StdioTransport(
        binary_path="m31a",
        args=["app-server", "stdio"],
        cwd="/workspace",
    )
)
```

---

## License

MIT
