/**
 * Typed Tauri IPC Client for M31A Desktop.
 * Calls Rust command handlers defined in `apps/desktop/src-tauri/src/commands/mod.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  EnvironmentInfoDto,
  ModelInfoDto,
  PendingInteractionsDto,
  RuntimeStatusDto,
  SessionDetailsDto,
  SessionSummaryDto,
  SubmitTurnPayload,
  TurnHandleDto,
  WorkspaceInfoDto,
} from '../types/ipc';

export const ipc = {
  // Lifecycle
  async getRuntimeStatus(): Promise<RuntimeStatusDto> {
    return invoke<RuntimeStatusDto>('get_runtime_status');
  },

  async getEnvironmentInfo(): Promise<EnvironmentInfoDto> {
    return invoke<EnvironmentInfoDto>('get_environment_info');
  },

  async shutdownRuntime(): Promise<void> {
    return invoke<void>('shutdown_runtime');
  },

  // Workspace
  async getActiveWorkspace(): Promise<WorkspaceInfoDto> {
    return invoke<WorkspaceInfoDto>('get_active_workspace');
  },

  async setWorkspace(path: string): Promise<WorkspaceInfoDto> {
    return invoke<WorkspaceInfoDto>('set_workspace', { path });
  },

  async authorizeWorkspace(path: string): Promise<WorkspaceInfoDto> {
    return invoke<WorkspaceInfoDto>('authorize_workspace', { path });
  },

  // Models
  async listAvailableModels(): Promise<ModelInfoDto[]> {
    return invoke<ModelInfoDto[]>('list_available_models');
  },

  async getActiveModel(): Promise<string> {
    return invoke<string>('get_active_model');
  },

  async activateModel(model: string): Promise<string> {
    return invoke<string>('activate_model', { model });
  },

  // Sessions
  async listSessions(): Promise<SessionSummaryDto[]> {
    return invoke<SessionSummaryDto[]>('list_sessions');
  },

  async createSession(title?: string): Promise<SessionSummaryDto> {
    return invoke<SessionSummaryDto>('create_session', { title });
  },

  async loadSession(sessionId: string): Promise<SessionDetailsDto> {
    return invoke<SessionDetailsDto>('load_session', { sessionId });
  },

  // Turns
  async submitTurn(payload: SubmitTurnPayload): Promise<TurnHandleDto> {
    return invoke<TurnHandleDto>('submit_turn', { payload });
  },

  async cancelTurn(threadId: string, turnId: string): Promise<void> {
    return invoke<void>('cancel_turn', { threadId, turnId });
  },

  async steerTurn(
    threadId: string,
    turnId: string,
    message: string
  ): Promise<void> {
    return invoke<void>('steer_turn', { threadId, turnId, message });
  },

  // Approvals & Questions
  async respondApproval(
    requestId: string,
    decision: boolean,
    reason?: string
  ): Promise<void> {
    return invoke<void>('respond_approval', {
      requestId,
      decision,
      reason,
    });
  },

  async respondQuestion(
    questionId: string,
    answers: string[]
  ): Promise<void> {
    return invoke<void>('respond_question', {
      questionId,
      answers,
    });
  },
};
