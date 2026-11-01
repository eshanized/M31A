# M31A SDK Ecosystem

Official multi-language client SDKs for **M31A**, the high-performance autonomous agentic coding engine powered by NVIDIA NIM.

```
                      +-----------------------------+
                      |       M31A App Server       |
                      |    (JSON-RPC 2.0 Daemon)    |
                      +--------------+--------------+
                                     |
              +----------------------+----------------------+
              |                      |                      |
     [Unix Domain Socket]      [Subprocess Stdio]      [HTTP / SSE]
              |                      |                      |
    +---------v----------+ +---------v----------+ +---------v----------+
    |    UdsTransport    | |   StdioTransport   | |   HttpTransport    |
    +---------+----------+ +---------+----------+ +---------+----------+
              \                      |                      /
               \                     |                     /
           +----v--------------------+--------------------v----+
           |                                                   |
           |             Official Client Libraries             |
           |                                                   |
           |   TypeScript SDK (@m31a/sdk)   |   Python SDK (m31a)   |
           +---------------------------------------------------+
```

---

## Available Packages

| Language | Package Directory | Package Name | Transports | Key Highlights |
| :--- | :--- | :--- | :--- | :--- |
| **TypeScript / Node.js** | [`sdk/typescript`](./typescript) | `@m31a/sdk` | UDS, Stdio, HTTP/SSE, Mock | Async iterable streaming, DeepSeek R1 reasoning paths, approvals, sections |
| **Python** | [`sdk/python`](./python) | `m31a` | UDS, Stdio, HTTP/SSE, Mock | `asyncio` & synchronous client (`SyncM31AClient`), zero external dependencies |

---

## Architecture & Capabilities Matrix

| Feature Area | Protocol Method | TypeScript SDK | Python SDK |
| :--- | :--- | :---: | :---: |
| **Server Diagnostics** | `server/diagnostics` | `client.getDiagnostics()` | `client.get_diagnostics()` |
| **Configuration** | `config/read` | `client.readConfig()` | `client.read_config()` |
| **NVIDIA Models Catalog** | `models/list` | `client.listModels()` | `client.list_models()` |
| **Tool Definitions** | `tools/list` | `client.listTools()` | `client.list_tools()` |
| **Skill Subsystem** | `skills/list` | `client.listSkills()` | `client.list_skills()` |
| **Instruction Sources** | `instructions/list` | `client.listInstructions()` | `client.list_instructions()` |
| **Shell Command Exec** | `command/exec` | `client.execCommand()` | `client.exec_command()` |
| **Thread Sessions** | `thread/create`, `read`, `list` | `client.createThread()` | `client.create_thread()` |
| **Session Archival & Forking** | `thread/archive`, `fork`, `resume` | `client.forkThread()`, etc. | `client.fork_thread()`, etc. |
| **Thread Sections** | `thread/section/*` | `client.createThreadSection()` | `client.create_thread_section()` |
| **Turn Admission & Execution** | `turn/start`, `finish` | `thread.run()`, `client.startTurn()` | `thread.run()`, `client.start_turn()` |
| **DeepSeek R1 / Reasoning Stream** | `notification/turn_reasoning_chunk` | `onReasoning` callback | `on_reasoning` callback |
| **Tool Invocation Hooks** | `notification/tool_*` | `onToolStarted`, `onToolFinished` | `on_tool_started`, `on_tool_finished` |
| **Steering & Interruption** | `turn/steer`, `turn/interrupt` | `turn.steer()`, `turn.interrupt()` | `turn.steer()`, `turn.interrupt()` |
| **Interactive Approvals** | `turn/approve`, `turn/deny` | `onApprovalRequested` callback | `on_approval` callback |
| **Interactive Questions** | `turn/answer` | `onQuestionRequested` callback | `on_question` callback |
| **Snapshot Checkpoints** | `checkpoint/create`, `restore` | `thread.createCheckpoint()`, etc. | `thread.create_checkpoint()`, etc. |
| **Concurrency Path Locks** | `locks/acquire`, `locks/release` | `client.acquireLock()`, etc. | `client.acquire_lock()`, etc. |
| **Message Board Pub-Sub** | `board/post`, `board/list` | `client.postToBoard()`, etc. | `client.post_to_board()`, etc. |
| **Session Inboxes & Goals** | `inbox/*`, `goal/*` | `thread.setGoal()`, `enqueueInbox()` | `thread.set_goal()`, `enqueue_inbox()` |

---

## Directory Layout

```
sdk/
├── typescript/                 # Official TypeScript / JavaScript SDK
│   ├── src/
│   │   ├── client.ts           # M31AClient with all RPC endpoints
│   │   ├── thread.ts           # Fluent ThreadHandle abstraction
│   │   ├── turn.ts             # ActiveTurn streaming controller
│   │   ├── transport.ts        # UDS, Stdio, HTTP/SSE, Mock transports
│   │   ├── types.ts            # Full protocol types & notification maps
│   │   ├── errors.ts           # Exception hierarchy
│   │   ├── index.ts            # Public entrypoint
│   │   └── test/               # Node test runner suite
│   ├── examples/               # TypeScript runnable demonstrations
│   └── README.md
│
├── python/                     # Official Python SDK
│   ├── m31a/
│   │   ├── client.py           # Async M31AClient
│   │   ├── sync_client.py      # Blocking SyncM31AClient
│   │   ├── thread.py           # ThreadHandle abstraction
│   │   ├── turn.py             # ActiveTurn streaming controller
│   │   ├── transport.py        # UDS, Stdio, HTTP/SSE, Mock transports
│   │   ├── types.py            # Dataclasses & enums
│   │   ├── errors.py           # Python exception classes
│   │   └── __init__.py         # Public exports
│   ├── tests/                  # unittest test suite
│   ├── examples/               # Python runnable demonstrations
│   └── README.md
└── README.md
```
