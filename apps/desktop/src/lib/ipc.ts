/**
 * Typed Tauri IPC Client for M31A Desktop.
 * Calls Rust command handlers defined in `apps/desktop/src-tauri/src/commands/mod.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  EnvironmentInfoDto,
  ModelInfoDto,
  PendingInteractionsDto,
  RuntimeConfigDto,
  RuntimeStatusDto,
  SessionDetailsDto,
  SessionSummaryDto,
  ShutdownReportDto,
  SubmitTurnPayload,
  ToolDefinitionDto,
  TurnHandleDto,
  WorkspaceFileEntryDto,
  WorkspaceInfoDto,
  DocxPreviewPayloadDto,
  XlsxPreviewPayloadDto,
  PdfPreviewPayloadDto,
} from '../types/ipc';

export const ipc = {
  // Lifecycle
  async getRuntimeStatus(): Promise<RuntimeStatusDto> {
    return invoke<RuntimeStatusDto>('get_runtime_status');
  },

  async getEnvironmentInfo(): Promise<EnvironmentInfoDto> {
    return invoke<EnvironmentInfoDto>('get_environment_info');
  },

  async shutdownRuntime(): Promise<ShutdownReportDto> {
    return invoke<ShutdownReportDto>('shutdown_runtime');
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

  async getPendingInteractions(): Promise<PendingInteractionsDto> {
    return invoke<PendingInteractionsDto>('get_pending_interactions');
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

  // Workspace Exploration
  async pickWorkspaceDirectory(): Promise<WorkspaceInfoDto | null> {
    return invoke<WorkspaceInfoDto | null>('pick_workspace_directory');
  },

  async listWorkspaceFiles(subpath?: string): Promise<WorkspaceFileEntryDto[]> {
    return invoke<WorkspaceFileEntryDto[]>('list_workspace_files', { subpath });
  },

  async readWorkspaceFilePreview(path: string): Promise<string> {
    return invoke<string>('read_workspace_file_preview', { path });
  },

  async previewDocxDocument(path: string): Promise<DocxPreviewPayloadDto> {
    return invoke<DocxPreviewPayloadDto>('preview_docx_document', { path });
  },

  async previewXlsxSpreadsheet(path: string): Promise<XlsxPreviewPayloadDto> {
    return invoke<XlsxPreviewPayloadDto>('preview_xlsx_spreadsheet', { path });
  },

  async previewPdfDocument(path: string): Promise<PdfPreviewPayloadDto> {
    return invoke<PdfPreviewPayloadDto>('preview_pdf_document', { path });
  },

  // Extended Session Actions
  async renameSession(sessionId: string, newTitle: string): Promise<void> {
    return invoke<void>('rename_session', { sessionId, newTitle });
  },

  async deleteSession(sessionId: string): Promise<void> {
    return invoke<void>('delete_session', { sessionId });
  },

  async forkSession(
    sessionId: string,
    newTitle?: string
  ): Promise<SessionSummaryDto> {
    return invoke<SessionSummaryDto>('fork_session', { sessionId, newTitle });
  },

  async exportSessionTranscript(sessionId: string): Promise<string> {
    return invoke<string>('export_session_transcript', { sessionId });
  },

  // Tool Catalog & System Configuration
  async listRegisteredTools(): Promise<ToolDefinitionDto[]> {
    return invoke<ToolDefinitionDto[]>('list_registered_tools');
  },

  async getRuntimeConfig(): Promise<RuntimeConfigDto> {
    return invoke<RuntimeConfigDto>('get_runtime_config');
  },
};
