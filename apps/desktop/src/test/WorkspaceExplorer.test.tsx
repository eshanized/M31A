import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { AppProvider } from '../stores/AppContext';
import { WorkspaceExplorer } from '../features/workspace/WorkspaceExplorer';
import { invoke } from '@tauri-apps/api/core';

describe('WorkspaceExplorer Component', () => {
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
        return {
          path: '/workspace/m31a',
          is_trusted: true,
          exists: true,
        };
      }
      if (cmd === 'list_available_models') return [];
      if (cmd === 'list_sessions') return [];
      if (cmd === 'get_pending_interactions') {
        return { approvals: [], questions: [] };
      }
      if (cmd === 'list_workspace_files') {
        const payload = args as { subpath?: string };
        if (payload?.subpath === 'src') {
          return [
            { name: 'main.rs', path: 'src/main.rs', is_dir: false, size: 512 },
          ];
        }
        return [
          { name: 'src', path: 'src', is_dir: true, size: 0 },
          { name: 'Cargo.toml', path: 'Cargo.toml', is_dir: false, size: 1024 },
          { name: 'README.md', path: 'README.md', is_dir: false, size: 2048 },
        ];
      }
      if (cmd === 'read_workspace_file_preview') {
        const payload = args as { path: string };
        if (payload?.path === 'Cargo.toml') {
          return '[package]\nname = "m31a"\nversion = "0.1.15"';
        }
        return 'Sample file content';
      }
      return null;
    });
  };

  it('renders workspace files and handles file filtering', async () => {
    setupMocks();

    await act(async () => {
      render(
        <AppProvider>
          <WorkspaceExplorer />
        </AppProvider>
      );
    });

    expect(screen.getByText('Cargo.toml')).toBeInTheDocument();
    expect(screen.getByText('README.md')).toBeInTheDocument();
    expect(screen.getByText('src')).toBeInTheDocument();

    const searchInput = screen.getByLabelText('Filter workspace files');
    fireEvent.change(searchInput, { target: { value: 'Cargo' } });

    expect(screen.getByText('Cargo.toml')).toBeInTheDocument();
    expect(screen.queryByText('README.md')).not.toBeInTheDocument();
  });

  it('navigates directories and previews file content', async () => {
    setupMocks();

    await act(async () => {
      render(
        <AppProvider>
          <WorkspaceExplorer />
        </AppProvider>
      );
    });

    // Preview Cargo.toml
    const cargoFile = screen.getByText('Cargo.toml');
    await act(async () => {
      fireEvent.click(cargoFile);
    });

    expect(screen.getByText(/version = "0.1.15"/)).toBeInTheDocument();

    // Navigate into 'src' directory
    const srcDir = screen.getByText('src');
    await act(async () => {
      fireEvent.click(srcDir);
    });

    expect(screen.getByText('main.rs')).toBeInTheDocument();
  });
});
