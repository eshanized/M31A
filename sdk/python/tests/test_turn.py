"""Tests for ActiveTurn streaming and callbacks."""

import asyncio
import unittest
from m31a.turn import ActiveTurn

class FakeBridge:
    def __init__(self):
        self.steered = []
        self.interrupted = False
        self.approved = []
        self.answered = []

    async def steer_turn(self, message: str, turn_id=None, thread_id=None):
        self.steered.append(message)

    async def interrupt_turn(self, turn_id=None, thread_id=None):
        self.interrupted = True

    async def approve_turn(self, approval_id: str, **kwargs):
        self.approved.append((approval_id, kwargs))

    async def deny_turn(self, approval_id: str, reason="Denied"):
        self.approved.append((approval_id, {"decision": "deny", "reason": reason}))

    async def answer_question(self, question_id: str, answers=None):
        self.answered.append((question_id, answers))

class TestTurn(unittest.IsolatedAsyncioTestCase):
    async def test_turn_content_accumulation_and_result(self):
        bridge = FakeBridge()
        turn = ActiveTurn("turn_1", "thread_1", "started", "lease_1", bridge)

        received_chunks = []
        turn.on_content(lambda c: received_chunks.append(c))

        turn.dispatch_server_event("notification/turn_content_chunk", {"turn_id": "turn_1", "chunk": "Python "})
        turn.dispatch_server_event("notification/turn_content_chunk", {"turn_id": "turn_1", "chunk": "SDK!"})
        turn.dispatch_server_event("notification/turn_completed", {"turn_id": "turn_1", "thread_id": "thread_1"})

        self.assertEqual(received_chunks, ["Python ", "SDK!"])
        self.assertEqual(turn.content, "Python SDK!")
        self.assertEqual(await turn.result(), "Python SDK!")

        await turn.steer("Add typing annotations")
        self.assertEqual(bridge.steered, ["Add typing annotations"])

    async def test_turn_async_iteration(self):
        bridge = FakeBridge()
        turn = ActiveTurn("turn_2", "thread_1", "started", None, bridge)

        async def emit_events():
            await asyncio.sleep(0.01)
            turn.dispatch_server_event("notification/turn_reasoning_chunk", {"turn_id": "turn_2", "chunk": "Plan step"})
            turn.dispatch_server_event("notification/turn_content_chunk", {"turn_id": "turn_2", "chunk": "Execute step"})
            turn.dispatch_server_event("notification/turn_completed", {"turn_id": "turn_2", "thread_id": "thread_1"})

        asyncio.create_task(emit_events())

        events = []
        async for ev in turn:
            events.append(ev)

        self.assertEqual(len(events), 3)
        self.assertEqual(events[0].type, "reasoning")
        self.assertEqual(events[1].type, "content")
        self.assertEqual(events[2].type, "completed")

    async def test_turn_automated_approval_and_question_callbacks(self):
        bridge = FakeBridge()
        turn = ActiveTurn("turn_3", "thread_1", "started", None, bridge)

        turn.set_approval_handler(lambda req: "allow")
        turn.set_question_handler(lambda q: ["PostgreSQL"])

        turn.dispatch_server_event(
            "notification/approval_requested",
            {
                "approval_id": "appr_99",
                "turn_id": "turn_3",
                "thread_id": "thread_1",
                "tool": "exec_command",
                "arguments": '{"command": "cargo test"}',
                "reason": "Security review",
            },
        )

        turn.dispatch_server_event(
            "notification/question_requested",
            {
                "question_id": "quest_42",
                "turn_id": "turn_3",
                "thread_id": "thread_1",
                "prompt": "Choose database",
                "options": ["MySQL", "PostgreSQL"],
                "is_multi_select": False,
            },
        )

        await asyncio.sleep(0.05)

        self.assertEqual(len(bridge.approved), 1)
        self.assertEqual(bridge.approved[0][0], "appr_99")
        self.assertEqual(len(bridge.answered), 1)
        self.assertEqual(bridge.answered[0], ("quest_42", ["PostgreSQL"]))

if __name__ == "__main__":
    unittest.main()
