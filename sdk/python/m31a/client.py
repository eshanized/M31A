"""M31A App Server Python Client.

Provides an asynchronous client interface for communicating with the M31A daemon,
managing sessions, running turns with real-time streaming notifications,
handling approvals and questions, and collaborating via distributed message boards.
"""

import os
from typing import Any, Callable, Dict, List, Optional
from .errors import M31AError, TurnRejectedError
from .thread import ThreadHandle
from .transport import HttpTransport, StdioTransport, Transport, UdsTransport
from .turn import ActiveTurn
from .types import (
    AppConfig,
    BoardPost,
    CheckpointRecord,
    ClaimRecord,
    DiagnosticsResult,
    FinishTurnResult,
    InboxItem,
    InitializeResult,
    InstructionSource,
    LockRecord,
    ModelInfo,
    SessionMetadata,
    SkillDefinition,
    StartTurnResult,
    ThreadDetails,
    ThreadGoal,
    ThreadItem,
    ThreadSection,
    ToolDefinition,
)

class M31AClient:
    """High-level asynchronous client for M31A App Server daemon."""

    def __init__(
        self,
        transport: Optional[Transport] = None,
        socket_path: Optional[str] = None,
        url: Optional[str] = None,
        binary_path: Optional[str] = None,
        timeout_sec: float = 30.0,
    ):
        self.default_timeout_sec = timeout_sec
        self._next_id = 1
        self._active_turns: Dict[str, ActiveTurn] = {}

        if transport is not None:
            self.transport = transport
        elif url or os.environ.get("M31A_URL"):
            base_url = url or os.environ["M31A_URL"]
            self.transport = HttpTransport(base_url)
        elif socket_path or os.environ.get("M31A_SOCKET"):
            sock = socket_path or os.environ["M31A_SOCKET"]
            self.transport = UdsTransport(sock)
        elif binary_path:
            self.transport = StdioTransport(binary_path=binary_path)
        else:
            home = os.environ.get("HOME", ".")
            default_socket = os.path.join(home, ".m31a", "app-server-control", "app-server-control.sock")
            temp_socket = "/tmp/m31a-daemon.sock"

            if os.path.exists(temp_socket):
                self.transport = UdsTransport(temp_socket)
            else:
                self.transport = UdsTransport(default_socket)

        self.transport.on_notification("*", self._handle_notification)

    def _handle_notification(self, params: Dict[str, Any]):
        # Route notifications to active turns if matching turn_id or thread_id
        turn_id = params.get("turn_id")
        if turn_id and turn_id in self._active_turns:
            turn = self._active_turns[turn_id]
            # determine event method from params or context
            # Transport handlers can receive method directly or pass event
            pass

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()

    async def connect(self) -> InitializeResult:
        """Connect transport and perform initialize handshake."""
        await self.transport.connect()
        return await self.initialize()

    async def close(self):
        """Close connection and clean up resources."""
        await self.transport.close()

    async def request(
        self,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout_sec: Optional[float] = None,
    ) -> Any:
        """Send a JSON-RPC request and await response."""
        req_id = self._next_id
        self._next_id += 1
        eff_timeout = timeout_sec if timeout_sec is not None else self.default_timeout_sec
        return await self.transport.send_request(req_id, method, params, timeout_sec=eff_timeout)

    def on(self, event: str, handler: Callable[[Dict[str, Any]], None]):
        """Subscribe to streaming server notifications (e.g. 'notification/turn_content_chunk')."""
        self.transport.on_notification(event, handler)

    # --- Core Lifecycle & Discovery ---

    async def initialize(
        self,
        client_name: str = "m31a-python-sdk",
        client_version: str = "0.1.13",
    ) -> InitializeResult:
        res = await self.request("initialize", {"client_name": client_name, "client_version": client_version})
        return InitializeResult(
            server_name=res["server_name"],
            version=res["version"],
            protocol_version=res["protocol_version"],
            capabilities=res.get("capabilities", {}),
        )

    async def get_diagnostics(self) -> DiagnosticsResult:
        res = await self.request("server/diagnostics")
        return DiagnosticsResult(
            status=res["status"],
            api_key_configured=res["api_key_configured"],
            bubblewrap_available=res["bubblewrap_available"],
            active_model=res["active_model"],
            sandbox_policy=res["sandbox_policy"],
            approval_policy=res["approval_policy"],
        )

    async def read_config(self) -> AppConfig:
        res = await self.request("config/read")
        return AppConfig(
            model=res.get("model", ""),
            api_base=res.get("api_base", ""),
            sandbox=str(res.get("sandbox", "")),
            approval=str(res.get("approval", "")),
            api_key=res.get("api_key"),
            routing_enabled=res.get("routing_enabled"),
            tool_output_token_limit=res.get("tool_output_token_limit"),
            features=res.get("features", {}),
            raw=res,
        )

    async def list_models(self) -> List[ModelInfo]:
        res = await self.request("models/list")
        models = []
        for m in res:
            models.append(
                ModelInfo(
                    slug=m["slug"],
                    display_name=m["display_name"],
                    description=m.get("description", ""),
                    context_window=m.get("context_window", 0),
                    max_output_tokens=m.get("max_output_tokens", 0),
                    supports_tools=m.get("supports_tools", False),
                    supports_thinking=m.get("supports_thinking", False),
                    supports_vision=m.get("supports_vision", False),
                    originator=m.get("originator", "nvidia"),
                )
            )
        return models

    async def list_skills(self) -> List[SkillDefinition]:
        res = await self.request("skills/list")
        skills = []
        for s in res:
            skills.append(
                SkillDefinition(
                    name=s["name"],
                    description=s["description"],
                    source_path=s.get("source_path"),
                    instructions=s.get("instructions"),
                )
            )
        return skills

    async def list_instructions(self, cwd: Optional[str] = None) -> List[InstructionSource]:
        params = {"cwd": cwd} if cwd else {}
        res = await self.request("instructions/list", params)
        instructions = []
        for i in res:
            instructions.append(
                InstructionSource(
                    source=i.get("source", ""),
                    path=i.get("path"),
                    content_preview=i.get("content_preview"),
                )
            )
        return instructions

    async def list_tools(self) -> List[ToolDefinition]:
        res = await self.request("tools/list")
        tools = []
        for t in res:
            tools.append(ToolDefinition(type=t.get("type", "function"), function=t.get("function", {})))
        return tools

    async def exec_command(self, command: str, cwd: Optional[str] = None) -> str:
        params: Dict[str, Any] = {"command": command}
        if cwd:
            params["cwd"] = cwd
        res = await self.request("command/exec", params)
        return res.get("output", "")

    # --- Thread Management ---

    def thread(self, thread_id: str) -> ThreadHandle:
        """Get a bound ThreadHandle instance for an existing thread ID."""
        return ThreadHandle(thread_id, self)

    async def create_thread(self, cwd: Optional[str] = None, model: Optional[str] = None) -> ThreadHandle:
        """Create a new session thread and return its ThreadHandle."""
        params: Dict[str, Any] = {}
        if cwd:
            params["cwd"] = cwd
        if model:
            params["model"] = model
        res = await self.request("thread/create", params)
        return ThreadHandle(res["id"], self)

    async def read_thread(self, thread_id: str) -> ThreadDetails:
        res = await self.request("thread/read", {"thread_id": thread_id})
        t_meta = res["thread"]
        meta = SessionMetadata(
            id=t_meta["id"],
            title=t_meta["title"],
            model=t_meta["model"],
            cwd=t_meta["cwd"],
            created_at=t_meta["created_at"],
            updated_at=t_meta["updated_at"],
            extra=t_meta.get("extra"),
        )
        items = []
        for item in res.get("items", []):
            items.append(
                ThreadItem(
                    id=item.get("id", ""),
                    thread_id=thread_id,
                    role=item.get("role", item.get("item_type", "")),
                    content=item.get("content", ""),
                    created_at=item.get("created_at", ""),
                    turn_id=item.get("turn_id"),
                )
            )
        return ThreadDetails(thread=meta, items=items)

    async def list_threads(self, limit: int = 50) -> List[SessionMetadata]:
        res = await self.request("thread/list", {"limit": limit})
        threads = []
        for t in res:
            threads.append(
                SessionMetadata(
                    id=t["id"],
                    title=t["title"],
                    model=t["model"],
                    cwd=t["cwd"],
                    created_at=t["created_at"],
                    updated_at=t["updated_at"],
                    extra=t.get("extra"),
                )
            )
        return threads

    async def archive_thread(self, thread_id: str) -> Dict[str, Any]:
        return await self.request("thread/archive", {"thread_id": thread_id})

    async def unarchive_thread(self, thread_id: str) -> Dict[str, Any]:
        return await self.request("thread/unarchive", {"thread_id": thread_id})

    async def delete_thread(self, thread_id: str) -> Dict[str, Any]:
        return await self.request("thread/delete", {"thread_id": thread_id})

    async def fork_thread(self, thread_id: str, title: str = "Forked Session") -> SessionMetadata:
        res = await self.request("thread/fork", {"thread_id": thread_id, "title": title})
        return SessionMetadata(
            id=res["id"],
            title=res["title"],
            model=res["model"],
            cwd=res["cwd"],
            created_at=res["created_at"],
            updated_at=res["updated_at"],
            extra=res.get("extra"),
        )

    async def resume_thread(self, thread_id: str) -> SessionMetadata:
        res = await self.request("thread/resume", {"thread_id": thread_id})
        return SessionMetadata(
            id=res["id"],
            title=res["title"],
            model=res["model"],
            cwd=res["cwd"],
            created_at=res["created_at"],
            updated_at=res["updated_at"],
            extra=res.get("extra"),
        )

    async def compact_session(self, thread_id: str, keep_recent_turns: int = 6) -> Dict[str, Any]:
        return await self.request("session/compact", {"thread_id": thread_id, "keep_recent_turns": keep_recent_turns})

    # --- Thread Sections ---

    async def create_thread_section(
        self,
        name: str,
        color: Optional[str] = None,
        icon: Optional[str] = None,
    ) -> ThreadSection:
        params: Dict[str, Any] = {"name": name}
        if color:
            params["color"] = color
        if icon:
            params["icon"] = icon
        res = await self.request("thread/section/create", params)
        return ThreadSection(
            id=res["id"],
            name=res["name"],
            color=res.get("color"),
            icon=res.get("icon"),
            created_at=res.get("created_at", ""),
            updated_at=res.get("updated_at", ""),
        )

    async def list_thread_sections(self) -> List[ThreadSection]:
        res = await self.request("thread/section/list")
        sections = []
        for s in res:
            sections.append(
                ThreadSection(
                    id=s["id"],
                    name=s["name"],
                    color=s.get("color"),
                    icon=s.get("icon"),
                    created_at=s.get("created_at", ""),
                    updated_at=s.get("updated_at", ""),
                )
            )
        return sections

    async def rename_thread_section(
        self,
        section_id: str,
        name: str,
        color: Optional[str] = None,
        icon: Optional[str] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"section_id": section_id, "name": name}
        if color:
            params["color"] = color
        if icon:
            params["icon"] = icon
        return await self.request("thread/section/rename", params)

    async def delete_thread_section(self, section_id: str) -> bool:
        res = await self.request("thread/section/delete", {"section_id": section_id})
        return res.get("deleted", False)

    async def move_thread_to_section(self, thread_id: str, section_id: Optional[str] = None) -> bool:
        params: Dict[str, Any] = {"thread_id": thread_id}
        if section_id:
            params["section_id"] = section_id
        res = await self.request("thread/section/move", params)
        return res.get("moved", False)

    async def list_section_threads(self, section_id: Optional[str] = None, limit: int = 50) -> List[SessionMetadata]:
        params: Dict[str, Any] = {"limit": limit}
        if section_id:
            params["section_id"] = section_id
        res = await self.request("thread/section/threads", params)
        threads = []
        for t in res:
            threads.append(
                SessionMetadata(
                    id=t["id"],
                    title=t["title"],
                    model=t["model"],
                    cwd=t["cwd"],
                    created_at=t["created_at"],
                    updated_at=t["updated_at"],
                    extra=t.get("extra"),
                )
            )
        return threads

    # --- Turn Execution & Admissions ---

    async def start_turn(self, thread_id: str, prompt: str) -> StartTurnResult:
        res = await self.request("turn/start", {"thread_id": thread_id, "prompt": prompt})
        status = res.get("status", "started")
        if status == "rejected":
            raise TurnRejectedError(res.get("reason", "Admission rejected"))

        return StartTurnResult(
            status=status,
            thread_id=thread_id,
            turn_id=res.get("turn_id"),
            lease_id=res.get("lease_id"),
            active_lease_id=res.get("active_lease_id"),
            queue_position=res.get("queue_position"),
            estimated_wait_ms=res.get("estimated_wait_ms"),
        )

    async def finish_turn(self, thread_id: str, lease_id: str) -> FinishTurnResult:
        res = await self.request("turn/finish", {"thread_id": thread_id, "lease_id": lease_id})
        return FinishTurnResult(
            thread_id=thread_id,
            status=res.get("status", "finished"),
            has_next_queued=res.get("has_next_queued", False),
        )

    async def run_turn(
        self,
        thread_id: str,
        prompt: str,
        on_content: Optional[Callable[[str], None]] = None,
        on_reasoning: Optional[Callable[[str], None]] = None,
        on_tool_started: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_tool_finished: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_approval: Optional[Callable[[Any], Any]] = None,
        on_question: Optional[Callable[[Any], Any]] = None,
    ) -> ActiveTurn:
        """High-level helper to admit turn, attach streaming listeners, and return ActiveTurn."""
        start_res = await self.start_turn(thread_id, prompt)
        turn_id = start_res.turn_id or f"turn_{id(prompt)}"
        turn = ActiveTurn(turn_id, thread_id, start_res.status, start_res.lease_id, self)

        if on_content:
            turn.on_content(on_content)
        if on_reasoning:
            turn.on_reasoning(on_reasoning)
        if on_tool_started:
            turn.on_tool_started(on_tool_started)
        if on_tool_finished:
            turn.on_tool_finished(on_tool_finished)
        if on_approval:
            turn.set_approval_handler(on_approval)
        if on_question:
            turn.set_question_handler(on_question)

        self._active_turns[turn_id] = turn

        # Hook notification listener
        def turn_notif_listener(method: str, params: Dict[str, Any]):
            if params.get("turn_id") == turn_id or (
                params.get("thread_id") == thread_id and not params.get("turn_id")
            ):
                turn.dispatch_server_event(method, params)
                if method in ("notification/turn_completed", "notification/turn_failed", "notification/turn_interrupted"):
                    self._active_turns.pop(turn_id, None)

        for event_name in [
            "notification/turn_content_chunk",
            "notification/turn_reasoning_chunk",
            "notification/tool_started",
            "notification/tool_finished",
            "notification/approval_requested",
            "notification/question_requested",
            "notification/turn_message",
            "notification/turn_completed",
            "notification/turn_failed",
            "notification/turn_interrupted",
        ]:
            self.on(event_name, lambda p, en=event_name: turn_notif_listener(en, p))

        return turn

    async def steer_turn(
        self,
        message: str,
        turn_id: Optional[str] = None,
        thread_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"message": message}
        if turn_id:
            params["turn_id"] = turn_id
        if thread_id:
            params["thread_id"] = thread_id
        return await self.request("turn/steer", params)

    async def interrupt_turn(
        self,
        turn_id: Optional[str] = None,
        thread_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {}
        if turn_id:
            params["turn_id"] = turn_id
        if thread_id:
            params["thread_id"] = thread_id
        return await self.request("turn/interrupt", params)

    async def approve_turn(
        self,
        approval_id: str,
        decision: str = "allow",
        reason: Optional[str] = None,
        command: Optional[str] = None,
        pattern: Optional[str] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"approval_id": approval_id, "decision": decision}
        if reason:
            params["reason"] = reason
        if command:
            params["command"] = command
        if pattern:
            params["pattern"] = pattern
        return await self.request("turn/approve", params)

    async def deny_turn(self, approval_id: str, reason: str = "Denied by client") -> Dict[str, Any]:
        return await self.request("turn/deny", {"approval_id": approval_id, "reason": reason})

    async def answer_question(self, question_id: str, answers: List[str]) -> Dict[str, Any]:
        return await self.request("turn/answer", {"question_id": question_id, "answers": answers})

    # --- Checkpoints ---

    async def create_checkpoint(
        self,
        thread_id: str,
        label: str,
        modified_files: Optional[List[str]] = None,
        turn_index: int = 1,
        git_commit: Optional[str] = None,
    ) -> CheckpointRecord:
        params: Dict[str, Any] = {
            "thread_id": thread_id,
            "label": label,
            "turn_index": turn_index,
            "modified_files": modified_files or [],
        }
        if git_commit:
            params["git_commit"] = git_commit
        res = await self.request("checkpoint/create", params)
        return CheckpointRecord(
            id=res["id"],
            thread_id=res["thread_id"],
            label=res["label"],
            turn_index=res["turn_index"],
            modified_files=res.get("modified_files", []),
            created_at=res.get("created_at", ""),
            git_commit=res.get("git_commit"),
            metadata=res.get("metadata"),
        )

    async def list_checkpoints(self, thread_id: str) -> List[CheckpointRecord]:
        res = await self.request("checkpoint/list", {"thread_id": thread_id})
        records = []
        for r in res:
            records.append(
                CheckpointRecord(
                    id=r["id"],
                    thread_id=r["thread_id"],
                    label=r["label"],
                    turn_index=r["turn_index"],
                    modified_files=r.get("modified_files", []),
                    created_at=r.get("created_at", ""),
                    git_commit=r.get("git_commit"),
                    metadata=r.get("metadata"),
                )
            )
        return records

    async def restore_checkpoint(self, checkpoint_id: str, revert_conversation: bool = True) -> CheckpointRecord:
        res = await self.request(
            "checkpoint/restore",
            {"checkpoint_id": checkpoint_id, "revert_conversation": revert_conversation},
        )
        return CheckpointRecord(
            id=res["id"],
            thread_id=res["thread_id"],
            label=res.get("label", ""),
            turn_index=res.get("turn_index", 1),
            modified_files=res.get("modified_files", []),
            created_at=res.get("created_at", ""),
            git_commit=res.get("git_commit"),
        )

    # --- Path Concurrency Locks ---

    async def list_locks(self) -> List[LockRecord]:
        res = await self.request("locks/list")
        locks = []
        for lk in res:
            locks.append(
                LockRecord(
                    token=lk["token"],
                    path=lk["path"],
                    mode=lk["mode"],
                    holder=lk["holder"],
                    reason=lk.get("reason", ""),
                    expires_at_ms=lk.get("expires_at_ms", 0),
                )
            )
        return locks

    async def acquire_lock(
        self,
        path: str,
        mode: str = "read",
        holder: str = "python-sdk",
        timeout_ms: int = 5000,
    ) -> Dict[str, Any]:
        return await self.request(
            "locks/acquire",
            {"path": path, "mode": mode, "holder": holder, "timeout_ms": timeout_ms},
        )

    async def release_lock(self, token: str) -> bool:
        res = await self.request("locks/release", {"token": token})
        return res.get("released", False)

    # --- Multi-Agent Message Board ---

    async def post_to_board(
        self,
        body: str,
        channel: str = "general",
        author: str = "agent",
        payload: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {
            "channel": channel,
            "author": author,
            "body": body,
            "payload": payload or {},
        }
        return await self.request("board/post", params)

    async def list_board_posts(self, channel: str = "general", limit: int = 50) -> List[BoardPost]:
        res = await self.request("board/list", {"channel": channel, "limit": limit})
        posts = []
        for p in res:
            posts.append(
                BoardPost(
                    id=p["id"],
                    board=p.get("board", "default"),
                    channel=p.get("channel", channel),
                    author=p.get("author", "agent"),
                    body=p.get("body", ""),
                    created_at=p.get("created_at", ""),
                    payload=p.get("payload", {}),
                )
            )
        return posts

    # --- Mission Goals ---

    async def set_goal(self, thread_id: str, objective: str, budget: Optional[int] = None) -> ThreadGoal:
        params: Dict[str, Any] = {"thread_id": thread_id, "objective": objective}
        if budget is not None:
            params["budget"] = budget
        res = await self.request("goal/set", params)
        return ThreadGoal(
            id=res["id"],
            thread_id=res["thread_id"],
            objective=res["objective"],
            budget=res.get("budget"),
            status=res.get("status", "active"),
            created_at=res.get("created_at", ""),
            updated_at=res.get("updated_at", ""),
        )

    async def get_goal(self, thread_id: str) -> Optional[ThreadGoal]:
        res = await self.request("goal/get", {"thread_id": thread_id})
        if not res:
            return None
        return ThreadGoal(
            id=res["id"],
            thread_id=res["thread_id"],
            objective=res["objective"],
            budget=res.get("budget"),
            status=res.get("status", "active"),
            created_at=res.get("created_at", ""),
            updated_at=res.get("updated_at", ""),
        )

    async def clear_goal(self, thread_id: str) -> None:
        await self.request("goal/clear", {"thread_id": thread_id})

    async def update_goal(self, thread_id: str, status: str) -> None:
        await self.request("goal/update", {"thread_id": thread_id, "status": status})

    # --- Session Inboxes ---

    async def enqueue_inbox(self, thread_id: str, payload: str, kind: str = "prompt") -> InboxItem:
        res = await self.request("inbox/enqueue", {"thread_id": thread_id, "kind": kind, "payload": payload})
        return InboxItem(
            id=res["id"],
            thread_id=res["thread_id"],
            kind=res["kind"],
            payload=res["payload"],
            created_at=res.get("created_at", ""),
        )

    async def dequeue_inbox(self, thread_id: str) -> Optional[InboxItem]:
        res = await self.request("inbox/dequeue", {"thread_id": thread_id})
        if not res:
            return None
        return InboxItem(
            id=res["id"],
            thread_id=res["thread_id"],
            kind=res["kind"],
            payload=res["payload"],
            created_at=res.get("created_at", ""),
        )

    async def list_inbox(self, thread_id: str) -> List[InboxItem]:
        res = await self.request("inbox/list", {"thread_id": thread_id})
        items = []
        for i in res:
            items.append(
                InboxItem(
                    id=i["id"],
                    thread_id=i["thread_id"],
                    kind=i["kind"],
                    payload=i["payload"],
                    created_at=i.get("created_at", ""),
                )
            )
        return items

    # --- Active Claims ---

    async def get_active_claim(self, thread_id: str) -> Optional[ClaimRecord]:
        res = await self.request("claims/active", {"thread_id": thread_id})
        if not res:
            return None
        return ClaimRecord(
            id=res["id"],
            thread_id=res["thread_id"],
            agent_id=res["agent_id"],
            lease_expires_at=res.get("lease_expires_at", ""),
            status=res.get("status", "active"),
        )

    async def recover_claims(self) -> List[ClaimRecord]:
        res = await self.request("claims/recover")
        records = []
        for r in res:
            records.append(
                ClaimRecord(
                    id=r["id"],
                    thread_id=r["thread_id"],
                    agent_id=r["agent_id"],
                    lease_expires_at=r.get("lease_expires_at", ""),
                    status=r.get("status", "active"),
                )
            )
        return records
