/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { renderWithProviders } from '../../test-utils/render.js';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { SessionSummaryDisplay } from './SessionSummaryDisplay.js';
import * as SessionContext from '../contexts/SessionContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { type SessionMetrics } from '../contexts/SessionContext.js';
import { ToolCallDecision, isWindows, type WorktreeSettings } from '@m31a/core';

vi.mock('@m31a/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@m31a/core')>();
  return {
    ...actual,
    isWindows: vi.fn(),
  };
});

vi.mock('../contexts/SessionContext.js', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('../contexts/SessionContext.js')>();
  return {
    ...actual,
    useSessionStats: vi.fn(),
  };
});

vi.mock('../contexts/ConfigContext.js', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('../contexts/ConfigContext.js')>();
  return {
    ...actual,
    useConfig: vi.fn(),
  };
});

const isWindowsMock = vi.mocked(isWindows);
const useSessionStatsMock = vi.mocked(SessionContext.useSessionStats);

const renderWithMockedStats = async (
  metrics: SessionMetrics,
  sessionId = 'test-session',
  worktreeSettings?: WorktreeSettings,
) => {
  useSessionStatsMock.mockReturnValue({
    stats: {
      sessionId,
      sessionStartTime: new Date(),
      metrics,
      lastPromptTokenCount: 0,
      promptCount: 5,
    },

    getPromptCount: () => 5,
    startNewPrompt: vi.fn(),
  } as unknown as ReturnType<typeof SessionContext.useSessionStats>);

  vi.mocked(useConfig).mockReturnValue({
    getWorktreeSettings: () => worktreeSettings,
  } as never);

  const result = await renderWithProviders(
    <SessionSummaryDisplay duration="1h 23m 45s" />,
    {
      width: 100,
    },
  );
  await result.waitUntilReady();
  return result;
};

