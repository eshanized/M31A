/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  resolvePolicyChain,
  buildFallbackPolicyContext,
  applyModelSelection,
  applyAvailabilityTransition,
} from './policyHelpers.js';
import { createDefaultPolicy, SILENT_ACTIONS } from './policyCatalog.js';
import type { RetryAvailabilityContext } from './modelPolicy.js';
import type { Config } from '../config/config.js';
import {
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_FLASH_LITE_MODEL,
  DEFAULT_M31A_MODEL_AUTO,
  M31A_MODEL_ALIAS_FLASH_LITE,
} from '../config/models.js';
import { AuthType } from '../core/contentGenerator.js';
import { ModelConfigService } from '../services/modelConfigService.js';
import { DEFAULT_MODEL_CONFIGS } from '../config/defaultModelConfigs.js';
import { ApprovalMode } from '../policy/types.js';

const createMockConfig = (overrides: Partial<Config> = {}): Config => {
  const config = {
    getUserTier: () => undefined,
    getModel: () => 'gemini-2.5-pro',
    getContentGeneratorConfig: () => ({ authType: undefined }),
    getMaxAttemptsPerTurn: () => 3,
    getExperimentalDynamicModelConfiguration: () => false,
    getReleaseChannel: () => 'preview',
    modelConfigService: new ModelConfigService(DEFAULT_MODEL_CONFIGS),
    ...overrides,
  } as unknown as Config;
  return config;
};

