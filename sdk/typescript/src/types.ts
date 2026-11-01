/**
 * Comprehensive Protocol & Data Types for M31A App Server SDK.
 */

// --- JSON-RPC 2.0 Base Types ---

export interface JsonRpcRequest<T = unknown> {
  jsonrpc: '2.0';
  id: number | string;
  method: string;
  params?: T;
}

export interface JsonRpcResponse<T = unknown> {
  jsonrpc: '2.0';
  id?: number | string | null;
  result?: T;
  error?: JsonRpcErrorData;
}

export interface JsonRpcErrorData {
  code: number;
  message: string;
  data?: unknown;
}

export interface JsonRpcNotification<T = unknown> {
  jsonrpc: '2.0';
  method: string;
  params?: T;
}

// --- Server Lifecycle & Diagnostics ---

export interface ServerCapabilities {
  threads: boolean;
  turns: boolean;
  interactive_approvals?: boolean;
  turn_steering?: boolean;
  turn_interrupt?: boolean;
  code_mode: boolean;
  guardian_review: boolean;
  message_board: boolean;
  skills: boolean;
  sandbox: boolean;
}

export interface InitializeResult {
  server_name: string;
  version: string;
  protocol_version: string;
  capabilities: ServerCapabilities;
}

export interface DiagnosticsResult {
  status: string;
  api_key_configured: boolean;
  bubblewrap_available: boolean;
  active_model: string;
  sandbox_policy: string;
  approval_policy: string;
}

export interface AppConfig {
  model: string;
  api_base: string;
  api_key?: string;
  sandbox: string;
  approval: string;
  routing_enabled?: boolean;
  tool_output_token_limit?: number;
  features?: Record<string, boolean>;
  [key: string]: unknown;
}

// --- Thread & Session Types ---

export interface SessionMetadata {
  id: string;
  title: string;
  model: string;
  cwd: string;
  created_at: string;
  updated_at: string;
  extra?: Record<string, string>;
}

export interface ThreadItem {
  id: string;
  thread_id: string;
  turn_id?: string;
  role: 'system' | 'user' | 'assistant' | 'tool' | 'system_notice';
  content: string;
  created_at: string;
}

export interface ThreadDetails {
  thread: SessionMetadata;
  items: ThreadItem[];
}

export interface CreateThreadParams {
  cwd?: string;
  model?: string;
}

export interface ForkThreadParams {
  thread_id: string;
  title?: string;
}

// --- Thread Sections ---

export interface ThreadSection {
  id: string;
  name: string;
  color?: string | null;
  icon?: string | null;
  created_at: string;
  updated_at: string;
}

// --- Turn Execution & Admission ---

export interface StartTurnParams {
  thread_id: string;
  prompt: string;
}

export type TurnAdmissionStatus = 'started' | 'steered_into_active_turn' | 'queued';

export interface StartTurnResult {
  turn_id?: string;
  thread_id: string;
  lease_id?: string;
  status: TurnAdmissionStatus;
  active_lease_id?: string;
  queue_position?: number;
  estimated_wait_ms?: number;
}

export interface FinishTurnResult {
  thread_id: string;
  status: string;
  has_next_queued: boolean;
}

export interface SteerTurnResult {
  steered: boolean;
  turn_id: string;
  thread_id?: string;
}

export interface InterruptTurnResult {
  interrupted: boolean;
  turn_id: string;
  thread_id?: string;
}

// --- Interactive Approvals & Questions ---

export type ApprovalDecisionType = 'allow' | 'allow_always' | 'deny';

export interface ApprovalDecision {
  decision?: ApprovalDecisionType;
  reason?: string;
  command?: string;
  pattern?: string;
}

export interface ApprovalRequest {
  approval_id: string;
  turn_id: string;
  thread_id: string;
  tool: string;
  arguments: string;
  command?: string;
  reason: string;
}

export interface ApprovalResult {
  approval_id: string;
  resolved: boolean;
  denied?: boolean;
}

export interface QuestionRequest {
  question_id: string;
  turn_id: string;
  thread_id: string;
  prompt: string;
  options: string[];
  is_multi_select: boolean;
}

export interface QuestionResponse {
  question_id: string;
  resolved: boolean;
  answers: string[];
}

// --- Checkpoints ---

export interface CheckpointRecord {
  id: string;
  thread_id: string;
  label: string;
  git_commit?: string | null;
  turn_index: number;
  modified_files: string[];
  created_at: string;
  metadata?: Record<string, unknown>;
}

export interface CreateCheckpointParams {
  thread_id: string;
  label: string;
  git_commit?: string;
  turn_index?: number;
  modified_files?: string[];
}

// --- Concurrency Path Locks ---

export type LockMode = 'read' | 'write';

export interface LockRecord {
  token: string;
  path: string;
  mode: LockMode;
  holder: string;
  reason: string;
  expires_at_ms: number;
}

