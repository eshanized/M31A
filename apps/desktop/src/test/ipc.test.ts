import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { ipc } from '../lib/ipc';

describe('Tauri IPC Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes get_runtime_status command correctly', async () => {
    const mockStatus = {
      state: 'Ready',
      active_model: 'claude-3-5-sonnet',
      active_workspace: '/test/workspace',
      is_operational: true,
      is_terminal: false,
      active_session_id: 'session-123',
    };
    vi.mocked(invoke).mockResolvedValueOnce(mockStatus);

    const result = await ipc.getRuntimeStatus();
    expect(invoke).toHaveBeenCalledWith('get_runtime_status');
    expect(result).toEqual(mockStatus);
  });

  it('invokes get_active_workspace command', async () => {
    const mockWs = {
      path: '/test/workspace',
      is_trusted: true,
      exists: true,
    };
    vi.mocked(invoke).mockResolvedValueOnce(mockWs);

    const result = await ipc.getActiveWorkspace();
    expect(invoke).toHaveBeenCalledWith('get_active_workspace');
    expect(result.is_trusted).toBe(true);
  });

  it('invokes set_workspace command with parameters', async () => {
    const mockWs = {
      path: '/test/new-workspace',
      is_trusted: false,
      exists: true,
    };
    vi.mocked(invoke).mockResolvedValueOnce(mockWs);

    const result = await ipc.setWorkspace('/test/new-workspace');
    expect(invoke).toHaveBeenCalledWith('set_workspace', {
      path: '/test/new-workspace',
    });
    expect(result.path).toBe('/test/new-workspace');
  });

  it('uses string contracts for active model operations', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('nvidia/model-a');
    expect(await ipc.getActiveModel()).toBe('nvidia/model-a');
    expect(invoke).toHaveBeenCalledWith('get_active_model');

    vi.mocked(invoke).mockResolvedValueOnce('nvidia/model-b');
    expect(await ipc.activateModel('nvidia/model-b')).toBe('nvidia/model-b');
    expect(invoke).toHaveBeenCalledWith('activate_model', {
      model: 'nvidia/model-b',
    });
  });

  it('retrieves pending approvals and questions for event reconciliation', async () => {
    const pending = {
      approvals: [{
        request_id: 'approval-1',
        thread_id: 'thread-1',
        turn_id: 'turn-1',
        command: 'cargo test',
        reason: 'Execution requires approval',
      }],
      questions: [{
        question_id: 'question-1',
        thread_id: 'thread-1',
        turn_id: 'turn-1',
        question: 'Continue?',
        options: ['Yes', 'No'],
        multi_select: false,
      }],
    };
    vi.mocked(invoke).mockResolvedValueOnce(pending);

    expect(await ipc.getPendingInteractions()).toEqual(pending);
    expect(invoke).toHaveBeenCalledWith('get_pending_interactions');
  });

  it('invokes authorize_workspace with a native-confirmation request', async () => {
    const resultValue = {
      path: '/test/workspace',
      is_trusted: true,
      exists: true,
    };
    vi.mocked(invoke).mockResolvedValueOnce(resultValue);

    expect(await ipc.authorizeWorkspace('/test/workspace')).toEqual(resultValue);
    expect(invoke).toHaveBeenCalledWith('authorize_workspace', {
      path: '/test/workspace',
    });
  });

  it('invokes cancel_turn with the exact runtime turn identifier', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await ipc.cancelTurn('thread-1', 'turn-runtime-1');
    expect(invoke).toHaveBeenCalledWith('cancel_turn', {
      threadId: 'thread-1',
      turnId: 'turn-runtime-1',
    });
  });

  it('invokes steer_turn with the exact runtime turn identifier', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await ipc.steerTurn('thread-1', 'turn-runtime-1', 'Continue carefully');
    expect(invoke).toHaveBeenCalledWith('steer_turn', {
      threadId: 'thread-1',
      turnId: 'turn-runtime-1',
      message: 'Continue carefully',
    });
  });

  it('invokes submit_turn with prompt payload', async () => {
    const mockHandle = {
      thread_id: 'thread-1',
      turn_id: 'turn-1',
    };
    vi.mocked(invoke).mockResolvedValueOnce(mockHandle);

    const result = await ipc.submitTurn({
      thread_id: 'thread-1',
      prompt: 'Refactor parser',
      collaboration_mode: 'plan',
    });

    expect(invoke).toHaveBeenCalledWith('submit_turn', {
      payload: {
        thread_id: 'thread-1',
        prompt: 'Refactor parser',
        collaboration_mode: 'plan',
      },
    });
    expect(result.turn_id).toBe('turn-1');
  });

  it('invokes respond_approval with decision and reason', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await ipc.respondApproval('req-1', false, 'Disallowed command');

    expect(invoke).toHaveBeenCalledWith('respond_approval', {
      requestId: 'req-1',
      decision: false,
      reason: 'Disallowed command',
    });
  });

  it('invokes respond_question with selected answers', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await ipc.respondQuestion('q-1', ['Option A', 'Option B']);

    expect(invoke).toHaveBeenCalledWith('respond_question', {
      questionId: 'q-1',
      answers: ['Option A', 'Option B'],
    });
  });

  it('invokes workspace exploration commands', async () => {
    const mockFiles = [
      { name: 'src', path: 'src', is_dir: true, size: 0 },
      { name: 'Cargo.toml', path: 'Cargo.toml', is_dir: false, size: 120 },
    ];
    vi.mocked(invoke).mockResolvedValueOnce(mockFiles);
    expect(await ipc.listWorkspaceFiles('src')).toEqual(mockFiles);
    expect(invoke).toHaveBeenCalledWith('list_workspace_files', { subpath: 'src' });

    vi.mocked(invoke).mockResolvedValueOnce('file contents');
    expect(await ipc.readWorkspaceFilePreview('src/main.rs')).toBe('file contents');
    expect(invoke).toHaveBeenCalledWith('read_workspace_file_preview', { path: 'src/main.rs' });
  });

  it('invokes session lifecycle operations: rename, delete, fork, export', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await ipc.renameSession('sess-1', 'Renamed Title');
    expect(invoke).toHaveBeenCalledWith('rename_session', { sessionId: 'sess-1', newTitle: 'Renamed Title' });

    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await ipc.deleteSession('sess-1');
    expect(invoke).toHaveBeenCalledWith('delete_session', { sessionId: 'sess-1' });

    const forked = {
      id: 'sess-2',
      title: 'Forked Title',
      model: 'model-a',
      cwd: '/workspace',
      created_at: 1000,
      updated_at: 1000,
      is_active: true,
    };
    vi.mocked(invoke).mockResolvedValueOnce(forked);
    expect(await ipc.forkSession('sess-1', 'Forked Title')).toEqual(forked);
    expect(invoke).toHaveBeenCalledWith('fork_session', { sessionId: 'sess-1', newTitle: 'Forked Title' });

    vi.mocked(invoke).mockResolvedValueOnce('# Transcript');
    expect(await ipc.exportSessionTranscript('sess-1')).toBe('# Transcript');
    expect(invoke).toHaveBeenCalledWith('export_session_transcript', { sessionId: 'sess-1' });
  });

  it('invokes tool catalog and config commands', async () => {
    const mockTools = [{ name: 'read_file', description: 'Reads file', parameters_schema: null }];
    vi.mocked(invoke).mockResolvedValueOnce(mockTools);
    expect(await ipc.listRegisteredTools()).toEqual(mockTools);
    expect(invoke).toHaveBeenCalledWith('list_registered_tools');

    const mockConfig = {
      model: 'test-model',
      api_base: 'https://test',
      sandbox_policy: 'WorkspaceWrite',
      approval_policy: 'OnRequest',
      active_features: ['guardian'],
      dangerously_bypass: false,
    };
    vi.mocked(invoke).mockResolvedValueOnce(mockConfig);
    expect(await ipc.getRuntimeConfig()).toEqual(mockConfig);
    expect(invoke).toHaveBeenCalledWith('get_runtime_config');
  });
});
