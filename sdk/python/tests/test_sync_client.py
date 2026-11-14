"""Tests for SyncM31AClient blocking client."""

import unittest
from m31a.sync_client import SyncM31AClient
from m31a.transport import MockTransport

class TestSyncClient(unittest.TestCase):
    def setUp(self):
        self.transport = MockTransport()
        self.transport.register_handler(
            "initialize",
            lambda p: {
                "server_name": "m31a-app-server",
                "version": "0.1.15",
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
            "thread/create",
            lambda p: {
                "id": "thread_sync_1",
                "title": "Sync Session",
                "model": "nvidia/nemotron-3.5-lightning-30b-a3b",
                "cwd": ".",
                "created_at": "2026-10-30T00:00:00Z",
                "updated_at": "2026-10-30T00:00:00Z",
            },
        )
        self.transport.register_handler(
            "turn/start",
            lambda p: {
                "turn_id": "turn_sync_1",
                "thread_id": p["thread_id"],
                "lease_id": "lease_sync",
                "status": "started",
            },
        )
        self.transport.register_handler(
            "checkpoint/create",
            lambda p: {
                "id": "cp_sync_1",
                "thread_id": p["thread_id"],
                "label": p["label"],
                "turn_index": 1,
                "modified_files": [],
                "created_at": "2026-10-30T00:00:00Z",
            },
        )

        self.client = SyncM31AClient(transport=self.transport, timeout_sec=1.0)
        self.client.connect()

    def tearDown(self):
        self.client.close()

    def test_sync_diagnostics_and_threads(self):
        diag = self.client.get_diagnostics()
        self.assertEqual(diag.status, "healthy")

        thread = self.client.create_thread(cwd="/tmp")
        self.assertEqual(thread.id, "thread_sync_1")

        cp = thread.create_checkpoint("Sync Snapshot")
        self.assertEqual(cp.id, "cp_sync_1")

if __name__ == "__main__":
    unittest.main()
