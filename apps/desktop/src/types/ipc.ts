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

export interface SessionDetailsDto {
  id: string;
  title: string;
  model: string;
  cwd: string;
  created_at: number;
  updated_at: number;
  turns: TurnSummaryDto[];
}

export interface SubmitTurnPayload {
  thread_id?: string | null;
  prompt: string;
  collaboration_mode?: 'default' | 'plan' | 'review';
  allow_untrusted_workspace?: boolean;
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
