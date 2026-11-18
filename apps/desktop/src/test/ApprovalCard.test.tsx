import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { AppProvider } from '../stores/AppContext';
import { ApprovalCard } from '../features/approvals/ApprovalCard';
import { invoke } from '@tauri-apps/api/core';

describe('ApprovalCard Component', () => {
  it('renders command, reason, and triggers allow once decision', async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_runtime_status') {
        return {
          state: 'Ready',
          active_model: 'test-model',
          active_workspace: '/test',
          is_operational: true,
          is_terminal: false,
        };
      }
      if (cmd === 'get_environment_info') {
        return {
          app_version: '0.1.15',
          target_platform: 'linux',
          target_arch: 'x86_64',
          features: [],
        };
      }
      if (cmd === 'get_active_workspace') {
        return { path: '/test', is_trusted: true, exists: true };
      }
      if (cmd === 'list_available_models') return [];
      if (cmd === 'list_sessions') return [];
      if (cmd === 'respond_approval') return undefined;
      return null;
    });

    const approval = {
      request_id: 'req-12345678',
      thread_id: 'thread-1',
      turn_id: 'turn-1',
      command: 'rm -rf /tmp/test',
      reason: 'Removal of build directory',
    };

    await act(async () => {
      render(
        <AppProvider>
          <ApprovalCard approval={approval} />
        </AppProvider>
      );
    });

    expect(screen.getByText('Authorization Required')).toBeInTheDocument();
    expect(screen.getByText('rm -rf /tmp/test')).toBeInTheDocument();
    expect(screen.getByText('Removal of build directory')).toBeInTheDocument();

    const allowBtn = screen.getByRole('button', { name: /allow once/i });
    await act(async () => {
      fireEvent.click(allowBtn);
    });

    expect(invoke).toHaveBeenCalledWith('respond_approval', {
      requestId: 'req-12345678',
      decision: true,
      reason: undefined,
    });
  });
});
