/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi } from 'vitest';
import { DefaultStrategy } from './defaultStrategy.js';
import type { RoutingContext } from '../routingStrategy.js';
import type { BaseLlmClient } from '../../core/baseLlmClient.js';
import {
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_MODEL_AUTO,
  M31A_MODEL_ALIAS_AUTO,
} from '../../config/models.js';
import type { Config } from '../../config/config.js';

describe('DefaultStrategy', () => {
  it('should route to the default model when requested model is default auto', async () => {
    const strategy = new DefaultStrategy();
    const mockContext = {} as RoutingContext;
    const mockConfig = {
      getModel: vi.fn().mockReturnValue(DEFAULT_M31A_MODEL_AUTO),
    } as unknown as Config;
    const mockClient = {} as BaseLlmClient;

    const decision = await strategy.route(mockContext, mockConfig, mockClient);

    expect(decision).toEqual({
      model: DEFAULT_M31A_MODEL,
      metadata: {
        source: 'default',
        latencyMs: 0,
        reasoning: `Routing to default model: ${DEFAULT_M31A_MODEL}`,
      },
    });
  });

  it('should route to DEFAULT_M31A_MODEL when requested model is auto', async () => {
    const strategy = new DefaultStrategy();
    const mockContext = {} as RoutingContext;
    const mockConfig = {
      getModel: vi.fn().mockReturnValue(M31A_MODEL_ALIAS_AUTO),
    } as unknown as Config;
    const mockClient = {} as BaseLlmClient;

    const decision = await strategy.route(mockContext, mockConfig, mockClient);

    expect(decision).toEqual({
      model: DEFAULT_M31A_MODEL,
      metadata: {
        source: 'default',
        latencyMs: 0,
        reasoning: `Routing to default model: ${DEFAULT_M31A_MODEL}`,
      },
    });
  });

  // this should not happen, adding the test just in case it happens.
  it('should route to the same model if it is not an auto mode', async () => {
    const strategy = new DefaultStrategy();
    const mockContext = {} as RoutingContext;
    const mockConfig = {
      getModel: vi.fn().mockReturnValue('custom-nim-model'),
    } as unknown as Config;
    const mockClient = {} as BaseLlmClient;

    const decision = await strategy.route(mockContext, mockConfig, mockClient);

    expect(decision).toEqual({
      model: 'custom-nim-model',
      metadata: {
        source: 'default',
        latencyMs: 0,
        reasoning: 'Routing to default model: custom-nim-model',
      },
    });
  });

  it('should route to DEFAULT_M31A_MODEL for pro alias', async () => {
    const strategy = new DefaultStrategy();
    const mockContext = {} as RoutingContext;
    const mockConfig = {
      getModel: vi.fn().mockReturnValue('pro'),
    } as unknown as Config;
    const mockClient = {} as BaseLlmClient;

    const decision = await strategy.route(mockContext, mockConfig, mockClient);

    expect(decision.model).toBe(DEFAULT_M31A_MODEL);
  });
});
