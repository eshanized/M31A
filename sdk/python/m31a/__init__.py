"""M31A Python SDK.

Official Python client for the M31A Autonomous Agentic Coding Engine powered by NVIDIA NIM.
Provides typed async and synchronous clients, multi-transport communication
(Unix Domain Sockets, Subprocess Stdio, HTTP/SSE), turn streaming abstractions,
automated tool approvals, checkpoints, path locks, and distributed message boards.
"""

from .client import M31AClient
from .sync_client import SyncM31AClient, SyncThreadHandle, SyncActiveTurn
from .thread import ThreadHandle
from .turn import ActiveTurn, TurnEvent
from .transport import HttpTransport, MockTransport, StdioTransport, Transport, UdsTransport
from .errors import (
    ApprovalDeniedError,
    JsonRpcError,
    M31AError,
    TimeoutError,
    TransportError,
    TurnRejectedError,
)
from .types import (
    AppConfig,
    ApprovalDecision,
    ApprovalRequest,
    BoardPost,
    CheckpointRecord,
    ClaimRecord,
    DiagnosticsResult,
    FinishTurnResult,
    InboxItem,
    InitializeResult,
    InstructionSource,
    LockMode,
    LockRecord,
    ModelInfo,
    QuestionRequest,
    ServerCapabilities,
    SessionMetadata,
    SkillDefinition,
    StartTurnResult,
    ThreadDetails,
    ThreadGoal,
    ThreadItem,
    ThreadSection,
    ToolDefinition,
    TurnPriority,
)

__all__ = [
    # Clients & Handles
    "M31AClient",
    "SyncM31AClient",
    "ThreadHandle",
    "SyncThreadHandle",
    "ActiveTurn",
    "SyncActiveTurn",
    "TurnEvent",
    # Transports
    "Transport",
    "UdsTransport",
    "StdioTransport",
    "HttpTransport",
    "MockTransport",
    # Errors
    "M31AError",
    "JsonRpcError",
    "TransportError",
    "TimeoutError",
    "TurnRejectedError",
    "ApprovalDeniedError",
    # Data Models
    "InitializeResult",
    "ServerCapabilities",
    "DiagnosticsResult",
    "AppConfig",
    "SessionMetadata",
    "ThreadItem",
    "ThreadDetails",
    "ThreadSection",
    "StartTurnResult",
    "FinishTurnResult",
    "ApprovalRequest",
    "ApprovalDecision",
    "QuestionRequest",
    "CheckpointRecord",
    "LockRecord",
    "LockMode",
    "BoardPost",
    "ThreadGoal",
    "InboxItem",
    "ClaimRecord",
    "ModelInfo",
    "ToolDefinition",
    "SkillDefinition",
    "InstructionSource",
    "TurnPriority",
]
