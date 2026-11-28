/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { ApprovalMode, AuthType, makeFakeConfig } from '@m31a/core';
import { renderWithProviders } from '../../test-utils/render.js';
import { createMockSettings } from '../../test-utils/settings.js';
import { waitFor } from '../../test-utils/async.js';
import { StartScreen, STARTER_ACTIONS } from './StartScreen.js';
import type { UIState } from '../contexts/UIStateContext.js';
import type { UIActions } from '../contexts/UIActionsContext.js';

describe('<StartScreen />', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders hero, Plan/Act mode bar, workspace metadata, and quick-start actions', async () => {
    const config = makeFakeConfig({
      model: 'qwen/qwen3.5-122b-a10b',
      sandbox: { enabled: true, command: 'docker', image: 'test' },
    });
    vi.spyOn(config, 'getTargetDir').mockReturnValue(
      '/home/user/workspace/demo-app',
    );
    vi.spyOn(config, 'getThinkingEffort').mockReturnValue('high');
    vi.spyOn(config, 'getContentGeneratorConfig').mockReturnValue({
      authType: AuthType.USE_NVIDIA_NIM,
      apiKey: 'nvapi-test-key',
    } as ReturnType<typeof config.getContentGeneratorConfig>);
    vi.spyOn(config, 'getUserTierName').mockReturnValue('Pro');
    vi.spyOn(config, 'getMcpClientManager').mockReturnValue({
      getMcpServers: () => ({ github: {}, postgres: {} }),
    } as unknown as ReturnType<typeof config.getMcpClientManager>);
    vi.spyOn(config, 'getSkillManager').mockReturnValue({
      getDisplayableSkills: () => [{ name: 'gravitas' }],
    } as unknown as ReturnType<typeof config.getSkillManager>);

    const { lastFrame, unmount } = await renderWithProviders(
      <StartScreen version="1.2.3" />,
      {
        config,
        uiState: {
          branchName: 'feat/start-screen',
          currentModel: 'qwen/qwen3.5-122b-a10b',
          m31aMdFileCount: 2,
          showApprovalModeIndicator: ApprovalMode.DEFAULT,
          allowPlanMode: true,
          terminalWidth: 100,
          mainAreaWidth: 100,
        } as Partial<UIState>,
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('M31A v1.2.3');
    expect(frame).toContain('What can I help you build today?');
    expect(frame).toContain('[● Act]');
    expect(frame).toContain('Auto-Edit');
    expect(frame).toContain('Plan');
    expect(frame).toContain('Workspace');
    expect(frame).toContain('feat/start-screen');
    expect(frame).toContain('2 M31A.md · 2 MCP · 1 skills');
    expect(frame).toContain('qwen/qwen3.5-122b-a10b');
    expect(frame).toContain('high');
    expect(frame).toContain('NVIDIA NIM (key ✓)');
    expect(frame).toContain('Pro · sandbox');
    expect(frame).toContain('Quick start');
    expect(frame).toContain('[Alt+1] Explore codebase');
    expect(frame).toContain('[Alt+4] Find & fix a bug');
    unmount();
  });

  it.each([
    { mode: ApprovalMode.PLAN, expected: '[● Plan]' },
    { mode: ApprovalMode.AUTO_EDIT, expected: '[● Auto-Edit]' },
    { mode: ApprovalMode.YOLO, expected: '[● ⚡ YOLO]' },
  ])('highlights active approval mode $mode', async ({ mode, expected }) => {
    const { lastFrame, unmount } = await renderWithProviders(
      <StartScreen version="1.2.3" />,
      {
        uiState: {
          showApprovalModeIndicator: mode,
          allowPlanMode: true,
          terminalWidth: 90,
          mainAreaWidth: 90,
        } as Partial<UIState>,
      },
    );

    expect(lastFrame()).toContain(expected);
    unmount();
  });

  it('triggers quick-start submission via Alt+1..4 shortcuts', async () => {
    const handleFinalSubmit = vi.fn().mockResolvedValue(undefined);
    const { stdin, unmount } = await renderWithProviders(
      <StartScreen version="1.2.3" />,
      {
        uiState: {
          dialogsVisible: false,
          terminalWidth: 100,
          mainAreaWidth: 100,
        } as Partial<UIState>,
        uiActions: {
          handleFinalSubmit,
        } as Partial<UIActions>,
      },
    );

    await act(async () => {
      stdin.write('\x1b1');
    });
    await waitFor(() => {
      expect(handleFinalSubmit).toHaveBeenCalledWith(STARTER_ACTIONS[0].prompt);
    });

    await act(async () => {
      stdin.write('\x1b3');
    });
    await waitFor(() => {
      expect(handleFinalSubmit).toHaveBeenCalledWith(STARTER_ACTIONS[2].prompt);
    });

    unmount();
  });

  it('ignores Alt+1..4 shortcuts when dialogs are visible', async () => {
    const handleFinalSubmit = vi.fn().mockResolvedValue(undefined);
    const { stdin, unmount } = await renderWithProviders(
      <StartScreen version="1.2.3" />,
      {
        uiState: {
          dialogsVisible: true,
          terminalWidth: 100,
          mainAreaWidth: 100,
        } as Partial<UIState>,
        uiActions: {
          handleFinalSubmit,
        } as Partial<UIActions>,
      },
    );

    await act(async () => {
      stdin.write('\x1b1');
    });
    await new Promise((resolve) => setTimeout(resolve, 60));

    expect(handleFinalSubmit).not.toHaveBeenCalled();
    unmount();
  });

  it('respects hideBanner and minimal showDetails=false modes', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <StartScreen version="1.2.3" showDetails={false} />,
      {
        settings: createMockSettings({
          ui: { hideBanner: true },
        }),
        uiState: {
          showApprovalModeIndicator: ApprovalMode.DEFAULT,
          terminalWidth: 90,
          mainAreaWidth: 90,
        } as Partial<UIState>,
      },
    );

    const frame = lastFrame();
    expect(frame).not.toContain('M31A v1.2.3');
    expect(frame).not.toContain('Quick start');
    expect(frame).toContain('[● Act]');
    unmount();
  });

  it('adapts layout cleanly on compact terminal widths', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <StartScreen version="1.2.3" />,
      {
        uiState: {
          terminalWidth: 48,
          mainAreaWidth: 48,
          branchName: 'main',
        } as Partial<UIState>,
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('M31A v1.2.3');
    expect(frame).toContain('What can I help you build today?');
    expect(frame).toContain('[Alt+1] Explore codebase');
    expect(frame).not.toContain('Summarize architecture and key entry points');
    unmount();
  });
});
