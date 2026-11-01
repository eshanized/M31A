"""Tests for M31A transport layer."""

import asyncio
import unittest
from m31a.transport import MockTransport
from m31a.errors import JsonRpcError

class TestTransport(unittest.IsolatedAsyncioTestCase):
    async def test_mock_transport_request_response(self):
        transport = MockTransport()
        await transport.connect()
        self.assertTrue(transport.is_connected())

        transport.register_handler("math/add", lambda p: {"sum": p["a"] + p["b"]})

        res = await transport.send_request(1, "math/add", {"a": 10, "b": 32})
        self.assertEqual(res, {"sum": 42})

        await transport.close()
        self.assertFalse(transport.is_connected())

    async def test_mock_transport_notifications(self):
        transport = MockTransport()
        await transport.connect()

        received = []
        transport.on_notification("test/notice", lambda p: received.append(p))

        transport.simulate_notification("test/notice", {"msg": "hello"})
        await asyncio.sleep(0.01)

        self.assertEqual(len(received), 1)
        self.assertEqual(received[0], {"msg": "hello"})

        await transport.close()

    async def test_mock_transport_error_handling(self):
        transport = MockTransport()
        await transport.connect()

        def fail_handler(p):
            raise ValueError("Operation not allowed")

        transport.register_handler("fail", fail_handler)

        with self.assertRaises(JsonRpcError) as ctx:
            await transport.send_request(2, "fail")
        self.assertIn("Operation not allowed", str(ctx.exception))

        await transport.close()

if __name__ == "__main__":
    unittest.main()
