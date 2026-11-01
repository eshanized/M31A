"""Transport layer implementations for M31A Python SDK.

Supports Unix Domain Sockets (UDS), Subprocess Standard I/O (stdio),
HTTP/SSE, and Mock transports for offline testing.
"""

import abc
import asyncio
import inspect
import json
import os
import urllib.request
import urllib.error
from typing import Any, Callable, Dict, List, Optional
from .errors import JsonRpcError, TimeoutError, TransportError

class Transport(abc.ABC):
    """Abstract base class for M31A App Server transports."""

    @abc.abstractmethod
    async def connect(self):
        """Establish connection to the server."""
        pass

    @abc.abstractmethod
    async def send_request(
        self,
        request_id: Any,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout_sec: Optional[float] = None,
    ) -> Any:
        """Send a JSON-RPC request and await its response."""
        pass

    @abc.abstractmethod
    async def close(self):
        """Close connection and clean up resources."""
        pass

    @abc.abstractmethod
    def on_notification(self, method: str, handler: Callable[[Dict[str, Any]], Any]):
        """Register a notification callback for a specific method."""
        pass

    @abc.abstractmethod
    def is_connected(self) -> bool:
        """Return True if the transport is actively connected."""
        pass


class UdsTransport(Transport):
    """Unix Domain Socket Transport for high-performance local IPC."""

    def __init__(self, socket_path: str, connect_timeout_sec: float = 10.0):
        self.socket_path = socket_path
        self.connect_timeout_sec = connect_timeout_sec
        self.reader: Optional[asyncio.StreamReader] = None
        self.writer: Optional[asyncio.StreamWriter] = None
        self._notification_handlers: Dict[str, List[Callable[[Dict[str, Any]], Any]]] = {}
        self._pending_responses: Dict[Any, asyncio.Future] = {}
        self._running = False
        self._read_task: Optional[asyncio.Task] = None

    def is_connected(self) -> bool:
        return self._running and self.writer is not None and not self.writer.is_closing()

    async def connect(self):
        try:
            self.reader, self.writer = await asyncio.wait_for(
                asyncio.open_unix_connection(self.socket_path),
                timeout=self.connect_timeout_sec,
            )
            self._running = True
            self._read_task = asyncio.create_task(self._listen_loop())
        except asyncio.TimeoutError as exc:
            raise TimeoutError(f"Connection to UDS socket '{self.socket_path}' timed out", self.connect_timeout_sec) from exc
        except Exception as exc:
            raise TransportError(f"Failed to connect to UDS socket '{self.socket_path}': {exc}", exc) from exc

    async def send_request(
        self,
        request_id: Any,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout_sec: Optional[float] = None,
    ) -> Any:
        if not self.is_connected() or self.writer is None:
            raise TransportError("Transport is not connected")

        payload = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
        }
        if params is not None:
            payload["params"] = params

        data = json.dumps(payload) + "\n"
        loop = asyncio.get_running_loop()
        future = loop.create_future()
        self._pending_responses[request_id] = future

        try:
            self.writer.write(data.encode("utf-8"))
            await self.writer.drain()
        except Exception as exc:
            self._pending_responses.pop(request_id, None)
            raise TransportError(f"Failed to write to socket: {exc}", exc) from exc

        if timeout_sec is not None:
            try:
                return await asyncio.wait_for(future, timeout=timeout_sec)
            except asyncio.TimeoutError as exc:
                self._pending_responses.pop(request_id, None)
                raise TimeoutError(f"Request '{method}' timed out after {timeout_sec}s", timeout_sec) from exc
        return await future

    def on_notification(self, method: str, handler: Callable[[Dict[str, Any]], Any]):
        if method not in self._notification_handlers:
            self._notification_handlers[method] = []
        self._notification_handlers[method].append(handler)

    async def _listen_loop(self):
        while self._running and self.reader:
            try:
                line = await self.reader.readline()
                if not line:
                    break
                line_str = line.decode("utf-8").strip()
                if not line_str:
                    continue

                frame = json.loads(line_str)
                if "id" in frame and frame["id"] is not None:
                    req_id = frame["id"]
                    future = self._pending_responses.pop(req_id, None)
                    if future and not future.done():
                        if "error" in frame and frame["error"]:
                            err = frame["error"]
                            future.set_exception(
                                JsonRpcError(
                                    code=err.get("code", -32603),
                                    message=err.get("message", "Unknown error"),
                                    data=err.get("data"),
                                )
                            )
                        else:
                            future.set_result(frame.get("result"))
                elif "method" in frame:
                    method = frame["method"]
                    params = frame.get("params", {})
                    handlers = self._notification_handlers.get(method, []) + self._notification_handlers.get("*", [])
                    for handler in handlers:
                        try:
                            if inspect.iscoroutinefunction(handler):
                                asyncio.create_task(handler(params))
                            else:
                                handler(params)
                        except Exception:
                            pass
            except Exception:
                break

    async def close(self):
        self._running = False
        if self._read_task:
            self._read_task.cancel()
        for fut in self._pending_responses.values():
            if not fut.done():
                fut.set_exception(TransportError("Transport connection closed"))
        self._pending_responses.clear()
        if self.writer:
            try:
                self.writer.close()
                await self.writer.wait_closed()
            except Exception:
                pass
            self.writer = None


