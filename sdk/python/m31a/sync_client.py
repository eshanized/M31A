"""Synchronous client wrapper for M31A App Server.

Provides a synchronous, blocking interface suitable for CLI scripts,
data analysis pipelines, Jupyter notebooks, and non-async environments.
"""

import asyncio
import threading
from typing import Any, Callable, Dict, List, Optional
from .client import M31AClient
from .errors import M31AError
from .types import (
    AppConfig,
    BoardPost,
    CheckpointRecord,
    ClaimRecord,
    DiagnosticsResult,
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

class SyncActiveTurn:
    """Synchronous wrapper for an active turn."""

    def __init__(self, async_turn: Any, sync_runner: Callable[[Any], Any]):
        self._async_turn = async_turn
        self._run = sync_runner

    @property
    def turn_id(self) -> str:
        return self._async_turn.turn_id

    @property
    def thread_id(self) -> str:
        return self._async_turn.thread_id

    @property
    def status(self) -> str:
        return self._async_turn.status

    @property
    def content(self) -> str:
        return self._async_turn.content

    @property
    def reasoning(self) -> str:
        return self._async_turn.reasoning

    @property
    def final_message(self) -> str:
        return self._async_turn.final_message

    def steer(self, message: str) -> None:
        self._run(self._async_turn.steer(message))

    def interrupt(self) -> None:
        self._run(self._async_turn.interrupt())

    def result(self, timeout_sec: float = 120.0) -> str:
        return self._run(self._async_turn.result(timeout_sec=timeout_sec))


class SyncThreadHandle:
    """Synchronous wrapper for a thread session."""

    def __init__(self, thread_id: str, client: "SyncM31AClient"):
        self.id = thread_id
        self._client = client

    def run(
        self,
        prompt: str,
        on_content: Optional[Callable[[str], None]] = None,
        on_reasoning: Optional[Callable[[str], None]] = None,
        on_tool_started: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_tool_finished: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_approval: Optional[Callable[[Any], Any]] = None,
        on_question: Optional[Callable[[Any], Any]] = None,
    ) -> SyncActiveTurn:
        return self._client.run_turn(
            self.id,
            prompt,
            on_content=on_content,
            on_reasoning=on_reasoning,
            on_tool_started=on_tool_started,
            on_tool_finished=on_tool_finished,
            on_approval=on_approval,
            on_question=on_question,
        )

    def read(self) -> ThreadDetails:
        return self._client.read_thread(self.id)

    def history(self) -> List[ThreadItem]:
        details = self.read()
        return details.items

    def archive(self):
        self._client.archive_thread(self.id)

    def unarchive(self):
        self._client.unarchive_thread(self.id)

    def delete(self):
        self._client.delete_thread(self.id)

    def fork(self, title: str = "Forked Session") -> "SyncThreadHandle":
        res = self._client.fork_thread(self.id, title=title)
        return SyncThreadHandle(res.id, self._client)

    def resume(self) -> SessionMetadata:
        return self._client.resume_thread(self.id)

    def compact(self, keep_recent_turns: int = 6) -> Optional[str]:
        res = self._client.compact_session(self.id, keep_recent_turns=keep_recent_turns)
        return res.get("summary")

    def create_checkpoint(
        self,
        label: str,
        modified_files: Optional[List[str]] = None,
        turn_index: int = 1,
        git_commit: Optional[str] = None,
    ) -> CheckpointRecord:
        return self._client.create_checkpoint(
            self.id,
            label,
            modified_files=modified_files,
            turn_index=turn_index,
            git_commit=git_commit,
        )

    def list_checkpoints(self) -> List[CheckpointRecord]:
        return self._client.list_checkpoints(self.id)

    def restore_checkpoint(self, checkpoint_id: str, revert_conversation: bool = True) -> CheckpointRecord:
        return self._client.restore_checkpoint(checkpoint_id, revert_conversation=revert_conversation)

    def set_goal(self, objective: str, budget: Optional[int] = None) -> ThreadGoal:
        return self._client.set_goal(self.id, objective, budget=budget)

    def get_goal(self) -> Optional[ThreadGoal]:
        return self._client.get_goal(self.id)

    def clear_goal(self):
        self._client.clear_goal(self.id)

    def move_to_section(self, section_id: Optional[str]) -> bool:
        return self._client.move_thread_to_section(self.id, section_id=section_id)

    def enqueue_inbox(self, payload: str, kind: str = "prompt") -> InboxItem:
        return self._client.enqueue_inbox(self.id, payload, kind=kind)

    def dequeue_inbox(self) -> Optional[InboxItem]:
        return self._client.dequeue_inbox(self.id)

    def list_inbox(self) -> List[InboxItem]:
        return self._client.list_inbox(self.id)


class SyncM31AClient:
    """Synchronous M31A Client driving an internal async loop in a worker thread."""

    def __init__(self, **kwargs):
        self._client = M31AClient(**kwargs)
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(target=self._run_loop, daemon=True)
        self._thread.start()

    def _run_loop(self):
        asyncio.set_event_loop(self._loop)
        self._loop.run_forever()

    def _run(self, coro: Any) -> Any:
        future = asyncio.run_coroutine_threadsafe(coro, self._loop)
        return future.result()

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def connect(self) -> InitializeResult:
        return self._run(self._client.connect())

    def close(self):
        self._run(self._client.close())
        self._loop.call_soon_threadsafe(self._loop.stop)
        self._thread.join(timeout=2.0)

    def get_diagnostics(self) -> DiagnosticsResult:
        return self._run(self._client.get_diagnostics())

    def read_config(self) -> AppConfig:
        return self._run(self._client.read_config())

    def list_models(self) -> List[ModelInfo]:
        return self._run(self._client.list_models())

    def list_skills(self) -> List[SkillDefinition]:
        return self._run(self._client.list_skills())

    def list_instructions(self, cwd: Optional[str] = None) -> List[InstructionSource]:
        return self._run(self._client.list_instructions(cwd=cwd))

    def list_tools(self) -> List[ToolDefinition]:
        return self._run(self._client.list_tools())

    def exec_command(self, command: str, cwd: Optional[str] = None) -> str:
        return self._run(self._client.exec_command(command, cwd=cwd))

    def thread(self, thread_id: str) -> SyncThreadHandle:
        return SyncThreadHandle(thread_id, self)

    def create_thread(self, cwd: Optional[str] = None, model: Optional[str] = None) -> SyncThreadHandle:
        handle = self._run(self._client.create_thread(cwd=cwd, model=model))
        return SyncThreadHandle(handle.id, self)

    def read_thread(self, thread_id: str) -> ThreadDetails:
        return self._run(self._client.read_thread(thread_id))

    def list_threads(self, limit: int = 50) -> List[SessionMetadata]:
        return self._run(self._client.list_threads(limit=limit))

    def archive_thread(self, thread_id: str) -> Dict[str, Any]:
        return self._run(self._client.archive_thread(thread_id))

    def unarchive_thread(self, thread_id: str) -> Dict[str, Any]:
        return self._run(self._client.unarchive_thread(thread_id))

    def delete_thread(self, thread_id: str) -> Dict[str, Any]:
        return self._run(self._client.delete_thread(thread_id))

    def fork_thread(self, thread_id: str, title: str = "Forked Session") -> SessionMetadata:
        return self._run(self._client.fork_thread(thread_id, title=title))

    def resume_thread(self, thread_id: str) -> SessionMetadata:
        return self._run(self._client.resume_thread(thread_id))

    def compact_session(self, thread_id: str, keep_recent_turns: int = 6) -> Dict[str, Any]:
        return self._run(self._client.compact_session(thread_id, keep_recent_turns=keep_recent_turns))

    def create_thread_section(self, name: str, color: Optional[str] = None, icon: Optional[str] = None) -> ThreadSection:
        return self._run(self._client.create_thread_section(name, color=color, icon=icon))

    def list_thread_sections(self) -> List[ThreadSection]:
        return self._run(self._client.list_thread_sections())

    def move_thread_to_section(self, thread_id: str, section_id: Optional[str] = None) -> bool:
        return self._run(self._client.move_thread_to_section(thread_id, section_id=section_id))

    def start_turn(self, thread_id: str, prompt: str) -> StartTurnResult:
        return self._run(self._client.start_turn(thread_id, prompt))

    def run_turn(
        self,
        thread_id: str,
        prompt: str,
        on_content: Optional[Callable[[str], None]] = None,
        on_reasoning: Optional[Callable[[str], None]] = None,
        on_tool_started: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_tool_finished: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_approval: Optional[Callable[[Any], Any]] = None,
        on_question: Optional[Callable[[Any], Any]] = None,
    ) -> SyncActiveTurn:
        turn = self._run(
            self._client.run_turn(
                thread_id,
                prompt,
                on_content=on_content,
                on_reasoning=on_reasoning,
                on_tool_started=on_tool_started,
                on_tool_finished=on_tool_finished,
                on_approval=on_approval,
                on_question=on_question,
            )
        )
        return SyncActiveTurn(turn, self._run)

    def steer_turn(self, message: str, turn_id: Optional[str] = None, thread_id: Optional[str] = None) -> Dict[str, Any]:
        return self._run(self._client.steer_turn(message, turn_id=turn_id, thread_id=thread_id))

    def interrupt_turn(self, turn_id: Optional[str] = None, thread_id: Optional[str] = None) -> Dict[str, Any]:
        return self._run(self._client.interrupt_turn(turn_id=turn_id, thread_id=thread_id))

    def approve_turn(self, approval_id: str, decision: str = "allow", reason: Optional[str] = None) -> Dict[str, Any]:
        return self._run(self._client.approve_turn(approval_id, decision=decision, reason=reason))

    def deny_turn(self, approval_id: str, reason: str = "Denied by client") -> Dict[str, Any]:
        return self._run(self._client.deny_turn(approval_id, reason=reason))

    def answer_question(self, question_id: str, answers: List[str]) -> Dict[str, Any]:
        return self._run(self._client.answer_question(question_id, answers))

    def create_checkpoint(
        self,
        thread_id: str,
        label: str,
        modified_files: Optional[List[str]] = None,
        turn_index: int = 1,
        git_commit: Optional[str] = None,
    ) -> CheckpointRecord:
        return self._run(
            self._client.create_checkpoint(
                thread_id,
                label,
                modified_files=modified_files,
                turn_index=turn_index,
                git_commit=git_commit,
            )
        )

    def list_checkpoints(self, thread_id: str) -> List[CheckpointRecord]:
        return self._run(self._client.list_checkpoints(thread_id))

    def restore_checkpoint(self, checkpoint_id: str, revert_conversation: bool = True) -> CheckpointRecord:
        return self._run(self._client.restore_checkpoint(checkpoint_id, revert_conversation=revert_conversation))

    def list_locks(self) -> List[LockRecord]:
        return self._run(self._client.list_locks())

    def acquire_lock(self, path: str, mode: str = "read", holder: str = "python-sdk", timeout_ms: int = 5000) -> Dict[str, Any]:
        return self._run(self._client.acquire_lock(path, mode=mode, holder=holder, timeout_ms=timeout_ms))

    def release_lock(self, token: str) -> bool:
        return self._run(self._client.release_lock(token))

    def post_to_board(
        self,
        body: str,
        channel: str = "general",
        author: str = "agent",
        payload: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._run(self._client.post_to_board(body, channel=channel, author=author, payload=payload))

    def list_board_posts(self, channel: str = "general", limit: int = 50) -> List[BoardPost]:
        return self._run(self._client.list_board_posts(channel=channel, limit=limit))

    def set_goal(self, thread_id: str, objective: str, budget: Optional[int] = None) -> ThreadGoal:
        return self._run(self._client.set_goal(thread_id, objective, budget=budget))

    def get_goal(self, thread_id: str) -> Optional[ThreadGoal]:
        return self._run(self._client.get_goal(thread_id))

    def clear_goal(self, thread_id: str):
        self._run(self._client.clear_goal(thread_id))

    def enqueue_inbox(self, thread_id: str, payload: str, kind: str = "prompt") -> InboxItem:
        return self._run(self._client.enqueue_inbox(thread_id, payload, kind=kind))

    def dequeue_inbox(self, thread_id: str) -> Optional[InboxItem]:
        return self._run(self._client.dequeue_inbox(thread_id))

    def list_inbox(self, thread_id: str) -> List[InboxItem]:
        return self._run(self._client.list_inbox(thread_id))
