# M31A TypeScript SDK (`@m31a/sdk`)

Official TypeScript client SDK for **M31A**, the high-performance autonomous agentic coding engine powered by NVIDIA NIM.

Provides strongly typed client interfaces, multi-transport communication (Unix Domain Sockets, Child Process Stdio, HTTP/SSE), turn streaming abstractions with DeepSeek R1 / Nemotron reasoning chunk introspection, automated tool approval & interactive question callbacks, snapshot checkpoints, path-based concurrency locks, and distributed message board pub-sub.

---

## Installation

```bash
npm install @m31a/sdk
```

---

## Features

- **Multi-Transport Support**:
  - `UdsTransport`: Ultra-fast local IPC over Unix Domain Sockets (`~/.m31a/app-server-control/app-server-control.sock` or custom path).
  - `StdioTransport`: Directly launches and orchestrates `m31a app-server --stdio` as a subprocess.
  - `HttpTransport`: HTTP POST JSON-RPC 2.0 with Server-Sent Events (SSE) streaming for web browsers and remote cloud workers.
  - `MockTransport`: In-memory transport for unit testing and offline simulation.
- **Full JSON-RPC 2.0 Protocol Coverage**: Complete parity with all 40+ M31A App Server RPC endpoints (`thread/*`, `turn/*`, `checkpoint/*`, `locks/*`, `board/*`, `goal/*`, `inbox/*`, `claims/*`, `skills/*`, `models/*`).
- **Ergonomic Turn Streaming & Reasoning Inspection**:
  - `turn.onContent(chunk => ...)`: Real-time assistant text stream.
  - `turn.onReasoning(chunk => ...)`: Real-time DeepSeek R1 / Nemotron thinking and reasoning stream.
  - `turn.onToolStarted(ev => ...)`, `turn.onToolFinished(ev => ...)`: Live tool invocation lifecycle tracking.
  - `for await (const event of turn)`: Async iterable streaming.
- **Interactive Human-in-the-Loop & Policy Callbacks**:
  - Automated or interactive tool approvals (`onApprovalRequested` -> `allow`, `allow_always`, `deny`, or edit command).
  - Interactive multi-choice questions (`onQuestionRequested` -> answer list).
- **Session & Thread Management**:
  - Branching and forking threads with full conversation memory.
  - Checkpoint creation and atomic rollback DAG traversal.
  - Thread Sections for organizing conversations by team or feature.
- **Multi-Agent Coordination**:
  - Cross-agent pub-sub message board (`postToBoard`, `listBoardPosts`).
  - Path-based concurrency locks (`acquireLock`, `releaseLock`).
  - Mission goal setting and token budgeting.

---

## Quickstart

### 1. Initialize Client & Create Session

```typescript
import { M31AClient } from '@m31a/sdk';

const client = new M31AClient();
await client.connect();

// Create a new autonomous coding session
const thread = await client.createThread({ cwd: process.cwd() });
console.log(`Started session: ${thread.id}`);
```

### 2. Stream an Autonomous Coding Turn

```typescript
const turn = await thread.run('Analyze crates/m31a-core and implement a benchmark test suite.', {
  onReasoning: (chunk) => {
    process.stderr.write(`\x1b[35m${chunk}\x1b[0m`); // Thinking path
  },
  onContent: (chunk) => {
    process.stdout.write(chunk); // Assistant response
  },
  onToolStarted: (ev) => {
    console.log(`\n[Tool Executing] ${ev.tool}(${ev.arguments})`);
  },
  onToolFinished: (ev) => {
    console.log(`[Tool Finished] success=${ev.success}`);
  },
  onApprovalRequested: async (req) => {
    console.log(`Safety check: ${req.tool} requires approval: ${req.reason}`);
    return { decision: 'allow' };
  },
});

const finalMessage = await turn.result();
console.log('\nTurn finished:\n', finalMessage);
```

### 3. Async Iterable Streaming

```typescript
for await (const event of turn) {
  if (event.type === 'content') {
    process.stdout.write(event.chunk);
  } else if (event.type === 'reasoning') {
    process.stderr.write(event.chunk);
  } else if (event.type === 'tool_started') {
    console.log(`Started tool: ${event.tool}`);
  }
}
```

### 4. Create Snapshot Checkpoints & Rollbacks

```typescript
// Snapshot session before destructive mutations
const checkpoint = await thread.createCheckpoint('Pre-Migration Snapshot', ['Cargo.toml', 'src/']);

// Restore anytime if needed
await thread.restoreCheckpoint(checkpoint.id, /* revertConversation = */ true);
```

### 5. Multi-Agent Message Board

```typescript
// Post coordination messages across autonomous agents
await client.postToBoard('Database migration finished on thread #3', 'general', 'migration-agent', {
  schema_version: 'v4',
});

// Read channel messages
const posts = await client.listBoardPosts('general', 20);
```

---

## Transports Configuration

```typescript
// 1. Unix Domain Socket (Default)
const udsClient = new M31AClient({
  socketPath: '/path/to/custom.sock',
});

// 2. HTTP / Server-Sent Events (SSE)
const httpClient = new M31AClient({
  url: 'http://127.0.0.1:4096',
});

// 3. Child Process Stdio Transport
import { StdioTransport } from '@m31a/sdk';

const stdioClient = new M31AClient({
  transport: new StdioTransport({
    binaryPath: 'm31a',
    args: ['app-server', 'stdio'],
    cwd: '/workspace',
  }),
});
```

---

## License

MIT