class StdioTransport(Transport):
    """Child process Standard I/O Transport spawning `m31a app-server --stdio`."""

    def __init__(
        self,
        binary_path: str = "m31a",
        args: Optional[List[str]] = None,
        cwd: Optional[str] = None,
        env: Optional[Dict[str, str]] = None,
    ):
        self.binary_path = binary_path
        self.args = args or ["app-server", "stdio"]
        self.cwd = cwd
        self.env = env
        self.process: Optional[asyncio.subprocess.Process] = None
        self._notification_handlers: Dict[str, List[Callable[[Dict[str, Any]], Any]]] = {}
        self._pending_responses: Dict[Any, asyncio.Future] = {}
        self._running = False
        self._read_task: Optional[asyncio.Task] = None

    def is_connected(self) -> bool:
        return self._running and self.process is not None and self.process.returncode is None

    async def connect(self):
        cmd_env = dict(os.environ)
        if self.env:
            cmd_env.update(self.env)

        try:
            self.process = await asyncio.create_subprocess_exec(
                self.binary_path,
                *self.args,
                cwd=self.cwd,
                env=cmd_env,
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            self._running = True
            self._read_task = asyncio.create_task(self._listen_loop())
        except Exception as exc:
            raise TransportError(f"Failed to launch child process '{self.binary_path}': {exc}", exc) from exc

    async def send_request(
        self,
        request_id: Any,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout_sec: Optional[float] = None,
    ) -> Any:
        if not self.is_connected() or not self.process or not self.process.stdin:
            raise TransportError("Child process stdin not available")

        payload = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
        }
        if params is not None:
            payload["params"] = params

        data = json.dumps(payload) + "\n"
        loop = asyncio.get_running_loop()
        future = loop.create_future()
        self._pending_responses[request_id] = future

        try:
            self.process.stdin.write(data.encode("utf-8"))
            await self.process.stdin.drain()
        except Exception as exc:
            self._pending_responses.pop(request_id, None)
            raise TransportError(f"Failed to write to stdin: {exc}", exc) from exc

        if timeout_sec is not None:
            try:
                return await asyncio.wait_for(future, timeout=timeout_sec)
            except asyncio.TimeoutError as exc:
                self._pending_responses.pop(request_id, None)
                raise TimeoutError(f"Request '{method}' timed out after {timeout_sec}s", timeout_sec) from exc
        return await future

    def on_notification(self, method: str, handler: Callable[[Dict[str, Any]], Any]):
        if method not in self._notification_handlers:
            self._notification_handlers[method] = []
        self._notification_handlers[method].append(handler)

    async def _listen_loop(self):
        while self._running and self.process and self.process.stdout:
            try:
                line = await self.process.stdout.readline()
                if not line:
                    break
                line_str = line.decode("utf-8").strip()
                if not line_str:
                    continue

                frame = json.loads(line_str)
                if "id" in frame and frame["id"] is not None:
                    req_id = frame["id"]
                    future = self._pending_responses.pop(req_id, None)
                    if future and not future.done():
                        if "error" in frame and frame["error"]:
                            err = frame["error"]
                            future.set_exception(
                                JsonRpcError(
                                    code=err.get("code", -32603),
                                    message=err.get("message", "Unknown error"),
                                    data=err.get("data"),
                                )
                            )
                        else:
                            future.set_result(frame.get("result"))
                elif "method" in frame:
                    method = frame["method"]
                    params = frame.get("params", {})
                    handlers = self._notification_handlers.get(method, []) + self._notification_handlers.get("*", [])
                    for handler in handlers:
                        try:
                            if inspect.iscoroutinefunction(handler):
                                asyncio.create_task(handler(params))
                            else:
                                handler(params)
                        except Exception:
                            pass
            except Exception:
                break

    async def close(self):
        self._running = False
        if self._read_task:
            self._read_task.cancel()
        for fut in self._pending_responses.values():
            if not fut.done():
                fut.set_exception(TransportError("Subprocess closed"))
        self._pending_responses.clear()
        if self.process:
            try:
                self.process.terminate()
                await self.process.wait()
            except Exception:
                pass
            self.process = None


