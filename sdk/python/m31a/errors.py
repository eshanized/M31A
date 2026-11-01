"""Error hierarchy for M31A Python SDK."""

from typing import Any, Optional

class M31AError(Exception):
    """Base exception for all M31A SDK errors."""
    pass

class JsonRpcError(M31AError):
    """Exception representing a standard JSON-RPC 2.0 protocol error."""

    def __init__(self, code: int, message: str, data: Optional[Any] = None):
        super().__init__(f"JSON-RPC Error [{code}]: {message}")
        self.code = code
        self.message = message
        self.data = data

class TransportError(M31AError):
    """Raised when an underlying transport fails or drops connection."""

    def __init__(self, message: str, cause: Optional[Exception] = None):
        super().__init__(message)
        self.cause = cause

class TimeoutError(M31AError):
    """Raised when an RPC request or turn execution times out."""

    def __init__(self, message: str, timeout_sec: Optional[float] = None):
        super().__init__(message)
        self.timeout_sec = timeout_sec

class TurnRejectedError(M31AError):
    """Raised when turn admission is rejected by the server."""

    def __init__(self, reason: str):
        super().__init__(f"Turn admission rejected: {reason}")
        self.reason = reason

class ApprovalDeniedError(M31AError):
    """Raised when an action or tool call is denied by Guardian review or human policy."""

    def __init__(self, rationale: str):
        super().__init__(f"Action denied: {rationale}")
        self.rationale = rationale
