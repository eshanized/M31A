# M31A Desktop IPC & Event Protocol Specification

## 1. Overview

The desktop host communicates with the React presentation layer through an explicitly typed Tauri 2 IPC interface and a push-based event bridge.

All requests pass through Tauri's invoke handler. All asynchronous notifications, token streaming, tool calls, and lifecycle events stream over the `m31a://event` channel.

---

## 2. IPC Commands

### 2.1 Application Lifecycle
- **`get_runtime_status`**
  - Input: None
  - Output: `RuntimeStatusDto`
    ```json
    {
      "state": "Ready",
      "active_model": "claude-3-5-sonnet",
      "active_workspace": "/path/to/project",
      "is_operational": true,
      "is_terminal": false,
      "active_session_id": "optional-uuid"
    }
    ```
- **`get_environment_info`**
  - Input: None
  - Output: `EnvironmentInfoDto`
    ```json
    {
      "app_version": "0.1.15",
      "target_platform": "linux",
      "target_arch": "x86_64",
      "features": ["desktop", "git", "sandbox"]
    }
    ```
- **`shutdown_runtime`**
  - Input: None
  - Output: `ShutdownReport`

### 2.2 Workspace Operations
- **`get_active_workspace`**
  - Output: `WorkspaceInfoDto` (`path`, `is_trusted`, `exists`)
- **`set_workspace`**
  - Input: `{ path: string }`
  - Output: `WorkspaceInfoDto`; selects but never authorizes a workspace
- **`authorize_workspace`**
  - Input: `{ path: string }`
  - Output: `WorkspaceInfoDto`; requires a native OS confirmation for workspaces that require confirmation, and never overrides an explicit Denied decision

### 2.3 Model Management
- **`list_available_models`**
  - Output: `ModelInfoDto[]` (`id`, `name`, `originator`, `context_window`, `supports_tools`, `supports_thinking`, `supports_vision`)
- **`get_active_model`**
  - Output: active model slug (`string`)
- **`activate_model`**
  - Input: `{ model: string }`
  - Output: activated model slug (`string`)

### 2.4 Session & Turn Execution
- **`list_sessions`**
  - Output: `SessionSummaryDto[]` (`id`, `title`, `model`, `cwd`, `created_at`, `updated_at`, `is_active`)
- **`create_session`**
  - Input: `{ title?: string }`
  - Output: `SessionSummaryDto`
- **`load_session`**
  - Input: `{ sessionId: string }`
  - Output: `SessionDetailsDto` including `workspace_is_trusted` and persisted user/assistant `messages`
- **`submit_turn`**
  - Input: `{ payload: SubmitTurnPayload }`
  - Output: `TurnHandleDto` (`thread_id`, `turn_id`)
- **`cancel_turn`**
  - Input: `{ threadId: string, turnId: string }`
  - Output: `void`
- **`steer_turn`**
  - Input: `{ threadId: string, turnId: string, message: string }`
  - Output: `void`

### 2.5 Pending Interaction Recovery
- **`get_pending_interactions`**
  - Output: current pending approval and question snapshots from the authoritative runtime
  - Called on frontend subscription setup and after an event-gap notification

### 2.6 Security Approvals & Questions
- **`respond_approval`**
  - Input: `{ requestId: string, decision: boolean, reason?: string }`
  - Output: `void`
- **`respond_question`**
  - Input: `{ questionId: string, answers: string[] }`
  - Output: `void`

---

## 3. Runtime Event Bridge (`m31a://event`)

The host emits `m31a://event-gap` with the number of skipped broadcast events if the receiver lags. The frontend must reconcile pending interactions using `get_pending_interactions` rather than assuming every approval/question event was delivered.

The event bridge broadcasts serde-tagged JSON payloads:
```typescript
type RuntimeEvent =
  | { type: 'initializing' }
  | { type: 'ready' }
  | { type: 'shutting_down' }
  | { type: 'shutdown_complete'; data: { report: Record<string, unknown> } }
  | { type: 'runtime_error'; data: { error: string } }
  | { type: 'thread_created'; data: { thread_id: string; title: string } }
  | { type: 'turn_submitted'; data: { thread_id: string; turn_id: string } }
  | { type: 'turn_admitted'; data: { thread_id: string; turn_id: string } }
  | { type: 'turn_completed'; data: { thread_id: string; turn_id: string; usage: TokenUsage } }
  | { type: 'turn_cancelled'; data: { thread_id: string; turn_id: string } }
  | { type: 'turn_failed'; data: { thread_id: string; turn_id: string; error: string } }
  | { type: 'reasoning_delta'; data: { thread_id: string; turn_id: string; delta: string } }
  | { type: 'content_delta'; data: { thread_id: string; turn_id: string; delta: string } }
  | { type: 'tool_started'; data: { thread_id: string; turn_id: string; call_id: string; tool_name: string; arguments: string } }
  | { type: 'tool_finished'; data: { thread_id: string; turn_id: string; call_id: string; tool_name: string; output: string; success: boolean } }
  | { type: 'approval_requested'; data: { thread_id: string; turn_id: string; request_id: string; command: string; reason: string } }
  | { type: 'question_requested'; data: { thread_id: string; turn_id: string; question_id: string; question: string; options: string[]; multi_select: boolean } }
  | { type: 'agent_spawned'; data: { parent_thread_id: string; child_thread_id: string; role: string } }
  | { type: 'agent_finished'; data: { parent_thread_id: string; child_thread_id: string; success: boolean } }
  | { type: 'workflow_step'; data: { workflow_id: string; step: number; total_steps: number; message: string } }
  | { type: 'workspace_changed'; data: { paths: string[] } }
  | { type: 'model_changed'; data: { old_model: string; new_model: string } }
  | { type: 'provider_status'; data: { status: string; message: string } };
```