export interface AcquireLockParams {
  path: string;
  mode?: LockMode;
  holder?: string;
  reason?: string;
  timeout_ms?: number;
}

export interface AcquireLockResult {
  token: string;
  path: string;
  mode: string;
}

export interface ReleaseLockResult {
  token: string;
  released: boolean;
}

// --- Message Board (Pub-Sub) ---

export interface BoardPost {
  id: string;
  board: string;
  channel: string;
  author: string;
  body: string;
  payload?: Record<string, unknown>;
  created_at: string;
}

export interface PostToBoardParams {
  channel?: string;
  author?: string;
  body: string;
  payload?: Record<string, unknown>;
}

// --- Mission Goals ---

export interface ThreadGoal {
  id: string;
  thread_id: string;
  objective: string;
  budget?: number | null;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface SetGoalParams {
  thread_id: string;
  objective: string;
  budget?: number;
}

// --- Session Inboxes ---

export interface InboxItem {
  id: string;
  thread_id: string;
  kind: string;
  payload: string;
  created_at: string;
}

export interface EnqueueInboxParams {
  thread_id: string;
  kind?: string;
  payload: string;
}

// --- Active Claims ---

export interface ClaimRecord {
  id: string;
  thread_id: string;
  agent_id: string;
  lease_expires_at: string;
  status: string;
}

// --- Models, Tools & Skills ---

export interface ModelInfo {
  slug: string;
  display_name: string;
  description: string;
  context_window: number;
  max_output_tokens: number;
  supports_tools: boolean;
  supports_thinking: boolean;
  supports_vision: boolean;
  supports_audio: boolean;
  originator: string;
}

export interface ToolDefinition {
  type: string;
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}

export interface SkillDefinition {
  name: string;
  description: string;
  source_path?: string;
  instructions?: string;
}

export interface InstructionSource {
  source: string;
  path?: string;
  content_preview?: string;
}

// --- Strongly Typed Streaming Notifications ---

export interface TurnStartedNotification {
  turn_id: string;
  thread_id: string;
  lease_id?: string;
}

export interface TurnContentChunkNotification {
  turn_id: string;
  chunk: string;
}

export interface TurnReasoningChunkNotification {
  turn_id: string;
  chunk: string;
}

export interface ToolStartedNotification {
  turn_id: string;
  call_id: string;
  tool: string;
  arguments: string;
}

export interface ToolFinishedNotification {
  turn_id: string;
  call_id: string;
  success: boolean;
  output: string;
}

export interface GuardianDeniedNotification {
  turn_id: string;
  rationale: string;
}

export interface NoticeNotification {
  turn_id: string;
  message: string;
  warning: boolean;
}

export interface CompactionNotification {
  turn_id: string;
  window_id: string;
  original_tokens: number;
  compacted_tokens: number;
}

export interface TurnMessageNotification {
  turn_id: string;
  message: string;
  usage?: Record<string, unknown>;
}

export interface TurnCompletedNotification {
  turn_id: string;
  thread_id: string;
}

export interface TurnFailedNotification {
  turn_id: string;
  thread_id?: string;
  error: string;
}

export interface TurnInterruptedNotification {
  turn_id: string;
  thread_id?: string;
}

export interface TurnFinishedNotification {
  thread_id: string;
  lease_id: string;
}

export interface CheckpointCreatedNotification {
  id: string;
  thread_id: string;
  label: string;
  turn_index: number;
}

export interface CheckpointRestoredNotification {
  id: string;
  thread_id: string;
  turn_index: number;
}

export interface LockAcquiredNotification {
  token: string;
  path: string;
  mode: string;
  holder: string;
}

export interface LockReleasedNotification {
  token: string;
  released: boolean;
}

export interface NotificationPayloadMap {
  'notification/turn_started': TurnStartedNotification;
  'notification/turn_content_chunk': TurnContentChunkNotification;
  'notification/turn_reasoning_chunk': TurnReasoningChunkNotification;
  'notification/tool_started': ToolStartedNotification;
  'notification/tool_finished': ToolFinishedNotification;
  'notification/guardian_denied': GuardianDeniedNotification;
  'notification/notice': NoticeNotification;
  'notification/approval_requested': ApprovalRequest;
  'notification/question_requested': QuestionRequest;
  'notification/compaction': CompactionNotification;
  'notification/turn_message': TurnMessageNotification;
  'notification/turn_completed': TurnCompletedNotification;
  'notification/turn_failed': TurnFailedNotification;
  'notification/turn_interrupted': TurnInterruptedNotification;
  'notification/turn_finished': TurnFinishedNotification;
  'notification/checkpoint_created': CheckpointCreatedNotification;
  'notification/checkpoint_restored': CheckpointRestoredNotification;
  'notification/lock_acquired': LockAcquiredNotification;
  'notification/lock_released': LockReleasedNotification;
  [key: string]: unknown;
}
