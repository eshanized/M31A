/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { applyModelSelection } from './policyHelpers.js';
import type { Config } from '../config/config.js';
import {
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_MODEL_AUTO,
} from '../config/models.js';
import { ModelAvailabilityService } from './modelAvailabilityService.js';
import { ModelConfigService } from '../services/modelConfigService.js';
import { DEFAULT_MODEL_CONFIGS } from '../config/defaultModelConfigs.js';

describe('Fallback Integration', () => {
  let config: Config;
  let availabilityService: ModelAvailabilityService;
  let modelConfigService: ModelConfigService;

  beforeEach(() => {
    // Mocking Config because it has many dependencies
    config = {
      getModel: () => DEFAULT_M31A_MODEL_AUTO,
      getActiveModel: () => DEFAULT_M31A_MODEL_AUTO,
      setActiveModel: vi.fn(),
      getUserTier: () => undefined,
      getModelAvailabilityService: () => availabilityService,
      modelConfigService: undefined as unknown as ModelConfigService,
    } as unknown as Config;

    availabilityService = new ModelAvailabilityService();
    modelConfigService = new ModelConfigService(DEFAULT_MODEL_CONFIGS);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (config as any).modelConfigService = modelConfigService;
  });

  it('should fall back to last-resort NIM model when primary model is terminal', () => {
    availabilityService.markTerminal(DEFAULT_M31A_MODEL, 'quota');

    expect(
      availabilityService.selectFirstAvailable([DEFAULT_M31A_MODEL])
        .selectedModel,
    ).toBeNull();

    const result = applyModelSelection(config, {
      model: DEFAULT_M31A_MODEL,
      isChatModel: true,
    });

    expect(result.model).toBe(DEFAULT_M31A_MODEL);
  });

  it('should select default NIM model on initial attempt and track sticky failures', () => {
    const requestedModel = DEFAULT_M31A_MODEL;

    const result1 = applyModelSelection(config, {
      model: requestedModel,
      isChatModel: true,
    });
    expect(result1.model).toBe(DEFAULT_M31A_MODEL);
    expect(result1.maxAttempts).toBe(3);

    availabilityService.markRetryOncePerTurn(DEFAULT_M31A_MODEL, 3);
    availabilityService.consumeStickyAttempt(DEFAULT_M31A_MODEL);

    expect(
      availabilityService.selectFirstAvailable([DEFAULT_M31A_MODEL])
        .selectedModel,
    ).toBeNull();

    const result2 = applyModelSelection(config, {
      model: requestedModel,
      isChatModel: true,
    });
    expect(result2.model).toBe(DEFAULT_M31A_MODEL);

    availabilityService.resetTurn();

    const result3 = applyModelSelection(config, {
      model: requestedModel,
      isChatModel: true,
    });
    expect(result3.model).toBe(DEFAULT_M31A_MODEL);
    expect(result3.maxAttempts).toBe(3);
  });
});
