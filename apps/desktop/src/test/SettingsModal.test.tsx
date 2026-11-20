import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { AppProvider } from '../stores/AppContext';
import { SettingsModal } from '../features/settings/SettingsModal';
import { invoke } from '@tauri-apps/api/core';

describe('SettingsModal Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const setupMocks = () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_runtime_status') {
        return {
          state: 'Ready',
          active_model: 'claude-3-5-sonnet',
          active_workspace: '/workspace/m31a',
          is_operational: true,
          is_terminal: false,
          active_session_id: 'sess-abc-123',
        };
      }
      if (cmd === 'get_environment_info') {
        return {
          app_version: '0.1.15',
          target_platform: 'linux',
          target_arch: 'x86_64',
          features: ['desktop-ui', 'mcp-support'],
        };
      }
      if (cmd === 'get_active_workspace') {
        return {
          path: '/workspace/m31a',
          is_trusted: true,
          exists: true,
        };
      }
      if (cmd === 'list_available_models') {
        return [
          {
            id: 'claude-3-5-sonnet',
            name: 'Claude 3.5 Sonnet',
            originator: 'Anthropic',
            context_window: 200000,
            supports_tools: true,
            supports_thinking: true,
            supports_vision: true,
          },
        ];
      }
      if (cmd === 'list_sessions') return [];
      if (cmd === 'get_pending_interactions') {
        return { approvals: [], questions: [] };
      }
      if (cmd === 'list_registered_tools') {
        return [
          {
            name: 'read_file',
            description: 'Reads contents of a file',
            parameters_schema: '{"type": "object", "properties": {"path": {"type": "string"}}}',
          },
          {
            name: 'write_file',
            description: 'Writes contents to a file',
            parameters_schema: '{"type": "object", "properties": {"path": {"type": "string"}}}',
          },
        ];
      }
      if (cmd === 'get_runtime_config') {
        return {
          model: 'claude-3-5-sonnet',
          api_base: 'https://api.anthropic.com',
          sandbox_policy: 'workspace-write',
          approval_policy: 'on-request',
          active_features: ['desktop-ui'],
          dangerously_bypass: false,
        };
      }
      return null;
    });
  };

  it('does not render when isOpen is false', async () => {
    setupMocks();
    await act(async () => {
      render(
        <AppProvider>
          <SettingsModal isOpen={false} onClose={vi.fn()} />
        </AppProvider>
      );
    });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('renders tabs and general settings when open', async () => {
    setupMocks();
    const onClose = vi.fn();

    await act(async () => {
      render(
        <AppProvider>
          <SettingsModal isOpen={true} onClose={onClose} />
        </AppProvider>
      );
    });

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('M31A Runtime Settings & Architecture')).toBeInTheDocument();
    expect(screen.getByText('System Environment')).toBeInTheDocument();
    expect(screen.getByText('M31A Version:')).toBeInTheDocument();
    expect(screen.getByText('Authoritative Trust Granted')).toBeInTheDocument();

    const closeBtn = screen.getByRole('button', { name: /Close settings/i });
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });

  it('navigates tabs and displays security, tools, and diagnostics', async () => {
    setupMocks();

    await act(async () => {
      render(
        <AppProvider>
          <SettingsModal isOpen={true} onClose={vi.fn()} />
        </AppProvider>
      );
    });

    // Switch to Model tab
    const modelTab = screen.getByRole('tab', { name: /Model & Engine/i });
    fireEvent.click(modelTab);
    expect(screen.getByText('Model Configuration')).toBeInTheDocument();
    expect(screen.getByText(/Claude 3.5 Sonnet/)).toBeInTheDocument();

    // Switch to Security tab
    const secTab = screen.getByRole('tab', { name: /Security & Policies/i });
    fireEvent.click(secTab);
    expect(screen.getByText('Execution Governance & Security Policies')).toBeInTheDocument();
    expect(screen.getByText('Sandbox Policy')).toBeInTheDocument();
    expect(screen.getByText('Approval Policy')).toBeInTheDocument();

    // Switch to Tools tab
    const toolsTab = screen.getByRole('tab', { name: /Tool Catalog/i });
    fireEvent.click(toolsTab);
    expect(screen.getByText('Registered Tool Catalog')).toBeInTheDocument();
    expect(screen.getByText('read_file')).toBeInTheDocument();
    expect(screen.getByText('write_file')).toBeInTheDocument();

    // Expand schema
    const viewSchemaBtn = screen.getAllByRole('button', { name: /View Schema/i })[0];
    fireEvent.click(viewSchemaBtn);
    expect(screen.getByText(/properties/)).toBeInTheDocument();

    // Filter tools
    const filterInput = screen.getByLabelText('Filter registered tools');
    fireEvent.change(filterInput, { target: { value: 'write' } });
    expect(screen.queryByText('read_file')).not.toBeInTheDocument();
    expect(screen.getByText('write_file')).toBeInTheDocument();

    // Switch to Diagnostics tab
    const diagTab = screen.getByRole('tab', { name: /Diagnostics/i });
    fireEvent.click(diagTab);
    expect(screen.getByText('Runtime Diagnostics & State')).toBeInTheDocument();
    expect(screen.getByText('Operational (Ready for turns)')).toBeInTheDocument();
  });
});
