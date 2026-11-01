"""Tests for M31AClient async client."""

import unittest
from m31a.client import M31AClient
from m31a.errors import JsonRpcError
from m31a.transport import MockTransport

class TestClient(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.transport = MockTransport()
        self.client = M31AClient(transport=self.transport, timeout_sec=1.0)

        self.transport.register_handler(
            "initialize",
            lambda p: {
                "server_name": "m31a-app-server",
                "version": "0.1.13",
                "protocol_version": "2.0",
                "capabilities": {"threads": True, "turns": True},
            },
        )

        self.transport.register_handler(
            "server/diagnostics",
            lambda p: {
                "status": "healthy",
                "api_key_configured": True,
                "bubblewrap_available": True,
                "active_model": "nvidia/nemotron-3.5-lightning-30b-a3b",
                "sandbox_policy": "WorkspaceWrite",
                "approval_policy": "OnRequest",
            },
        )

        self.transport.register_handler(
            "config/read",
            lambda p: {
                "model": "nvidia/nemotron-3.5-lightning-30b-a3b",
                "api_base": "https://integrate.api.nvidia.com/v1",
                "sandbox": "workspace-write",
                "approval": "on-request",
            },
        )

        self.transport.register_handler(
            "models/list",
            lambda p: [
                {
                    "slug": "nvidia/nemotron-3.5-lightning-30b-a3b",
                    "display_name": "Nemotron 3.5 Lightning",
                    "description": "Frontier workhorse",
                    "context_window": 131072,
                    "max_output_tokens": 8192,
                    "supports_tools": True,
                    "supports_thinking": True,
                    "supports_vision": False,
                    "originator": "nvidia",
                }
            ],
        )

        self.transport.register_handler(
            "thread/create",
            lambda p: {
                "id": "thread_abc",
                "title": "Python Session",
                "model": p.get("model", "nvidia/nemotron-3.5-lightning-30b-a3b"),
                "cwd": p.get("cwd", "."),
                "created_at": "2026-10-30T00:00:00Z",
                "updated_at": "2026-10-30T00:00:00Z",
            },
        )

        self.transport.register_handler(
            "thread/read",
            lambda p: {
                "thread": {
                    "id": p["thread_id"],
                    "title": "Python Session",
                    "model": "nvidia/nemotron-3.5-lightning-30b-a3b",
                    "cwd": ".",
                    "created_at": "2026-10-30T00:00:00Z",
                    "updated_at": "2026-10-30T00:00:00Z",
                },
                "items": [
                    {
                        "id": "item_1",
                        "thread_id": p["thread_id"],
                        "role": "user",
                        "content": "Initialize codebase",
                        "created_at": "2026-10-30T00:00:00Z",
                    }
                ],
            },
        )

        self.transport.register_handler(
            "checkpoint/create",
            lambda p: {
                "id": "cp_100",
                "thread_id": p["thread_id"],
                "label": p["label"],
                "turn_index": p["turn_index"],
                "modified_files": p.get("modified_files", []),
                "created_at": "2026-10-30T00:00:00Z",
            },
        )

        self.transport.register_handler(
            "locks/acquire",
            lambda p: {
                "token": "tok_123",
                "path": p["path"],
                "mode": p["mode"],
            },
        )

        self.transport.register_handler(
            "locks/release",
            lambda p: {"token": p["token"], "released": True},
        )

        self.transport.register_handler(
            "board/post",
            lambda p: {"post_id": "post_555"},
        )

        self.transport.register_handler(
            "board/list",
            lambda p: [
                {
                    "id": "post_555",
                    "board": "default",
                    "channel": p.get("channel", "general"),
                    "author": "tester",
                    "body": "System ready",
                    "created_at": "2026-10-30T00:00:00Z",
                }
            ],
        )

        self.transport.register_handler(
            "goal/set",
            lambda p: {
                "id": "goal_1",
                "thread_id": p["thread_id"],
                "objective": p["objective"],
                "budget": p.get("budget"),
                "status": "in_progress",
                "created_at": "2026-10-30T00:00:00Z",
                "updated_at": "2026-10-30T00:00:00Z",
            },
        )

        self.transport.register_handler(
            "turn/start",
            lambda p: {
                "turn_id": "turn_py_1",
                "thread_id": p["thread_id"],
                "lease_id": "lease_1",
                "status": "started",
            },
        )

        await self.client.connect()

    async def asyncTearDown(self):
        await self.client.close()

    async def test_diagnostics_and_models(self):
        diag = await self.client.get_diagnostics()
        self.assertEqual(diag.status, "healthy")
        self.assertTrue(diag.api_key_configured)

        models = await self.client.list_models()
        self.assertEqual(len(models), 1)
        self.assertEqual(models[0].slug, "nvidia/nemotron-3.5-lightning-30b-a3b")

    async def test_thread_management(self):
        thread = await self.client.create_thread(cwd="/workspace")
        self.assertEqual(thread.id, "thread_abc")

        details = await thread.read()
        self.assertEqual(details.thread.id, "thread_abc")
        self.assertEqual(len(details.items), 1)

        history = await thread.history()
        self.assertEqual(history[0].content, "Initialize codebase")

    async def test_checkpoints_locks_and_board(self):
        cp = await self.client.create_checkpoint("thread_abc", "Initial Setup", ["main.py"])
        self.assertEqual(cp.id, "cp_100")
        self.assertEqual(cp.label, "Initial Setup")

        lock = await self.client.acquire_lock("/tmp/data", mode="write")
        self.assertEqual(lock["token"], "tok_123")

        released = await self.client.release_lock(lock["token"])
        self.assertTrue(released)

        post = await self.client.post_to_board("System ready", channel="alerts", author="tester")
        self.assertEqual(post["post_id"], "post_555")

        posts = await self.client.list_board_posts("alerts")
        self.assertEqual(len(posts), 1)
        self.assertEqual(posts[0].body, "System ready")

    async def test_streaming_turn(self):
        chunks = []
        turn = await self.client.run_turn(
            "thread_abc",
            "Refactor network module",
            on_content=lambda c: chunks.append(c),
        )

        self.assertEqual(turn.turn_id, "turn_py_1")

        # Simulate events
        turn.dispatch_server_event("notification/turn_content_chunk", {"turn_id": "turn_py_1", "chunk": "Refactoring "})
        turn.dispatch_server_event("notification/turn_content_chunk", {"turn_id": "turn_py_1", "chunk": "complete."})
        turn.dispatch_server_event("notification/turn_completed", {"turn_id": "turn_py_1", "thread_id": "thread_abc"})

        res = await turn.result()
        self.assertEqual(res, "Refactoring complete.")
        self.assertEqual("".join(chunks), "Refactoring complete.")

    async def test_error_propagation(self):
        def bad_method(p):
            raise RuntimeError("Database error")

        self.transport.register_handler("error_call", bad_method)

        with self.assertRaises(JsonRpcError) as ctx:
            await self.client.request("error_call")
        self.assertIn("Database error", str(ctx.exception))

if __name__ == "__main__":
    unittest.main()
