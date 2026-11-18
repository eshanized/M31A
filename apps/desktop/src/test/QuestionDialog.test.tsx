import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { AppProvider } from '../stores/AppContext';
import { QuestionDialog } from '../features/approvals/QuestionDialog';
import { invoke } from '@tauri-apps/api/core';

describe('QuestionDialog Component', () => {
  it('renders question options and submits user selection', async () => {
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
      if (cmd === 'respond_question') return undefined;
      return null;
    });

    const question = {
      question_id: 'q-100',
      thread_id: 'thread-1',
      turn_id: 'turn-1',
      question: 'Which test framework should we configure?',
      options: ['Vitest', 'Jest', 'Playwright'],
      multi_select: false,
    };

    await act(async () => {
      render(
        <AppProvider>
          <QuestionDialog question={question} />
        </AppProvider>
      );
    });

    expect(screen.getByText('Which test framework should we configure?')).toBeInTheDocument();
    expect(screen.getByText('Vitest')).toBeInTheDocument();
    expect(screen.getByText('Jest')).toBeInTheDocument();

    const vitestOption = screen.getByLabelText('Vitest');
    await act(async () => {
      fireEvent.click(vitestOption);
    });

    const submitBtn = screen.getByRole('button', { name: /submit response/i });
    await act(async () => {
      fireEvent.click(submitBtn);
    });

    expect(invoke).toHaveBeenCalledWith('respond_question', {
      questionId: 'q-100',
      answers: ['Vitest'],
    });
  });
});