describe('<SessionSummaryDisplay />', () => {
  const emptyMetrics: SessionMetrics = {
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
        [ToolCallDecision.AUTO_ACCEPT]: 0,
      },
      byName: {},
    },
    files: {
      totalLinesAdded: 0,
      totalLinesRemoved: 0,
    },
  };

  beforeEach(() => {
    isWindowsMock.mockReturnValue(false);
  });

  it('renders the summary display with a title', async () => {
    const metrics: SessionMetrics = {
      ...emptyMetrics,
      models: {
        'gemini-2.5-pro': {
          api: { totalRequests: 10, totalErrors: 1, totalLatencyMs: 50234 },
          tokens: {
            input: 500,
            prompt: 1000,
            candidates: 2000,
            total: 3500,
            cached: 500,
            thoughts: 300,
            tool: 200,
          },
          roles: {},
        },
      },
      files: {
        totalLinesAdded: 42,
        totalLinesRemoved: 15,
      },
    };

    const { lastFrame, unmount } = await renderWithMockedStats(metrics);
    const output = lastFrame();

    expect(output).toContain('Agent powering down. Goodbye!');
    expect(output).toContain('Total Tokens:');
    expect(output).toContain('thinking 300');
    expect(output).toContain('cache 500 (50.0% hit)');
    expect(output).not.toContain('Session ID:');
    expect(output).not.toContain('Success Rate:');
    expect(output).not.toContain('Use /model to switch');
    expect(output).toMatchSnapshot();
    unmount();
  });

  it('omits zero tool stats and empty model table for quiet exits', async () => {
    const { lastFrame, unmount } = await renderWithMockedStats(emptyMetrics);
    const output = lastFrame();

    expect(output).toContain('Duration:');
    expect(output).toContain('Turns:');
    expect(output).toContain('m31a --resume test-session');
    expect(output).not.toContain('Session ID:');
    expect(output).not.toContain('Tools:');
    expect(output).not.toContain('Success Rate:');
    expect(output).not.toContain('Total Tokens:');
    expect(output).not.toContain('Use /model');
    unmount();
  });

  it('shows tools, user agreement, and aggregated thinking/token totals across models', async () => {
    const metrics: SessionMetrics = {
      models: {
        'model-a': {
          api: { totalRequests: 2, totalErrors: 0, totalLatencyMs: 1200 },
          tokens: {
            input: 400,
            prompt: 400,
            candidates: 150,
            total: 750,
            cached: 100,
            thoughts: 200,
            tool: 0,
          },
          roles: {},
        },
        'model-b': {
          api: { totalRequests: 1, totalErrors: 0, totalLatencyMs: 800 },
          tokens: {
            input: 600,
            prompt: 600,
            candidates: 350,
            total: 1050,
            cached: 0,
            thoughts: 100,
            tool: 0,
          },
          roles: {},
        },
      },
      tools: {
        totalCalls: 4,
        totalSuccess: 3,
        totalFail: 1,
        totalDurationMs: 2500,
        totalDecisions: {
          accept: 2,
          reject: 1,
          modify: 0,
          [ToolCallDecision.AUTO_ACCEPT]: 1,
        },
        byName: {},
      },
      files: {
        totalLinesAdded: 12,
        totalLinesRemoved: 4,
      },
    };

    const { lastFrame, unmount } = await renderWithMockedStats(metrics);
    const output = lastFrame();

    expect(output).toContain('Tools:');
    expect(output).toContain('75.0%');
    expect(output).toContain('User Agreement:');
    expect(output).toContain('4 reviewed');
    expect(output).toContain('Total Tokens:');
    expect(output).toContain('1,800');
    expect(output).toContain(
      'in 1,000 · out 500 · thinking 300 · cache 100 (10.0% hit)',
    );
    expect(output).toContain('model-a');
    expect(output).toContain('model-b');
    unmount();
  });

  describe('Session ID escaping', () => {
    it('renders a standard UUID-formatted session ID in the footer (bash)', async () => {
      const uuidSessionId = '1234-abcd-5678-efgh';
      const { lastFrame, unmount } = await renderWithMockedStats(
        emptyMetrics,
        uuidSessionId,
      );
      const output = lastFrame();

      // Standard UUID characters are NOT wrapped in double quotes on non-Windows.
      expect(output).toContain('m31a --resume 1234-abcd-5678-efgh');
      unmount();
    });

    it('sanitizes a malicious session ID in the footer (bash)', async () => {
      const maliciousSessionId = "'; rm -rf / #";
      const { lastFrame, unmount } = await renderWithMockedStats(
        emptyMetrics,
        maliciousSessionId,
      );
      const output = lastFrame();

      // escapeShellArg (using shell-quote for bash) will wrap special characters in double quotes.
      expect(output).toContain('m31a --resume "\'; rm -rf / #"');
      unmount();
    });

    it('renders a standard UUID-formatted session ID in the footer (powershell) on Windows', async () => {
      isWindowsMock.mockReturnValue(true);

      const uuidSessionId = '1234-abcd-5678-efgh';
      const { lastFrame, unmount } = await renderWithMockedStats(
        emptyMetrics,
        uuidSessionId,
      );
      const output = lastFrame();

      // PowerShell doesn't wrap UUID in quotes by default, but we wrap it in double quotes on Windows.
      expect(output).toContain('m31a --resume "1234-abcd-5678-efgh"');
      unmount();
    });

    it('sanitizes a malicious session ID in the footer (powershell)', async () => {
      isWindowsMock.mockReturnValue(true);

      const maliciousSessionId = "'; rm -rf / #";
      const { lastFrame, unmount } = await renderWithMockedStats(
        emptyMetrics,
        maliciousSessionId,
      );
      const output = lastFrame();

      // PowerShell wraps in single quotes and escapes internal single quotes by doubling them.
      // Since it's already quoted, we don't add redundant double quotes.
      expect(output).toContain("m31a --resume '''; rm -rf / #'");
      unmount();
    });
  });

  describe('Worktree status', () => {
    it('renders worktree instructions when worktreeSettings are present', async () => {
      const worktreeSettings: WorktreeSettings = {
        name: 'foo-bar',
        path: '/path/to/foo-bar',
        baseSha: 'base-sha',
      };

      const { lastFrame, unmount } = await renderWithMockedStats(
        emptyMetrics,
        'test-session',
        worktreeSettings,
      );
      const output = lastFrame();

      expect(output).toContain('To resume work in this worktree:');
      expect(output).toContain(
        'cd /path/to/foo-bar && m31a --resume test-session',
      );
      expect(output).toContain(
        'To remove manually: git worktree remove /path/to/foo-bar',
      );
      unmount();
    });
  });
});
