"""Active turn controller and event streaming for M31A Python SDK."""

import asyncio
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional
from .errors import TimeoutError
from .types import ApprovalDecision, ApprovalRequest, QuestionRequest

@dataclass
class TurnEvent:
    """Represents a discrete event emitted during turn execution."""
    type: str
    chunk: Optional[str] = None
    message: Optional[str] = None
    data: Optional[Dict[str, Any]] = None

class ActiveTurn:
    """Represents an active execution turn and exposes streaming handles."""

    def __init__(
        self,
        turn_id: str,
        thread_id: str,
        status: str,
        lease_id: Optional[str],
        bridge: Any,
    ):
        self.turn_id = turn_id
        self.thread_id = thread_id
        self.status = status
        self.lease_id = lease_id
        self._bridge = bridge

        self._content = ""
        self._reasoning = ""
        self._final_message = ""
        self._completed = False
        self._error: Optional[Exception] = None

        self._completed_event = asyncio.Event()
        self._event_queue: asyncio.Queue[TurnEvent] = asyncio.Queue()

        self._content_cbs: List[Callable[[str], None]] = []
        self._reasoning_cbs: List[Callable[[str], None]] = []
        self._tool_started_cbs: List[Callable[[Dict[str, Any]], None]] = []
        self._tool_finished_cbs: List[Callable[[Dict[str, Any]], None]] = []
        self._approval_callback: Optional[Callable[[ApprovalRequest], Any]] = None
        self._question_callback: Optional[Callable[[QuestionRequest], Any]] = None

    @property
    def is_completed(self) -> bool:
        return self._completed

    @property
    def content(self) -> str:
        return self._content

    @property
    def reasoning(self) -> str:
        return self._reasoning

    @property
    def final_message(self) -> str:
        return self._final_message or self._content

    def on_content(self, cb: Callable[[str], None]):
        self._content_cbs.append(cb)

    def on_reasoning(self, cb: Callable[[str], None]):
        self._reasoning_cbs.append(cb)

    def on_tool_started(self, cb: Callable[[Dict[str, Any]], None]):
        self._tool_started_cbs.append(cb)

    def on_tool_finished(self, cb: Callable[[Dict[str, Any]], None]):
        self._tool_finished_cbs.append(cb)

    def set_approval_handler(self, handler: Callable[[ApprovalRequest], Any]):
        self._approval_callback = handler

    def set_question_handler(self, handler: Callable[[QuestionRequest], Any]):
        self._question_callback = handler

    async def steer(self, message: str) -> None:
        """Inject steering instructions into this actively running turn."""
        await self._bridge.steer_turn(message, turn_id=self.turn_id, thread_id=self.thread_id)

    async def interrupt(self) -> None:
        """Cancel and interrupt this turn."""
        await self._bridge.interrupt_turn(turn_id=self.turn_id, thread_id=self.thread_id)

    async def result(self, timeout_sec: float = 120.0) -> str:
        """Wait for turn completion and return the final output message."""
        if self._completed:
            if self._error:
                raise self._error
            return self.final_message

        try:
            await asyncio.wait_for(self._completed_event.wait(), timeout=timeout_sec)
        except asyncio.TimeoutError as exc:
            raise TimeoutError(f"Turn '{self.turn_id}' timed out after {timeout_sec}s", timeout_sec) from exc

        if self._error:
            raise self._error
        return self.final_message

    def dispatch_server_event(self, method: str, params: Dict[str, Any]):
        """Dispatches server events to registered callbacks and async queue."""
        if self._completed and method != "notification/turn_finished":
            return

        if method == "notification/turn_content_chunk":
            chunk = params.get("chunk", "")
            self._content += chunk
            for cb in self._content_cbs:
                try:
                    cb(chunk)
                except Exception:
                    pass
            self._event_queue.put_nowait(TurnEvent(type="content", chunk=chunk))

        elif method == "notification/turn_reasoning_chunk":
            chunk = params.get("chunk", "")
            self._reasoning += chunk
            for cb in self._reasoning_cbs:
                try:
                    cb(chunk)
                except Exception:
                    pass
            self._event_queue.put_nowait(TurnEvent(type="reasoning", chunk=chunk))

        elif method == "notification/tool_started":
            for cb in self._tool_started_cbs:
                try:
                    cb(params)
                except Exception:
                    pass
            self._event_queue.put_nowait(TurnEvent(type="tool_started", data=params))

        elif method == "notification/tool_finished":
            for cb in self._tool_finished_cbs:
                try:
                    cb(params)
                except Exception:
                    pass
            self._event_queue.put_nowait(TurnEvent(type="tool_finished", data=params))

        elif method == "notification/approval_requested":
            req = ApprovalRequest(
                approval_id=params["approval_id"],
                turn_id=params["turn_id"],
                thread_id=params["thread_id"],
                tool=params.get("tool", ""),
                arguments=params.get("arguments", ""),
                reason=params.get("reason", ""),
                command=params.get("command"),
            )
            self._event_queue.put_nowait(TurnEvent(type="approval_requested", data=params))
            if self._approval_callback:
                async def handle_approval():
                    try:
                        res = self._approval_callback(req)
                        if asyncio.iscoroutine(res):
                            res = await res
                        if isinstance(res, ApprovalDecision):
                            await self._bridge.approve_turn(
                                req.approval_id,
                                decision=res.decision,
                                reason=res.reason,
                                command=res.command,
                                pattern=res.pattern,
                            )
                        elif isinstance(res, str):
                            await self._bridge.approve_turn(req.approval_id, decision=res)
                        elif res is True:
                            await self._bridge.approve_turn(req.approval_id, decision="allow")
                        elif res is False:
                            await self._bridge.deny_turn(req.approval_id, reason="Denied by automated callback")
                    except Exception:
                        pass
                asyncio.create_task(handle_approval())

        elif method == "notification/question_requested":
            q_req = QuestionRequest(
                question_id=params["question_id"],
                turn_id=params["turn_id"],
                thread_id=params["thread_id"],
                prompt=params.get("prompt", ""),
                options=params.get("options", []),
                is_multi_select=params.get("is_multi_select", False),
            )
            self._event_queue.put_nowait(TurnEvent(type="question_requested", data=params))
            if self._question_callback:
                async def handle_question():
                    try:
                        res = self._question_callback(q_req)
                        if asyncio.iscoroutine(res):
                            res = await res
                        if isinstance(res, list):
                            await self._bridge.answer_question(q_req.question_id, res)
                        elif isinstance(res, str):
                            await self._bridge.answer_question(q_req.question_id, [res])
                    except Exception:
                        pass
                asyncio.create_task(handle_question())

        elif method == "notification/turn_message":
            self._final_message = params.get("message", self._content)

        elif method == "notification/turn_completed":
            self._completed = True
            msg = self._final_message or self._content
            self._event_queue.put_nowait(TurnEvent(type="completed", message=msg, data=params.get("usage")))
            self._completed_event.set()

        elif method == "notification/turn_failed":
            self._completed = True
            err_msg = params.get("error", "Turn failed")
            self._error = RuntimeError(err_msg)
            self._event_queue.put_nowait(TurnEvent(type="failed", message=err_msg))
            self._completed_event.set()

        elif method == "notification/turn_interrupted":
            self._completed = True
            self._event_queue.put_nowait(TurnEvent(type="interrupted"))
            self._completed_event.set()

    async def __aiter__(self):
        """Asynchronous iterator for streaming turn events."""
        while True:
            event = await self._event_queue.get()
            yield event
            if event.type in ("completed", "failed", "interrupted"):
                break
