# M31A Desktop Architecture: Foundation & Runtime Integration (Phase 2)

## 1. Overview

M31A Phase 2 establishes **M31A** as a desktop-first autonomous engineering application built with **Tauri 2**, **React 18**, and **TypeScript**, backed directly by the authoritative Rust runtime (`m31a-runtime`).

The design strictly prevents the desktop frontend from becoming a secondary or divergent implementation of the agent runtime. The Rust backend retains complete authority over all security decisions, execution policies, tool dispatches, file modifications, model provider lifecycles, and session state.

```
+------------------------------------------------------------------------+
|                          React 18 / TypeScript                         |
|  Presentation • Component Trees • Streaming Render • Responsive Shell  |
+------------------------------------------------------------------------+
                                    ▲ │
            m31a://event (push)     │ │ typed invoke (pull)
                                    │ ▼
+------------------------------------------------------------------------+
|                          Tauri 2 IPC Boundary                          |
|         Commands • Event Bridge • CSP Enforcement • State Host         |
+------------------------------------------------------------------------+
                                    │
                                    ▼
+------------------------------------------------------------------------+
|                              Rust Desktop                              |
|           m31a-desktop: State Holder • Security Canonicalizer          |
+------------------------------------------------------------------------+
                                    │
                                    ▼
+------------------------------------------------------------------------+
|                      Authoritative m31a-runtime                        |
|   Agent Engine • Execution Policy • Storage • Trust • Elicitation      |
+------------------------------------------------------------------------+
```

---

## 2. Authority & Security Boundary

### 2.1 The Rust Authoritative Core
The frontend must **never** execute shell commands, alter files, or grant workspace permissions independently.

- **Workspace Path Canonicalization**: Handled in `m31a_desktop_lib::security::validate_and_canonicalize_workspace_path`. Paths are validated, checked for directory existence, and canonicalized after resolving symlinks before being admitted to the runtime.
- **Trust Evaluation**: All workspace operations query `m31a_trust::TrustStore`. Selecting a workspace never grants trust. A workspace requiring confirmation can be authorized only after a native operating-system confirmation dialog; an explicit `Denied` decision is never overridden by a session grant.
- **Session Isolation**: Every submitted turn resolves the session's persisted `cwd`, canonicalizes it, and verifies it matches the active workspace before execution.
- **Approval Gate**: Interactive approvals (`ApprovalRequested`) and user questions (`QuestionRequested`) originate from the runtime's elicitation and execution policy engines (`m31a-execpolicy`, `m31a-elicitation`). Frontend decisions are submitted as typed requests (`respond_approval`, `respond_question`) to Rust, which unblocks the waiting future.
- **Tool Dispatch & Agent Engine**: LLM streaming, reasoning parsing, tool orchestration, diff application, and git attribution are performed exclusively in `m31a-agent-engine` and `m31a-runtime`.

### 2.2 Tauri 2 Host & CSP
The desktop host configuration (`apps/desktop/src-tauri/tauri.conf.json`) enforces a strict Content Security Policy:
```text
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' asset: https: data:; connect-src 'self' ipc: http://ipc.localhost
```
Outbound frontend `fetch`/WebSocket connections are restricted by `connect-src`; external API communication is conducted through the Rust backend. The `img-src` directive permits HTTPS image loads.

---

## 3. Runtime Lifecycle & Ownership

### 3.1 Startup & Singleton Runtime
The desktop application initializes the authoritative `M31aRuntime` during Tauri application setup:
1. `init_runtime(Some(home_dir))` is invoked through `tauri::async_runtime::block_on` inside `tauri::Builder::setup`; setup waits synchronously for initialization to complete.
2. A single `M31aRuntime` instance is wrapped inside `DesktopState`.
3. `DesktopState` is registered as Tauri managed state via `app.manage(desktop_state)`.
4. The background event bridge (`start_event_bridge`) subscribes to `runtime.subscribe_events()` and forwards events to Tauri webviews.
5. If the broadcast receiver lags, the host emits `m31a://event-gap`; the frontend re-fetches authoritative pending approval and question snapshots through `get_pending_interactions`.

### 3.2 Orderly Teardown
When the desktop application window is closed or an exit request is received:
1. The Tauri event loop intercepts `RunEvent::ExitRequested`.
2. `lifecycle::teardown_desktop_application` sets `is_shutting_down = true`.
3. `runtime.shutdown().await` is invoked to cancel and drain active turns, stop managed processes, tool-owned resources, the watcher, MCP clients, and generate a `ShutdownReport`. It does not claim to flush arbitrary telemetry or storage transactions.

---

## 4. Frontend Architecture

The frontend is housed in `apps/desktop`:
- **`src/types/ipc.ts` & `src/types/events.ts`**: Mirror the Rust backend DTOs and `RuntimeEvent` variants with zero use of `any`.
- **`src/lib/ipc.ts`**: Type-safe async wrappers around Tauri's `invoke` API.
- **`src/lib/events.ts`**: Subscribes to the `m31a://event` channel with automatic cleanup upon unmount.
- **`src/stores/AppContext.tsx`**: Unified React Context connecting IPC commands and event streams into responsive state.
- **`src/features/`**:
  - `chat/`: Conversation timeline (`ChatTimeline`), message renderer (`MessageItem`), expandable thinking blocks, tool execution views (`ToolCallView`), syntax-highlighted code blocks (`CodeBlock`), and multiline prompt composer (`Composer`).
  - `sessions/`: Session drawer with active indicators and creation triggers (`SessionSidebar`).
  - `workspace/`: Directory path, trust status badge, model selection dropdown, and runtime health dot (`WorkspaceHeader`).
  - `approvals/`: Dedicated interactive cards for execution policy approvals (`ApprovalCard`) and user elicitation dialogs (`QuestionDialog`).
  - `settings/`: Runtime diagnostics, platform capabilities, and model context information (`SettingsModal`).
