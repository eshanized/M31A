import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { AppProvider } from '../stores/AppContext';
import { Composer } from '../features/chat/Composer';
import { invoke } from '@tauri-apps/api/core';

describe('Composer Component with Agentic Slash Commands & File Mentions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const setupMocks = () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'get_runtime_status') {
        return {
          state: 'Ready',
          active_model: 'claude-3-5-sonnet',
          active_workspace: '/workspace/m31a',
          is_operational: true,
          is_terminal: false,
          active_session_id: 'sess-abc',
        };
      }
      if (cmd === 'get_environment_info') {
        return {
          app_version: '0.1.15',
          target_platform: 'linux',
          target_arch: 'x86_64',
          features: ['desktop-ui'],
        };
      }
      if (cmd === 'get_active_workspace') {
        return { path: '/workspace/m31a', is_trusted: true, exists: true };
      }
      if (cmd === 'list_available_models') return [];
      if (cmd === 'list_sessions') return [];
      if (cmd === 'get_pending_interactions') return { approvals: [], questions: [] };
      if (cmd === 'list_registered_tools') return [];
      if (cmd === 'get_runtime_config') {
        return {
          model: 'claude-3-5-sonnet',
          api_base: 'https://test',
          sandbox_policy: 'WorkspaceWrite',
          approval_policy: 'OnRequest',
          active_features: [],
          dangerously_bypass: false,
        };
      }
      if (cmd === 'list_slash_commands') {
        return [
          {
            name: '/help',
            description: 'Show keybindings and commands',
            usage: '/help',
            category: 'Core',
            subcommands: [],
          },
          {
            name: '/plan',
            description: 'Read-only planning mode',
            usage: '/plan',
            category: 'Mode',
            subcommands: [],
          },
          {
            name: '/review',
            description: 'Audit and review mode',
            usage: '/review',
            category: 'Mode',
            subcommands: [],
          },
          {
            name: '/clear',
            description: 'Clear messages in session',
            usage: '/clear',
            category: 'Core',
            subcommands: [],
          },
          {
            name: '/context',
            description: 'Manage session context window',
            usage: '/context [compact|refresh|status]',
            category: 'Context',
            subcommands: ['compact', 'refresh', 'status'],
          },
        ];
      }
      if (cmd === 'search_workspace_files') {
        const query = (args as { query?: string })?.query ?? '';
        const allFiles = [
          { path: 'Cargo.toml', filename: 'Cargo.toml', is_dir: false },
          { path: 'src/main.rs', filename: 'main.rs', is_dir: false },
          { path: 'apps/desktop/src/Composer.tsx', filename: 'Composer.tsx', is_dir: false },
          { path: 'crates/m31a-core', filename: 'm31a-core', is_dir: true },
        ];
        if (!query) return allFiles;
        return allFiles.filter((f) =>
          f.path.toLowerCase().includes(query.toLowerCase())
        );
      }
      if (cmd === 'submit_turn') {
        return { thread_id: 'sess-abc', turn_id: 'turn-xyz' };
      }
      return null;
    });
  };

  it('renders mode selectors and autocomplete trigger buttons', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <Composer />
        </AppProvider>
      );
    });

    expect(screen.getByText('Default')).toBeDefined();
    expect(screen.getByText('Plan')).toBeDefined();
    expect(screen.getByText('Review')).toBeDefined();
    expect(screen.getByTitle('Open Slash Commands autocomplete')).toBeDefined();
    expect(screen.getByTitle('Mention workspace file (@)')).toBeDefined();
  });

  it('shows slash command popup when typing / and filters results', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <Composer />
        </AppProvider>
      );
    });

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    // Type '/'
    await act(async () => {
      fireEvent.change(textarea, { target: { value: '/', selectionStart: 1 } });
    });

    expect(screen.getByTestId('slash-commands-popup')).toBeDefined();
    expect(screen.getByText('/help')).toBeDefined();
    expect(screen.getByText('/plan')).toBeDefined();

    // Type '/pl' to filter
    await act(async () => {
      fireEvent.change(textarea, { target: { value: '/pl', selectionStart: 3 } });
    });

    expect(screen.getByText('/plan')).toBeDefined();
    expect(screen.queryByText('/help')).toBeNull();
  });

  it('shows subcommands when top-level command is completed with space', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <Composer />
        </AppProvider>
      );
    });

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    // Type '/context '
    await act(async () => {
      fireEvent.change(textarea, { target: { value: '/context ', selectionStart: 9 } });
    });

    expect(screen.getByTestId('slash-commands-popup')).toBeDefined();
    expect(screen.getByText('/context compact')).toBeDefined();
    expect(screen.getByText('/context refresh')).toBeDefined();
  });

  it('shows file mention popup when typing @ and autocompletes file selection', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <Composer />
        </AppProvider>
      );
    });

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    // Type '@'
    await act(async () => {
      fireEvent.change(textarea, { target: { value: '@', selectionStart: 1 } });
    });

    // Check mention popup appears
    expect(screen.getByTestId('file-mentions-popup')).toBeDefined();
    expect(screen.getByText('Cargo.toml')).toBeDefined();

    // Select Cargo.toml by clicking it
    const item = screen.getByText('Cargo.toml');
    await act(async () => {
      fireEvent.mouseDown(item);
    });

    // Mention popup closes and input is populated with @Cargo.toml
    expect(screen.queryByTestId('file-mentions-popup')).toBeNull();
    expect(textarea.value).toBe('@Cargo.toml ');
  });

  it('navigates and selects slash command using Enter key', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <Composer />
        </AppProvider>
      );
    });

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    // Type '/help'
    await act(async () => {
      fireEvent.change(textarea, { target: { value: '/help', selectionStart: 5 } });
    });

    // Press Enter to select
    await act(async () => {
      fireEvent.keyDown(textarea, { key: 'Enter' });
    });

    // Filled with '/help '
    expect(textarea.value).toBe('/help ');
    expect(screen.queryByTestId('slash-commands-popup')).toBeNull();
  });

  it('dismisses autocomplete popups when pressing Escape', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <Composer />
        </AppProvider>
      );
    });

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    // Type '/'
    await act(async () => {
      fireEvent.change(textarea, { target: { value: '/', selectionStart: 1 } });
    });
    expect(screen.getByTestId('slash-commands-popup')).toBeDefined();

    // Press Escape
    await act(async () => {
      fireEvent.keyDown(textarea, { key: 'Escape' });
    });
    expect(screen.queryByTestId('slash-commands-popup')).toBeNull();
  });
});