describe('policyHelpers', () => {
  beforeEach(() => {});

  afterEach(() => {});

  describe('resolvePolicyChain', () => {
    it('returns a single-model chain for a custom model', () => {
      const config = createMockConfig({
        getModel: () => 'custom-model',
      });
      const chain = resolvePolicyChain(config);
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe('custom-model');
    });

    it('leaves catalog order untouched when active model already present', () => {
      const config = createMockConfig({
        getModel: () => 'gemini-2.5-pro',
      });
      const chain = resolvePolicyChain(config);
      expect(chain[0]?.model).toBe('gemini-2.5-pro');
    });

    it('returns the default chain when active model is "auto"', () => {
      const config = createMockConfig({
        getModel: () => DEFAULT_M31A_MODEL_AUTO,
      });
      const chain = resolvePolicyChain(config);

      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
      expect(chain[0]?.isLastResort).toBe(true);
    });

    it('uses auto chain when preferred model is auto', () => {
      const config = createMockConfig({
        getModel: () => 'custom-model',
      });
      const chain = resolvePolicyChain(config, 'auto');
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
      expect(chain[0]?.maxAttempts).toBe(3);
    });

    it('uses auto chain when configured model is auto even if preferred is concrete', () => {
      const config = createMockConfig({
        getModel: () => 'auto',
      });
      const chain = resolvePolicyChain(config, DEFAULT_M31A_MODEL_AUTO);
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
    });

    it('starts chain from preferredModel when model is "auto"', () => {
      const config = createMockConfig({
        getModel: () => 'auto',
      });
      const chain = resolvePolicyChain(config, 'flash');
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
    });

    it('returns flash-lite chain when preferred model is flash-lite', () => {
      const config = createMockConfig({
        getModel: () => DEFAULT_M31A_MODEL_AUTO,
      });
      const chain = resolvePolicyChain(config, M31A_MODEL_ALIAS_FLASH_LITE);
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_FLASH_LITE_MODEL);
      expect(chain[0]?.actions).toEqual(SILENT_ACTIONS);
      expect(chain[0]?.isLastResort).toBe(true);
    });

    it('returns flash-lite chain when configured model is flash-lite', () => {
      const config = createMockConfig({
        getModel: () => M31A_MODEL_ALIAS_FLASH_LITE,
      });
      const chain = resolvePolicyChain(config);
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_FLASH_LITE_MODEL);
      expect(chain[0]?.actions).toEqual(SILENT_ACTIONS);
      expect(chain[0]?.isLastResort).toBe(true);
    });

    it('wraps around the chain when wrapsAround is true', () => {
      const config = createMockConfig({
        getModel: () => DEFAULT_M31A_MODEL_AUTO,
      });
      const chain = resolvePolicyChain(config, 'pro', true);
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
      expect(chain[0]?.isLastResort).toBe(true);
    });

    it('resolves concrete M31A alias models to the single default chain', () => {
      const config = createMockConfig({
        getModel: () => 'pro',
      });
      const chain = resolvePolicyChain(config);
      expect(chain).toHaveLength(1);
      expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
      expect(chain[0]?.isLastResort).toBe(true);
    });

    it('applies SILENT_ACTIONS when ApprovalMode is PLAN', () => {
      const config = createMockConfig({
        getApprovalMode: () => ApprovalMode.PLAN,
        getModel: () => DEFAULT_M31A_MODEL_AUTO,
      });
      const chain = resolvePolicyChain(config);

      expect(chain).toHaveLength(1);
      expect(chain[0]?.actions).toEqual(SILENT_ACTIONS);
    });
  });

  describe('resolvePolicyChain behavior is identical between dynamic and legacy implementations', () => {
    const testCases = [
      { name: 'Default M31A Model', model: DEFAULT_M31A_MODEL_AUTO },
      { name: 'Unified Auto', model: 'auto' },
      { name: 'Pro Alias', model: 'pro' },
      { name: 'Flash Alias', model: 'flash' },
      { name: 'Flash Lite Alias', model: 'flash-lite' },
      { name: 'Flash Lite Concrete', model: DEFAULT_M31A_FLASH_LITE_MODEL },
      { name: 'Custom Model', model: 'my-custom-model' },
      {
        name: 'Wrap Around',
        model: 'auto',
        wrapsAround: true,
      },
      {
        name: 'With Custom Auth Type',
        model: 'auto',
        authType: AuthType.USE_NVIDIA_NIM,
      },
    ];

    testCases.forEach(({ name, model, authType, wrapsAround }) => {
      it(`achieves parity for: ${name}`, () => {
        const createBaseConfig = (dynamic: boolean) =>
          createMockConfig({
            getExperimentalDynamicModelConfiguration: () => dynamic,
            getModel: () => model,
            getContentGeneratorConfig: () => ({ authType }),
            getReleaseChannel: () => 'preview',
            modelConfigService: new ModelConfigService(DEFAULT_MODEL_CONFIGS),
          });

        const legacyChain = resolvePolicyChain(
          createBaseConfig(false),
          model,
          wrapsAround,
        );
        const dynamicChain = resolvePolicyChain(
          createBaseConfig(true),
          model,
          wrapsAround,
        );

        expect(dynamicChain).toEqual(legacyChain);
      });
    });
  });

  describe('buildFallbackPolicyContext', () => {
    it('returns remaining candidates after the failed model', () => {
      const chain = [
        createDefaultPolicy('a'),
        createDefaultPolicy('b'),
        createDefaultPolicy('c'),
      ];
      const context = buildFallbackPolicyContext(chain, 'b');
      expect(context.failedPolicy?.model).toBe('b');
      expect(context.candidates.map((p) => p.model)).toEqual(['c']);
    });

    it('wraps around when building fallback context if wrapsAround is true', () => {
      const chain = [
        createDefaultPolicy('a'),
        createDefaultPolicy('b'),
        createDefaultPolicy('c'),
      ];
      const context = buildFallbackPolicyContext(chain, 'b', true);
      expect(context.failedPolicy?.model).toBe('b');
      expect(context.candidates.map((p) => p.model)).toEqual(['c', 'a']);
    });

    it('returns full chain when model is not in policy list', () => {
      const chain = [createDefaultPolicy('a'), createDefaultPolicy('b')];
      const context = buildFallbackPolicyContext(chain, 'x');
      expect(context.failedPolicy).toBeUndefined();
      expect(context.candidates).toEqual(chain);
    });
  });

  describe('applyModelSelection', () => {
    const mockModelConfigService = {
      getResolvedConfig: vi.fn(),
    };

    const mockAvailabilityService = {
      selectFirstAvailable: vi.fn(),
      consumeStickyAttempt: vi.fn(),
    };

    const createExtendedMockConfig = (
      overrides: Partial<Config> = {},
    ): Config => {
      const defaults = {
        getModelAvailabilityService: () => mockAvailabilityService,
        setActiveModel: vi.fn(),
        modelConfigService: mockModelConfigService,
      };
      return createMockConfig({ ...defaults, ...overrides } as Partial<Config>);
    };

    beforeEach(() => {
      vi.clearAllMocks();
    });

    it('returns requested model if it is available', () => {
      const config = createExtendedMockConfig();
      mockModelConfigService.getResolvedConfig.mockReturnValue({
        model: 'gemini-pro',
        generateContentConfig: {},
      });
      mockAvailabilityService.selectFirstAvailable.mockReturnValue({
        selectedModel: 'gemini-pro',
      });

      const result = applyModelSelection(config, {
        model: 'gemini-pro',
        isChatModel: true,
      });
      expect(result.model).toBe('gemini-pro');
      expect(result.maxAttempts).toBeUndefined();
      expect(config.setActiveModel).toHaveBeenCalledWith('gemini-pro');
    });

    it('switches to backup model and updates config if requested is unavailable', () => {
      const config = createExtendedMockConfig();
      mockModelConfigService.getResolvedConfig
        .mockReturnValueOnce({
          model: 'gemini-pro',
          generateContentConfig: { temperature: 0.9, topP: 1 },
        })
        .mockReturnValueOnce({
          model: 'gemini-flash',
          generateContentConfig: { temperature: 0.1, topP: 1 },
        });
      mockAvailabilityService.selectFirstAvailable.mockReturnValue({
        selectedModel: 'gemini-flash',
      });

      const result = applyModelSelection(config, {
        model: 'gemini-pro',
        isChatModel: true,
      });

      expect(result.model).toBe('gemini-flash');
      expect(result.config).toEqual({
        temperature: 0.1,
        topP: 1,
      });

      expect(mockModelConfigService.getResolvedConfig).toHaveBeenCalledWith({
        model: 'gemini-pro',
        isChatModel: true,
      });
      expect(mockModelConfigService.getResolvedConfig).toHaveBeenCalledWith({
        model: 'gemini-flash',
        isChatModel: true,
      });
      expect(config.setActiveModel).toHaveBeenCalledWith('gemini-flash');
    });

    it('does not call setActiveModel if isChatModel is false', () => {
      const config = createExtendedMockConfig();
      mockModelConfigService.getResolvedConfig.mockReturnValue({
        model: 'gemini-pro',
        generateContentConfig: {},
      });
      mockAvailabilityService.selectFirstAvailable.mockReturnValue({
        selectedModel: 'gemini-pro',
      });

      applyModelSelection(config, {
        model: 'gemini-pro',
        isChatModel: false,
      });
      expect(config.setActiveModel).not.toHaveBeenCalled();
    });

    it('consumes sticky attempt if indicated and isChatModel is true', () => {
      const config = createExtendedMockConfig();
      mockModelConfigService.getResolvedConfig.mockReturnValue({
        model: 'gemini-pro',
        generateContentConfig: {},
      });
      mockAvailabilityService.selectFirstAvailable.mockReturnValue({
        selectedModel: 'gemini-pro',
        attempts: 1,
      });

      const result = applyModelSelection(config, {
        model: 'gemini-pro',
        isChatModel: true,
      });
      expect(mockAvailabilityService.consumeStickyAttempt).toHaveBeenCalledWith(
        'gemini-pro',
      );
      expect(config.setActiveModel).toHaveBeenCalledWith('gemini-pro');
      expect(result.maxAttempts).toBe(1);
    });

    it('consumes sticky attempt if indicated but does not call setActiveModel if isChatModel is false', () => {
      const config = createExtendedMockConfig();
      mockModelConfigService.getResolvedConfig.mockReturnValue({
        model: 'gemini-pro',
        generateContentConfig: {},
      });
      mockAvailabilityService.selectFirstAvailable.mockReturnValue({
        selectedModel: 'gemini-pro',
        attempts: 1,
      });

      const result = applyModelSelection(config, {
        model: 'gemini-pro',
        isChatModel: false,
      });
      expect(mockAvailabilityService.consumeStickyAttempt).toHaveBeenCalledWith(
        'gemini-pro',
      );
      expect(config.setActiveModel).not.toHaveBeenCalled();
      expect(result.maxAttempts).toBe(1);
    });

    it('does not consume sticky attempt if consumeAttempt is false', () => {
      const config = createExtendedMockConfig();
      mockModelConfigService.getResolvedConfig.mockReturnValue({
        model: 'gemini-pro',
        generateContentConfig: {},
      });
      mockAvailabilityService.selectFirstAvailable.mockReturnValue({
        selectedModel: 'gemini-pro',
        attempts: 1,
      });

      const result = applyModelSelection(
        config,
        { model: 'gemini-pro', isChatModel: true },
        {
          consumeAttempt: false,
        },
      );
      expect(
        mockAvailabilityService.consumeStickyAttempt,
      ).not.toHaveBeenCalled();
      expect(config.setActiveModel).toHaveBeenCalledWith('gemini-pro');
      expect(result.maxAttempts).toBe(1);
    });
  });

  describe('applyAvailabilityTransition', () => {
    it('marks terminal on terminal transition', () => {
      const mockService = { markTerminal: vi.fn() };
      const context = {
        service: mockService,
        policy: {
          model: 'test-model',
          stateTransitions: { transient: 'terminal' },
        },
      };
      const getContext = () => context as unknown as RetryAvailabilityContext;

      applyAvailabilityTransition(getContext, 'transient');

      expect(mockService.markTerminal).toHaveBeenCalledWith(
        'test-model',
        'capacity',
      );
    });

    it('marks sticky and consumes on sticky_retry transition', () => {
      const mockService = {
        markRetryOncePerTurn: vi.fn(),
        consumeStickyAttempt: vi.fn(),
      };
      const context = {
        service: mockService,
        policy: {
          model: 'test-model',
          stateTransitions: { transient: 'sticky_retry' },
          maxAttempts: 3,
        },
      };
      const getContext = () => context as unknown as RetryAvailabilityContext;

      applyAvailabilityTransition(getContext, 'transient');

      expect(mockService.markRetryOncePerTurn).toHaveBeenCalledWith(
        'test-model',
        3,
      );
      expect(mockService.consumeStickyAttempt).toHaveBeenCalledWith(
        'test-model',
      );
    });
  });
});
