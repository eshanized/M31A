"""Comprehensive data models, types, and schemas for M31A Python SDK."""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Literal, Optional

class LockMode(str, Enum):
    READ = "read"
    WRITE = "write"

class TurnPriority(str, Enum):
    USER_INTERACTIVE = "user_interactive"
    BACKGROUND_TASK = "background_task"
    SUBAGENT_DELEGATION = "subagent_delegation"

@dataclass
class ServerCapabilities:
    threads: bool = True
    turns: bool = True
    interactive_approvals: bool = True
    turn_steering: bool = True
    turn_interrupt: bool = True
    code_mode: bool = True
    guardian_review: bool = True
    message_board: bool = True
    skills: bool = True
    sandbox: bool = True

@dataclass
class InitializeResult:
    server_name: str
    version: str
    protocol_version: str
    capabilities: Dict[str, Any]

@dataclass
class DiagnosticsResult:
    status: str
    api_key_configured: bool
    bubblewrap_available: bool
    active_model: str
    sandbox_policy: str
    approval_policy: str

@dataclass
class AppConfig:
    model: str
    api_base: str
    sandbox: str
    approval: str
    api_key: Optional[str] = None
    routing_enabled: Optional[bool] = None
    tool_output_token_limit: Optional[int] = None
    features: Dict[str, bool] = field(default_factory=dict)
    raw: Dict[str, Any] = field(default_factory=dict)

@dataclass
class SessionMetadata:
    id: str
    title: str
    model: str
    cwd: str
    created_at: str
    updated_at: str
    extra: Optional[Dict[str, str]] = None

@dataclass
class ThreadItem:
    id: str
    thread_id: str
    role: str
    content: str
    created_at: str
    turn_id: Optional[str] = None

@dataclass
class ThreadDetails:
    thread: SessionMetadata
    items: List[ThreadItem]

@dataclass
class ThreadSection:
    id: str
    name: str
    color: Optional[str] = None
    icon: Optional[str] = None
    created_at: str = ""
    updated_at: str = ""

@dataclass
class StartTurnResult:
    status: str
    thread_id: str
    turn_id: Optional[str] = None
    lease_id: Optional[str] = None
    active_lease_id: Optional[str] = None
    queue_position: Optional[int] = None
    estimated_wait_ms: Optional[int] = None

@dataclass
class FinishTurnResult:
    thread_id: str
    status: str
    has_next_queued: bool

@dataclass
class ApprovalRequest:
    approval_id: str
    turn_id: str
    thread_id: str
    tool: str
    arguments: str
    reason: str
    command: Optional[str] = None

@dataclass
class ApprovalDecision:
    decision: Literal["allow", "allow_always", "deny"] = "allow"
    reason: Optional[str] = None
    command: Optional[str] = None
    pattern: Optional[str] = None

@dataclass
class QuestionRequest:
    question_id: str
    turn_id: str
    thread_id: str
    prompt: str
    options: List[str]
    is_multi_select: bool

@dataclass
class CheckpointRecord:
    id: str
    thread_id: str
    label: str
    turn_index: int
    modified_files: List[str]
    created_at: str
    git_commit: Optional[str] = None
    metadata: Optional[Dict[str, Any]] = None

@dataclass
class LockRecord:
    token: str
    path: str
    mode: str
    holder: str
    reason: str
    expires_at_ms: int

@dataclass
class BoardPost:
    id: str
    board: str
    channel: str
    author: str
    body: str
    created_at: str
    payload: Dict[str, Any] = field(default_factory=dict)

@dataclass
class ThreadGoal:
    id: str
    thread_id: str
    objective: str
    status: str
    created_at: str
    updated_at: str
    budget: Optional[int] = None

@dataclass
class InboxItem:
    id: str
    thread_id: str
    kind: str
    payload: str
    created_at: str

@dataclass
class ClaimRecord:
    id: str
    thread_id: str
    agent_id: str
    lease_expires_at: str
    status: str

@dataclass
class ModelInfo:
    slug: str
    display_name: str
    description: str
    context_window: int
    max_output_tokens: int
    supports_tools: bool
    supports_thinking: bool
    supports_vision: bool
    originator: str

@dataclass
class ToolDefinition:
    type: str
    function: Dict[str, Any]

@dataclass
class SkillDefinition:
    name: str
    description: str
    source_path: Optional[str] = None
    instructions: Optional[str] = None

@dataclass
class InstructionSource:
    source: str
    path: Optional[str] = None
    content_preview: Optional[str] = None