class HttpTransport(Transport):
    """HTTP/SSE Transport for remote M31A App Server gateways."""

    def __init__(self, base_url: str, enable_sse: bool = True):
        self.base_url = base_url.rstrip("/")
        if not self.base_url.startswith("http://") and not self.base_url.startswith("https://"):
            self.base_url = "http://" + self.base_url
        self.enable_sse = enable_sse
        self._connected = False
        self._notification_handlers: Dict[str, List[Callable[[Dict[str, Any]], Any]]] = {}
        self._sse_task: Optional[asyncio.Task] = None

    def is_connected(self) -> bool:
        return self._connected

    async def connect(self):
        self._connected = True
        if self.enable_sse:
            self._sse_task = asyncio.create_task(self._sse_listener())

    async def _sse_listener(self):
        # Background SSE reader connecting to /events
        sse_url = f"{self.base_url}/events"
        while self._connected:
            try:
                def read_stream():
                    req = urllib.request.Request(sse_url, headers={"Accept": "text/event-stream"})
                    return urllib.request.urlopen(req, timeout=60)

                resp = await asyncio.to_thread(read_stream)
                while self._connected:
                    line_bytes = await asyncio.to_thread(resp.readline)
                    if not line_bytes:
                        break
                    line = line_bytes.decode("utf-8").strip()
                    if line.startswith("data:"):
                        data_str = line[5:].strip()
                        if data_str:
                            try:
                                parsed = json.loads(data_str)
                                method = parsed.get("method", "notification")
                                params = parsed.get("params", parsed)
                                handlers = self._notification_handlers.get(method, []) + self._notification_handlers.get("*", [])
                                for handler in handlers:
                                    if inspect.iscoroutinefunction(handler):
                                        asyncio.create_task(handler(params))
                                    else:
                                        handler(params)
                            except Exception:
                                pass
            except Exception:
                if not self._connected:
                    break
                await asyncio.sleep(1.0)

    async def send_request(
        self,
        request_id: Any,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout_sec: Optional[float] = None,
    ) -> Any:
        url = f"{self.base_url}/rpc"
        payload = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
        }
        if params is not None:
            payload["params"] = params

        data = json.dumps(payload).encode("utf-8")

        def make_http_call():
            req = urllib.request.Request(
                url,
                data=data,
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=timeout_sec or 30.0) as resp:
                return resp.read().decode("utf-8")

        try:
            body = await asyncio.to_thread(make_http_call)
            frame = json.loads(body)
            if "error" in frame and frame["error"]:
                err = frame["error"]
                raise JsonRpcError(
                    code=err.get("code", -32603),
                    message=err.get("message", "HTTP RPC error"),
                    data=err.get("data"),
                )
            return frame.get("result")
        except urllib.error.HTTPError as exc:
            raise TransportError(f"HTTP error {exc.code}: {exc.reason}", exc) from exc
        except Exception as exc:
            if isinstance(exc, JsonRpcError):
                raise
            raise TransportError(f"HTTP request failed: {exc}", exc) from exc

    def on_notification(self, method: str, handler: Callable[[Dict[str, Any]], Any]):
        if method not in self._notification_handlers:
            self._notification_handlers[method] = []
        self._notification_handlers[method].append(handler)

    async def close(self):
        self._connected = False
        if self._sse_task:
            self._sse_task.cancel()


class MockTransport(Transport):
    """In-memory mock transport for offline testing."""

    def __init__(self):
        self._connected = False
        self._handlers: Dict[str, Callable[[Any], Any]] = {}
        self._notification_handlers: Dict[str, List[Callable[[Dict[str, Any]], Any]]] = {}
        self.sent_requests: List[Dict[str, Any]] = []

    def is_connected(self) -> bool:
        return self._connected

    async def connect(self):
        self._connected = True

    def register_handler(self, method: str, handler: Callable[[Any], Any]):
        self._handlers[method] = handler

    async def send_request(
        self,
        request_id: Any,
        method: str,
        params: Optional[Dict[str, Any]] = None,
        timeout_sec: Optional[float] = None,
    ) -> Any:
        self.sent_requests.append({"id": request_id, "method": method, "params": params})
        if method in self._handlers:
            handler = self._handlers[method]
            try:
                res = handler(params)
                if asyncio.iscoroutine(res):
                    return await res
                return res
            except Exception as exc:
                raise JsonRpcError(code=-32603, message=str(exc)) from exc
        return {"ok": True}

    def on_notification(self, method: str, handler: Callable[[Dict[str, Any]], Any]):
        if method not in self._notification_handlers:
            self._notification_handlers[method] = []
        self._notification_handlers[method].append(handler)

    def simulate_notification(self, method: str, params: Dict[str, Any]):
        handlers = self._notification_handlers.get(method, []) + self._notification_handlers.get("*", [])
        for handler in handlers:
            if inspect.iscoroutinefunction(handler):
                asyncio.create_task(handler(params))
            else:
                handler(params)

    async def close(self):
        self._connected = False
