/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { CheckerRunner } from './checker-runner.js';
import { ContextBuilder } from './context-builder.js';
import { CheckerRegistry } from './registry.js';
import {
  type InProcessCheckerConfig,
  InProcessCheckerType,
} from '../policy/types.js';
import { SafetyCheckDecision, type SafetyCheckResult } from './protocol.js';
import type { Config } from '../config/config.js';

// Mock dependencies
vi.mock('./registry.js');
vi.mock('./context-builder.js');

describe('CheckerRunner', () => {
  let runner: CheckerRunner;
  let mockContextBuilder: ContextBuilder;
  let mockRegistry: CheckerRegistry;

  const mockToolCall = { name: 'test_tool', args: {} };
  const mockInProcessConfig: InProcessCheckerConfig = {
    type: 'in-process',
    name: InProcessCheckerType.ALLOWED_PATH,
  };

  beforeEach(() => {
    mockContextBuilder = new ContextBuilder({} as Config);
    vi.spyOn(mockContextBuilder, 'config', 'get').mockReturnValue({
      env: {},
      getWorkingDir: vi.fn().mockReturnValue('/mock/cwd'),
    } as unknown as Config);
    mockRegistry = new CheckerRegistry();
    CheckerRegistry.prototype.resolveInProcess = vi.fn();

    runner = new CheckerRunner(mockContextBuilder, mockRegistry, {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should run in-process checker successfully', async () => {
    const mockResult: SafetyCheckResult = {
      decision: SafetyCheckDecision.ALLOW,
    };
    const mockChecker = {
      check: vi.fn().mockResolvedValue(mockResult),
    };
    vi.mocked(mockRegistry.resolveInProcess).mockReturnValue(mockChecker);
    vi.mocked(mockContextBuilder.buildFullContext).mockReturnValue({
      environment: { cwd: '/tmp', workspaces: [] },
    });

    const result = await runner.runChecker(mockToolCall, mockInProcessConfig);

    expect(result).toEqual(mockResult);
    expect(mockRegistry.resolveInProcess).toHaveBeenCalledWith(
      InProcessCheckerType.ALLOWED_PATH,
    );
    expect(mockChecker.check).toHaveBeenCalled();
  });

  it('should handle in-process checker errors', async () => {
    const mockChecker = {
      check: vi.fn().mockRejectedValue(new Error('Checker failed')),
    };
    vi.mocked(mockRegistry.resolveInProcess).mockReturnValue(mockChecker);
    vi.mocked(mockContextBuilder.buildFullContext).mockReturnValue({
      environment: { cwd: '/tmp', workspaces: [] },
    });

    const result = await runner.runChecker(mockToolCall, mockInProcessConfig);

    expect(result.decision).toBe(SafetyCheckDecision.DENY);
    expect(result.reason).toContain('Failed to run in-process checker');
    expect(result.reason).toContain('Checker failed');
  });

  it('should respect timeout for in-process checkers', async () => {
    vi.useFakeTimers();
    const mockChecker = {
      check: vi.fn().mockImplementation(async () => {
        await new Promise((resolve) => setTimeout(resolve, 6000)); // Longer than default 5s timeout
        return { decision: SafetyCheckDecision.ALLOW };
      }),
    };
    vi.mocked(mockRegistry.resolveInProcess).mockReturnValue(mockChecker);
    vi.mocked(mockContextBuilder.buildFullContext).mockReturnValue({
      environment: { cwd: '/tmp', workspaces: [] },
    });

    const runPromise = runner.runChecker(mockToolCall, mockInProcessConfig);
    vi.advanceTimersByTime(5001);

    const result = await runPromise;
    expect(result.decision).toBe(SafetyCheckDecision.DENY);
    expect(result.reason).toContain('timed out');

    vi.useRealTimers();
  });

  it('should use minimal context when requested', async () => {
    const configWithContext: InProcessCheckerConfig = {
      ...mockInProcessConfig,
      required_context: ['environment'],
    };
    const mockChecker = {
      check: vi.fn().mockResolvedValue({ decision: SafetyCheckDecision.ALLOW }),
    };
    vi.mocked(mockRegistry.resolveInProcess).mockReturnValue(mockChecker);
    vi.mocked(mockContextBuilder.buildMinimalContext).mockReturnValue({
      environment: { cwd: '/tmp', workspaces: [] },
    });

    await runner.runChecker(mockToolCall, configWithContext);

    expect(mockContextBuilder.buildMinimalContext).toHaveBeenCalledWith([
      'environment',
    ]);
    expect(mockContextBuilder.buildFullContext).not.toHaveBeenCalled();
  });

  it('should pass config to in-process checker via toolCall', async () => {
    const mockConfig = { included_args: ['foo'] };
    const configWithConfig: InProcessCheckerConfig = {
      ...mockInProcessConfig,
      config: mockConfig,
    };
    const mockResult: SafetyCheckResult = {
      decision: SafetyCheckDecision.ALLOW,
    };
    const mockChecker = {
      check: vi.fn().mockResolvedValue(mockResult),
    };
    vi.mocked(mockRegistry.resolveInProcess).mockReturnValue(mockChecker);
    vi.mocked(mockContextBuilder.buildFullContext).mockReturnValue({
      environment: { cwd: '/tmp', workspaces: [] },
    });

    await runner.runChecker(mockToolCall, configWithConfig);

    expect(mockChecker.check).toHaveBeenCalledWith(
      expect.objectContaining({
        toolCall: mockToolCall,
        config: mockConfig,
      }),
    );
  });

  describe('External Checkers', () => {
    it('should deny unsupported external checkers', async () => {
      const result = await runner.runChecker(mockToolCall, {
        type: 'external',
        name: 'python-checker',
      });

      expect(result.decision).toBe(SafetyCheckDecision.DENY);
      expect(result.reason).toContain(
        'External safety checkers are not supported',
      );
    });
  });
});
