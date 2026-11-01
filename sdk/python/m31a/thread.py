"""ThreadHandle abstraction for M31A Python SDK."""

from typing import Any, AsyncIterator, Callable, Dict, List, Optional
from .types import (
    CheckpointRecord,
    ClaimRecord,
    InboxItem,
    SessionMetadata,
    ThreadDetails,
    ThreadGoal,
    ThreadItem,
)
from .turn import ActiveTurn, TurnEvent

class ThreadHandle:
    """Represents a bound agent conversation session thread."""

    def __init__(self, thread_id: str, client: Any):
        self.id = thread_id
        self._client = client

    async def run(
        self,
        prompt: str,
        on_content: Optional[Callable[[str], None]] = None,
        on_reasoning: Optional[Callable[[str], None]] = None,
        on_tool_started: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_tool_finished: Optional[Callable[[Dict[str, Any]], None]] = None,
        on_approval: Optional[Callable[[Any], Any]] = None,
        on_question: Optional[Callable[[Any], Any]] = None,
    ) -> ActiveTurn:
        """Admit and execute an autonomous turn on this thread."""
        return await self._client.run_turn(
            self.id,
            prompt,
            on_content=on_content,
            on_reasoning=on_reasoning,
            on_tool_started=on_tool_started,
            on_tool_finished=on_tool_finished,
            on_approval=on_approval,
            on_question=on_question,
        )

    async def stream(self, prompt: str) -> AsyncIterator[TurnEvent]:
        """Stream turn events as an asynchronous generator."""
        turn = await self.run(prompt)
        async for event in turn:
            yield event

    async def read(self) -> ThreadDetails:
        """Retrieve thread metadata and full message history."""
        return await self._client.read_thread(self.id)

    async def history(self) -> List[ThreadItem]:
        """Retrieve historical message items for this thread."""
        details = await self.read()
        return details.items

    async def archive(self):
        """Archive this thread."""
        await self._client.archive_thread(self.id)

    async def unarchive(self):
        """Unarchive this thread."""
        await self._client.unarchive_thread(self.id)

    async def delete(self):
        """Permanently delete this thread."""
        await self._client.delete_thread(self.id)

    async def fork(self, title: str = "Forked Session") -> "ThreadHandle":
        """Fork this thread into a new branch."""
        forked = await self._client.fork_thread(self.id, title=title)
        return ThreadHandle(forked.id, self._client)

    async def resume(self) -> SessionMetadata:
        """Resume or touch this thread."""
        return await self._client.resume_thread(self.id)

    async def compact(self, keep_recent_turns: int = 6) -> Optional[str]:
        """Compact message history to compress token context."""
        res = await self._client.compact_session(self.id, keep_recent_turns=keep_recent_turns)
        return res.get("summary")

    async def create_checkpoint(
        self,
        label: str,
        modified_files: Optional[List[str]] = None,
        turn_index: int = 1,
        git_commit: Optional[str] = None,
    ) -> CheckpointRecord:
        """Create a workspace and session snapshot checkpoint."""
        return await self._client.create_checkpoint(
            self.id,
            label,
            modified_files=modified_files or [],
            turn_index=turn_index,
            git_commit=git_commit,
        )

    async def list_checkpoints(self) -> List[CheckpointRecord]:
        """List all checkpoints recorded for this thread."""
        return await self._client.list_checkpoints(self.id)

    async def restore_checkpoint(self, checkpoint_id: str, revert_conversation: bool = True) -> CheckpointRecord:
        """Restore this thread and workspace to a prior checkpoint."""
        return await self._client.restore_checkpoint(checkpoint_id, revert_conversation=revert_conversation)

    async def set_goal(self, objective: str, budget: Optional[int] = None) -> ThreadGoal:
        """Define a mission objective and optional token budget for this thread."""
        return await self._client.set_goal(self.id, objective, budget=budget)

    async def get_goal(self) -> Optional[ThreadGoal]:
        """Retrieve current goal status for this thread."""
        return await self._client.get_goal(self.id)

    async def clear_goal(self):
        """Clear active goal."""
        await self._client.clear_goal(self.id)

    async def update_goal(self, status: str):
        """Update active goal status."""
        await self._client.update_goal(self.id, status)

    async def move_to_section(self, section_id: Optional[str]) -> bool:
        """Move this thread into an organizational section."""
        return await self._client.move_thread_to_section(self.id, section_id=section_id)

    async def enqueue_inbox(self, payload: str, kind: str = "prompt") -> InboxItem:
        """Add deferred work instruction to this thread's inbox."""
        return await self._client.enqueue_inbox(self.id, payload, kind=kind)

    async def dequeue_inbox(self) -> Optional[InboxItem]:
        """Pop the next work item from this thread's inbox."""
        return await self._client.dequeue_inbox(self.id)

    async def list_inbox(self) -> List[InboxItem]:
        """List all items queued in this thread's inbox."""
        return await self._client.list_inbox(self.id)

    async def get_active_claim(self) -> Optional[ClaimRecord]:
        """Query any active worker or agent lease claim on this thread."""
        return await self._client.get_active_claim(self.id)
