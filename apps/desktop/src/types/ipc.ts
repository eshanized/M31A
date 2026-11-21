/**
 * Strongly-typed IPC DTO definitions for M31A Desktop.
 * Matches Rust DTO definitions in `apps/desktop/src-tauri/src/commands/mod.rs`.
 */

export interface RuntimeStatusDto {
  state: string;
  active_model: string;
  active_workspace: string;
  is_operational: boolean;
  is_terminal: boolean;
  active_session_id?: string | null;
}

export interface EnvironmentInfoDto {
  app_version: string;
  target_platform: string;
  target_arch: string;
  features: string[];
}

export interface WorkspaceInfoDto {
  path: string;
  is_trusted: boolean;
  exists: boolean;
}

export interface ModelInfoDto {
  id: string;
  name: string;
  originator: string;
  context_window: number;
  supports_tools: boolean;
  supports_thinking: boolean;
  supports_vision: boolean;
}

export interface SessionSummaryDto {
  id: string;
  title: string;
  model: string;
  cwd: string;
  created_at: number;
  updated_at: number;
  is_active: boolean;
}

export interface TurnSummaryDto {
  id: string;
  thread_id: string;
  turn_index: number;
  status: string;
  created_at: number;
}

export interface SessionMessageDto {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

export interface SessionDetailsDto {
  id: string;
  title: string;
  model: string;
  cwd: string;
  workspace_is_trusted: boolean;
  created_at: number;
  updated_at: number;
  turns: TurnSummaryDto[];
  messages: SessionMessageDto[];
}

export interface SubmitTurnPayload {
  thread_id?: string | null;
  prompt: string;
  collaboration_mode?: 'default' | 'plan' | 'review';
}

export interface TurnHandleDto {
  thread_id: string;
  turn_id: string;
}

export interface PendingApprovalDto {
  request_id: string;
  thread_id: string;
  turn_id: string;
  command: string;
  reason: string;
}

export interface PendingQuestionDto {
  question_id: string;
  thread_id: string;
  turn_id: string;
  question: string;
  options: string[];
  multi_select: boolean;
}

export interface PendingInteractionsDto {
  approvals: PendingApprovalDto[];
  questions: PendingQuestionDto[];
}

export interface ShutdownReportDto {
  cleanly_stopped: boolean;
  background_processes_stopped: number;
  active_turns_cancelled: number;
  watcher_stopped: boolean;
  mcp_servers_stopped: number;
  timestamp: string;
}

export interface WorkspaceFileEntryDto {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified_at?: number | null;
}

export interface ToolDefinitionDto {
  name: string;
  description: string;
  parameters_schema?: string | null;
}

export interface DocxParagraphDto {
  text: string;
  style?: string | null;
  is_heading: boolean;
}

export interface DocxTableDto {
  rows: string[][];
}

export type DocxBlockDto =
  | { kind: 'paragraph'; data: DocxParagraphDto }
  | { kind: 'table'; data: DocxTableDto };

export interface DocxPreviewPayloadDto {
  file_name: string;
  blocks: DocxBlockDto[];
  total_paragraphs: number;
  total_words: number;
}

export interface XlsxSheetDto {
  name: string;
  rows: string[][];
  max_columns: number;
}

export interface XlsxPreviewPayloadDto {
  file_name: string;
  sheets: XlsxSheetDto[];
  active_sheet_index: number;
}

export interface PdfPreviewPayloadDto {
  file_name: string;
  page_count: number;
  title?: string | null;
  author?: string | null;
  file_size_bytes: number;
  text_preview: string;
}

export interface RuntimeConfigDto {
  model: string;
  api_base: string;
  sandbox_policy: string;
  approval_policy: string;
  active_features: string[];
  dangerously_bypass: boolean;
}

export interface FileMentionMatchDto {
  path: string;
  filename: string;
  is_dir: boolean;
}

export interface SlashCommandDto {
  name: string;
  description: string;
  usage: string;
  category: string;
  subcommands: string[];
}
