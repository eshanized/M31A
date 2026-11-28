/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ApprovalModeStrategy } from './approvalModeStrategy.js';
import type { RoutingContext } from '../routingStrategy.js';
import type { Config } from '../../config/config.js';
import {
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_FLASH_MODEL,
  DEFAULT_M31A_MODEL_AUTO,
  M31A_MODEL_ALIAS_AUTO,
} from '../../config/models.js';
import { AuthType } from '../../core/contentGenerator.js';
import { ApprovalMode } from '../../policy/types.js';
import type { BaseLlmClient } from '../../core/baseLlmClient.js';

describe('ApprovalModeStrategy', () => {
  let strategy: ApprovalModeStrategy;
  let mockContext: RoutingContext;
  let mockConfig: Config;
  let mockBaseLlmClient: BaseLlmClient;

  beforeEach(() => {
    vi.clearAllMocks();

    strategy = new ApprovalModeStrategy();
    mockContext = {
      history: [],
      request: [{ text: 'test' }],
      signal: new AbortController().signal,
    };

    mockConfig = {
      getModel: vi.fn().mockReturnValue(DEFAULT_M31A_MODEL_AUTO),
      getApprovalMode: vi.fn().mockReturnValue(ApprovalMode.DEFAULT),
      getApprovedPlanPath: vi.fn().mockReturnValue(undefined),
      getPlanModeRoutingEnabled: vi.fn().mockResolvedValue(true),
      getContentGeneratorConfig: vi.fn().mockReturnValue({
        authType: AuthType.USE_NVIDIA_NIM,
      }),
    } as unknown as Config;

    mockBaseLlmClient = {} as BaseLlmClient;
  });

  it('should return null if the model is not an auto model', async () => {
    vi.mocked(mockConfig.getModel).mockReturnValue(DEFAULT_M31A_MODEL);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision).toBeNull();
  });

  it('should return null if plan mode routing is disabled', async () => {
    vi.mocked(mockConfig.getPlanModeRoutingEnabled).mockResolvedValue(false);
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.PLAN);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision).toBeNull();
  });

  it('should route to PRO model if ApprovalMode is PLAN (Gemini 2.5)', async () => {
    vi.mocked(mockConfig.getModel).mockReturnValue(DEFAULT_M31A_MODEL_AUTO);
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.PLAN);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision).toEqual({
      model: DEFAULT_M31A_MODEL,
      metadata: {
        source: 'approval-mode',
        latencyMs: expect.any(Number),
        reasoning: 'Routing to Pro model because ApprovalMode is PLAN.',
      },
    });
  });

  it('should route to PRO model if ApprovalMode is PLAN (auto alias)', async () => {
    vi.mocked(mockConfig.getModel).mockReturnValue(M31A_MODEL_ALIAS_AUTO);
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.PLAN);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision).toEqual({
      model: DEFAULT_M31A_MODEL,
      metadata: {
        source: 'approval-mode',
        latencyMs: expect.any(Number),
        reasoning: 'Routing to Pro model because ApprovalMode is PLAN.',
      },
    });
  });

  it('should route to FLASH model if an approved plan exists (auto)', async () => {
    vi.mocked(mockConfig.getModel).mockReturnValue(DEFAULT_M31A_MODEL_AUTO);
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.DEFAULT);
    vi.mocked(mockConfig.getApprovedPlanPath).mockReturnValue(
      '/path/to/plan.md',
    );

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision).toEqual({
      model: DEFAULT_M31A_FLASH_MODEL,
      metadata: {
        source: 'approval-mode',
        latencyMs: expect.any(Number),
        reasoning:
          'Routing to Flash model because an approved plan exists at /path/to/plan.md.',
      },
    });
  });

  it('should return null if not in PLAN mode and no approved plan exists', async () => {
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.DEFAULT);
    vi.mocked(mockConfig.getApprovedPlanPath).mockReturnValue(undefined);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision).toBeNull();
  });

  it('should prioritize requestedModel over config model if it is an auto model', async () => {
    mockContext.requestedModel = M31A_MODEL_ALIAS_AUTO;
    vi.mocked(mockConfig.getModel).mockReturnValue(DEFAULT_M31A_MODEL);
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.PLAN);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision?.model).toBe(DEFAULT_M31A_MODEL);
  });

  it('should route to M31A models when using "auto" alias', async () => {
    vi.mocked(mockConfig.getModel).mockReturnValue(M31A_MODEL_ALIAS_AUTO);
    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.PLAN);

    const decision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(decision?.model).toBe(DEFAULT_M31A_MODEL);

    vi.mocked(mockConfig.getApprovalMode).mockReturnValue(ApprovalMode.DEFAULT);
    vi.mocked(mockConfig.getApprovedPlanPath).mockReturnValue(
      '/path/to/plan.md',
    );

    const implementationDecision = await strategy.route(
      mockContext,
      mockConfig,
      mockBaseLlmClient,
    );

    expect(implementationDecision?.model).toBe(DEFAULT_M31A_FLASH_MODEL);
  });
});
