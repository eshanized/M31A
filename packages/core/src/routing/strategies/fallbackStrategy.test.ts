/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { FallbackStrategy } from './fallbackStrategy.js';
import type { RoutingContext } from '../routingStrategy.js';
import type { BaseLlmClient } from '../../core/baseLlmClient.js';
import type { Config } from '../../config/config.js';
import type { ModelAvailabilityService } from '../../availability/modelAvailabilityService.js';
import {
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_MODEL_AUTO,
} from '../../config/models.js';
import { selectModelForAvailability } from '../../availability/policyHelpers.js';

vi.mock('../../availability/policyHelpers.js', () => ({
  selectModelForAvailability: vi.fn(),
}));

const createMockConfig = (overrides: Partial<Config> = {}): Config =>
  ({
    getModelAvailabilityService: vi.fn(),
    getModel: vi.fn().mockReturnValue(DEFAULT_M31A_MODEL),
    ...overrides,
  }) as unknown as Config;

describe('FallbackStrategy', () => {
  const strategy = new FallbackStrategy();
  const mockContext = {} as RoutingContext;
  const mockClient = {} as BaseLlmClient;
  let mockService: ModelAvailabilityService;
  let mockConfig: Config;

  beforeEach(() => {
    vi.resetAllMocks();

    mockService = {
      snapshot: vi.fn(),
    } as unknown as ModelAvailabilityService;

    mockConfig = createMockConfig({
      getModelAvailabilityService: vi.fn().mockReturnValue(mockService),
    });
  });

  it('should return null if the requested model is available', async () => {
    // Mock snapshot to return available
    vi.mocked(mockService.snapshot).mockReturnValue({ available: true });

    const decision = await strategy.route(mockContext, mockConfig, mockClient);
    expect(decision).toBeNull();
    // Should check availability of the resolved model (DEFAULT_M31A_MODEL)
    expect(mockService.snapshot).toHaveBeenCalledWith(DEFAULT_M31A_MODEL);
  });

  it('should return null if fallback selection is same as requested model', async () => {
    // Mock snapshot to return unavailable
    vi.mocked(mockService.snapshot).mockReturnValue({
      available: false,
      reason: 'quota',
    });
    // Mock selectModelForAvailability to return the SAME model (no fallback found)
    vi.mocked(selectModelForAvailability).mockReturnValue({
      selectedModel: DEFAULT_M31A_MODEL,
      skipped: [],
    });

    const decision = await strategy.route(mockContext, mockConfig, mockClient);
    expect(decision).toBeNull();
  });

  it('should return fallback decision if model is unavailable and fallback found', async () => {
    vi.mocked(mockConfig.getModel).mockReturnValue('custom-primary-model');
    // Mock snapshot to return unavailable
    vi.mocked(mockService.snapshot).mockReturnValue({
      available: false,
      reason: 'quota',
    });

    // Mock selectModelForAvailability to find a fallback
    vi.mocked(selectModelForAvailability).mockReturnValue({
      selectedModel: 'custom-fallback-model',
      skipped: [{ model: 'custom-primary-model', reason: 'quota' }],
    });

    const decision = await strategy.route(mockContext, mockConfig, mockClient);

    expect(decision).not.toBeNull();
    expect(decision?.model).toBe('custom-fallback-model');
    expect(decision?.metadata.source).toBe('fallback');
    expect(decision?.metadata.reasoning).toContain(
      'Model custom-primary-model is unavailable',
    );
  });

  it('should correctly handle "auto" alias by resolving it before checking availability', async () => {
    // Mock snapshot to return available for the RESOLVED model
    vi.mocked(mockService.snapshot).mockReturnValue({ available: true });
    vi.mocked(mockConfig.getModel).mockReturnValue(DEFAULT_M31A_MODEL_AUTO);

    const decision = await strategy.route(mockContext, mockConfig, mockClient);

    expect(decision).toBeNull();
    // Important: check that it queried snapshot with the RESOLVED model, not 'auto'
    expect(mockService.snapshot).toHaveBeenCalledWith(DEFAULT_M31A_MODEL);
  });

  it('should respect requestedModel from context', async () => {
    const requestedModel = 'requested-model';
    const configModel = 'config-model';
    vi.mocked(mockConfig.getModel).mockReturnValue(configModel);
    vi.mocked(mockService.snapshot).mockReturnValue({ available: true });

    const contextWithRequestedModel = {
      requestedModel,
    } as RoutingContext;

    const decision = await strategy.route(
      contextWithRequestedModel,
      mockConfig,
      mockClient,
    );

    expect(decision).toBeNull();
    // Should check availability of the requested model from context
    expect(mockService.snapshot).toHaveBeenCalledWith(requestedModel);
  });

  it('should resolve custom tool model variant when useCustomToolModel is enabled', async () => {
    const configWithCustomTools = createMockConfig({
      getModelAvailabilityService: vi.fn().mockReturnValue(mockService),
      getModel: vi.fn().mockReturnValue('pro'),
    });
    vi.mocked(mockService.snapshot).mockReturnValue({ available: true });

    const decision = await strategy.route(
      mockContext,
      configWithCustomTools,
      mockClient,
    );

    expect(decision).toBeNull();
    expect(mockService.snapshot).toHaveBeenCalledWith(DEFAULT_M31A_MODEL);
  });

  it('should return null if fallback selection equals resolved model', async () => {
    const configWithCustomTools = createMockConfig({
      getModelAvailabilityService: vi.fn().mockReturnValue(mockService),
      getModel: vi.fn().mockReturnValue('pro'),
    });
    vi.mocked(mockService.snapshot).mockReturnValue({
      available: false,
      reason: 'retry_once_per_turn',
    });
    vi.mocked(selectModelForAvailability).mockReturnValue({
      selectedModel: DEFAULT_M31A_MODEL,
      skipped: [],
    });

    const decision = await strategy.route(
      mockContext,
      configWithCustomTools,
      mockClient,
    );

    expect(decision).toBeNull();
  });
});
