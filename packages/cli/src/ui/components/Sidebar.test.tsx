/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useIsScreenReaderEnabled } from 'ink';
import { renderWithProviders } from '../../test-utils/render.js';
import { Sidebar } from './Sidebar.js';
import { createMockSettings } from '../../test-utils/settings.js';
import { AuthType, type Config } from '@m31a/core';

vi.mock('ink', async (importOriginal) => {
  const actual = await importOriginal<typeof import('ink')>();
  return {
    ...actual,
    useIsScreenReaderEnabled: vi.fn(() => false),
  };
});

const mockConfig = {
  getTargetDir: () => '/home/user/workspace/M31A',
  getDebugMode: () => false,
  getModel: () => 'nvidia/nemotron-3-ultra-550b-a55b',
  getIdeMode: () => false,
  isTrustedFolder: () => true,
  getExtensionRegistryURI: () => undefined,
  getContentGeneratorConfig: () => ({ authType: AuthType.USE_NVIDIA_NIM }),
  getSandboxEnabled: () => false,
  getSessionId: () => '12345678-abcd-ef00',
} as unknown as Config;

describe('<Sidebar />', () => {
  beforeEach(() => {
    vi.mocked(useIsScreenReaderEnabled).mockReturnValue(false);
  });

  it('renders full-height vertical M31A sidebar with workspace telemetry', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <Sidebar width={30} />,
      {
        config: mockConfig,
        settings: createMockSettings({}),
        uiState: {
          currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
          branchName: 'main',
          history: [{ id: 1, type: 'user', text: 'hi' }],
          sessionStats: {
            sessionId: '12345678-abcd-ef00',
            sessionStartTime: new Date(),
            promptCount: 1,
            lastPromptTokenCount: 1200,
            metrics: {
              models: {},
              tools: {
                totalCalls: 0,
                totalSuccess: 0,
                totalFail: 0,
                totalDurationMs: 0,
                totalDecisions: {
                  accept: 0,
                  reject: 0,
                  modify: 0,
                  auto_accept: 0,
                },
                byName: {},
              },
              files: { totalLinesAdded: 0, totalLinesRemoved: 0 },
            },
          },
        },
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('M31A');
    expect(frame).toContain('NIM');
    expect(frame).toContain('main');
    expect(frame).toContain('turns: 1');
    expect(frame).toContain('Alt+S hide');
    expect(frame).toContain('/model');
    unmount();
  });

  it('renders compact sidebar without bottom shortcut footer', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <Sidebar width={30} compact />,
      {
        config: mockConfig,
        settings: createMockSettings({}),
        uiState: {
          currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
          branchName: 'main',
          history: [],
        },
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('M31A');
    expect(frame).not.toContain('turns:');
    unmount();
  });

  it('falls back to showing NIM model, session ID, and context usage when omitted from footer items', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <Sidebar width={32} />,
      {
        config: mockConfig,
        settings: createMockSettings({
          ui: { footer: { items: ['workspace'] } },
        }),
        uiState: {
          currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
          history: [{ id: 1, type: 'user', text: 'hi' }],
          sessionStats: {
            sessionId: '12345678-abcd-ef00',
            sessionStartTime: new Date(),
            promptCount: 1,
            lastPromptTokenCount: 1200,
            metrics: {
              models: {},
              tools: {
                totalCalls: 0,
                totalSuccess: 0,
                totalFail: 0,
                totalDurationMs: 0,
                totalDecisions: {
                  accept: 0,
                  reject: 0,
                  modify: 0,
                  auto_accept: 0,
                },
                byName: {},
              },
              files: { totalLinesAdded: 0, totalLinesRemoved: 0 },
            },
          },
        },
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('nemotron');
    expect(frame).toContain('ses:12345678');
    expect(frame).toContain('%');
    unmount();
  });

  it('returns null when screen reader is enabled', async () => {
    vi.mocked(useIsScreenReaderEnabled).mockReturnValue(true);
    const { lastFrame, unmount } = await renderWithProviders(
      <Sidebar width={30} />,
      {
        config: mockConfig,
        settings: createMockSettings({}),
      },
    );

    expect(lastFrame({ allowEmpty: true })).toBe('');
    unmount();
  });

  it('shows NVIDIA NIM key status without leaking key and runtime mode badges', async () => {
    vi.stubEnv('NVIDIA_API_KEY', 'nvapi-super-secret-token');
    vi.stubEnv('M31A_REDUCED_MOTION', '1');
    const { lastFrame, unmount } = await renderWithProviders(
      <Sidebar width={32} />,
      {
        config: mockConfig,
        settings: createMockSettings({}),
        inputState: {
          shellModeActive: true,
          copyModeEnabled: true,
        },
        uiState: {
          currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
          history: [],
        },
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('NIM · key ✓');
    expect(frame).not.toContain('nvapi-super-secret');
    expect(frame).toContain('[! shell]');
    expect(frame).toContain('[copy]');
    expect(frame).toContain('[reduced-motion]');
    vi.unstubAllEnvs();
    unmount();
  });

  it('always shows total token count in both full and compact modes, plus thinking tokens when > 0', async () => {
    const models = {
      'nvidia/nemotron-3-ultra-550b-a55b': {
        api: { totalRequests: 2, totalErrors: 0, totalLatencyMs: 500 },
        tokens: {
          input: 800,
          prompt: 800,
          candidates: 700,
          total: 1500,
          cached: 0,
          thoughts: 320,
          tool: 0,
        },
        roles: {},
      },
    };
    const sessionStats = {
      sessionId: '12345678-abcd-ef00',
      sessionStartTime: new Date(),
      promptCount: 2,
      lastPromptTokenCount: 800,
      metrics: {
        models,
        tools: {
          totalCalls: 0,
          totalSuccess: 0,
          totalFail: 0,
          totalDurationMs: 0,
          totalDecisions: {
            accept: 0,
            reject: 0,
            modify: 0,
            auto_accept: 0,
          },
          byName: {},
        },
        files: { totalLinesAdded: 0, totalLinesRemoved: 0 },
      },
    };

    const fullRender = await renderWithProviders(<Sidebar width={30} />, {
      config: mockConfig,
      settings: createMockSettings({}),
      uiState: {
        currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
        history: [],
        sessionStats,
      },
    });
    expect(fullRender.lastFrame()).toContain('1.5k tok');
    expect(fullRender.lastFrame()).toContain('think:');
    expect(fullRender.lastFrame()).toContain('320 tok');
    fullRender.unmount();

    const compactRender = await renderWithProviders(
      <Sidebar width={30} compact />,
      {
        config: mockConfig,
        settings: createMockSettings({}),
        uiState: {
          currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
          history: [],
          sessionStats,
        },
      },
    );
    expect(compactRender.lastFrame()).toContain('tokens:');
    expect(compactRender.lastFrame()).toContain('1.5k tok');
    expect(compactRender.lastFrame()).toContain('think:');
    expect(compactRender.lastFrame()).toContain('320 tok');
    compactRender.unmount();
  });

  it('shows 0 tok in compact mode when no tokens have been used yet and omits think row', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <Sidebar width={30} compact />,
      {
        config: mockConfig,
        settings: createMockSettings({}),
        uiState: {
          currentModel: 'nvidia/nemotron-3-ultra-550b-a55b',
          history: [],
        },
      },
    );

    const frame = lastFrame();
    expect(frame).toContain('0 tok');
    expect(frame).not.toContain('think:');
    unmount();
  });
});
