/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { BaseLlmClient } from '../core/baseLlmClient.js';
import { FakeContentGenerator } from '../core/fakeContentGenerator.js';
import { Config } from '../config/config.js';
import {
  RetryableQuotaError,
  TerminalQuotaError,
} from '../utils/apiQuotaErrors.js';
import * as modelsModule from '../config/models.js';
import fs from 'node:fs';
import { AuthType } from '../core/contentGenerator.js';
import type { FallbackIntent } from '../fallback/types.js';
import { LlmRole } from '../telemetry/types.js';
import type { GenerateContentResponse } from '@m31a/genai';
import { normalizeModelId } from '../utils/modelUtils.js';
import * as policyCatalog from './policyCatalog.js';
import { createDefaultPolicy } from './policyCatalog.js';

const TEST_PRO_MODEL = 'test-pro-model';
const TEST_FLASH_MODEL = 'test-flash-model';
const TEST_CUSTOM_TOOLS_MODEL = 'test-custom-tools-model';
const TEST_AUTO_MODEL = 'auto';
const TEST_MANUAL_PRO_MODEL = 'test-manual-pro-model';

vi.mock('node:fs');

function buildTestChainForCatalog(
  isAutoSelection: boolean,
  mode: 'default' | 'manual' | 'customtools' = 'default',
) {
  const flashPolicy = {
    ...createDefaultPolicy(TEST_FLASH_MODEL, { isLastResort: true }),
    maxAttempts: 10,
  };

  if (mode === 'customtools') {
    const customPolicy = createDefaultPolicy(TEST_CUSTOM_TOOLS_MODEL);
    customPolicy.maxAttempts = 3;
    customPolicy.actions = { ...customPolicy.actions, terminal: 'silent' };
    customPolicy.stateTransitions = {
      ...customPolicy.stateTransitions,
      terminal: 'sticky_retry',
    };
    return [customPolicy, flashPolicy];
  }

  if (mode === 'manual' || !isAutoSelection) {
    const manualPolicy = createDefaultPolicy(TEST_MANUAL_PRO_MODEL);
    return [manualPolicy, flashPolicy];
  }

  const autoProPolicy = createDefaultPolicy(TEST_PRO_MODEL);
  autoProPolicy.maxAttempts = 3;
  autoProPolicy.actions = { ...autoProPolicy.actions, transient: 'silent' };
  autoProPolicy.stateTransitions = {
    ...autoProPolicy.stateTransitions,
    transient: 'sticky_retry',
  };
  return [autoProPolicy, flashPolicy];
}

describe('Auto Routing Fallback Integration', () => {
  let config: Config;
  let fakeGenerator: FakeContentGenerator;
  let client: BaseLlmClient;
  let activeTestMode: 'default' | 'manual' | 'customtools' = 'default';

  beforeEach(() => {
    activeTestMode = 'default';
    vi.useFakeTimers();
    vi.spyOn(modelsModule, 'isGemini3Model').mockImplementation(
      (model) =>
        model === TEST_PRO_MODEL ||
        model === TEST_MANUAL_PRO_MODEL ||
        model === TEST_CUSTOM_TOOLS_MODEL ||
        model === TEST_FLASH_MODEL,
    );
    vi.spyOn(policyCatalog, 'getModelPolicyChain').mockImplementation((opts) =>
      buildTestChainForCatalog(opts.isAutoSelection ?? false, activeTestMode),
    );
    const origResolveChain = Config.prototype[
      'modelConfigService' as never
    ] as unknown;
    void origResolveChain;

    // Mock fs to avoid real file system access
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.statSync).mockReturnValue({
      isDirectory: () => true,
    } as fs.Stats);

    // Provide a valid dummy sandbox policy for any readFileSync calls for TOML files
    vi.mocked(fs.readFileSync).mockImplementation((path) => {
      if (typeof path === 'string' && path.endsWith('.toml')) {
        return `
          [modes.plan]
          network = false
          readonly = true
          approvedTools = []

          [modes.default]
          network = false
          readonly = false
          approvedTools = []

          [modes.accepting_edits]
          network = false
          readonly = false
          approvedTools = []
        `;
      }
      return ''; // Fallback for other files
    });

    fakeGenerator = new FakeContentGenerator([]);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('should fallback to Flash after 3 tries and try 10 times for Flash in auto mode', async () => {
    // Instantiate real Config in auto mode
    config = new Config({
      sessionId: 'test-session',
      targetDir: '/test',
      debugMode: false,
      cwd: '/test',
      model: TEST_AUTO_MODEL, // Trigger auto mode
    });

    // Force interactive mode to enable fallback handler in BaseLlmClient
    vi.spyOn(config, 'isInteractive').mockReturnValue(true);

    client = new BaseLlmClient(fakeGenerator, config, AuthType.USE_NVIDIA_NIM);

    let attemptsPro = 0;
    let attemptsFlash = 0;

    const mockGoogleApiError = {
      code: 429,
      message: 'Quota exceeded',
      details: [],
    };

    // Spy on generateContent to simulate failures
    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_PRO_MODEL) {
          attemptsPro++;
          throw new RetryableQuotaError(
            'Quota exceeded for Pro',
            mockGoogleApiError,
            0,
          );
        } else if (modelId === TEST_FLASH_MODEL) {
          attemptsFlash++;
          throw new RetryableQuotaError(
            'Quota exceeded for Flash',
            mockGoogleApiError,
            0,
          );
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    // Set a fallback handler that approves the switch (simulating user or auto approval)
    config.setFallbackModelHandler(
      async (failed, _fallback, _error): Promise<FallbackIntent | null> => {
        if (normalizeModelId(failed) === TEST_FLASH_MODEL) {
          return 'stop'; // Stop retrying after Flash fails
        }
        return 'retry_always'; // Trigger fallback to Flash
      },
    );

    // Call generateContent
    const promise = client.generateContent({
      modelConfigKey: { model: TEST_PRO_MODEL, isChatModel: true },
      contents: [{ role: 'user', parts: [{ text: 'hi' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'test-prompt',
      role: LlmRole.UTILITY_TOOL,
    });

    await Promise.all([
      expect(promise).rejects.toThrow('Quota exceeded for Flash'),
      vi.runAllTimersAsync(),
    ]);

    // Verify attempts
    expect(attemptsPro).toBe(3);
    expect(attemptsFlash).toBe(10);
  });

  it('should try 10 times and prompt user in non-auto mode', async () => {
    activeTestMode = 'manual';
    // Instantiate real Config in non-auto mode
    const configNonAuto = new Config({
      sessionId: 'test-session',
      targetDir: '/test',
      debugMode: false,
      cwd: '/test',
      model: TEST_MANUAL_PRO_MODEL, // Non-auto mode
    });

    // Force interactive mode to enable fallback handler in BaseLlmClient
    vi.spyOn(configNonAuto, 'isInteractive').mockReturnValue(true);

    const clientNonAuto = new BaseLlmClient(
      fakeGenerator,
      configNonAuto,
      AuthType.USE_NVIDIA_NIM,
    );

    let attemptsPro = 0;

    const mockGoogleApiError = {
      code: 429,
      message: 'Quota exceeded',
      details: [],
    };

    // Spy on generateContent to simulate failures
    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_MANUAL_PRO_MODEL) {
          attemptsPro++;
          throw new RetryableQuotaError(
            'Quota exceeded for Pro',
            mockGoogleApiError,
            0,
          );
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    // Set a fallback handler that returns 'stop' (simulating user stopping or failing to handle)
    const handler = vi.fn(
      async (_failed, _fallback, _error): Promise<FallbackIntent | null> =>
        'stop',
    );
    configNonAuto.setFallbackModelHandler(handler);

    const promise = clientNonAuto.generateContent({
      modelConfigKey: { model: TEST_MANUAL_PRO_MODEL, isChatModel: true },
      contents: [{ role: 'user', parts: [{ text: 'hi' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'test-prompt',
      role: LlmRole.UTILITY_TOOL,
      maxAttempts: 10,
    });

    await Promise.all([
      expect(promise).rejects.toThrow('Quota exceeded for Pro'),
      vi.runAllTimersAsync(),
    ]);

    // Verify attempts (should default to 10)
    expect(attemptsPro).toBe(10);

    // Verify handler was called once after 10 attempts to prompt user
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(
      TEST_MANUAL_PRO_MODEL,
      TEST_FLASH_MODEL,
      expect.any(RetryableQuotaError),
    );
  });

  it('should fallback to Flash after 3 tries in experimental dynamic mode', async () => {
    // Instantiate real Config in auto mode
    const configDynamic = new Config({
      sessionId: 'test-session',
      targetDir: '/test',
      debugMode: false,
      cwd: '/test',
      model: TEST_AUTO_MODEL, // Trigger auto mode
    });

    // Force interactive mode to enable fallback handler in BaseLlmClient
    vi.spyOn(configDynamic, 'isInteractive').mockReturnValue(true);

    // Enable experimental dynamic model configuration
    vi.spyOn(
      configDynamic,
      'getExperimentalDynamicModelConfiguration',
    ).mockReturnValue(true);
    vi.spyOn(
      configDynamic.modelConfigService,
      'resolveChain',
    ).mockImplementation((chainName) =>
      buildTestChainForCatalog(
        String(chainName).startsWith('auto-'),
        'default',
      ),
    );

    const clientDynamic = new BaseLlmClient(
      fakeGenerator,
      configDynamic,
      AuthType.USE_NVIDIA_NIM,
    );

    let attemptsPro = 0;
    let attemptsFlash = 0;

    const mockGoogleApiError = {
      code: 429,
      message: 'Quota exceeded',
      details: [],
    };

    // Spy on generateContent to simulate failures
    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_PRO_MODEL) {
          attemptsPro++;
          throw new RetryableQuotaError(
            'Quota exceeded for Pro',
            mockGoogleApiError,
            0,
          );
        } else if (modelId === TEST_FLASH_MODEL) {
          attemptsFlash++;
          throw new RetryableQuotaError(
            'Quota exceeded for Flash',
            mockGoogleApiError,
            0,
          );
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    // Set a fallback handler that approves the switch
    configDynamic.setFallbackModelHandler(
      async (failed, _fallback, _error): Promise<FallbackIntent | null> => {
        if (normalizeModelId(failed) === TEST_FLASH_MODEL) {
          return 'stop';
        }
        return 'retry_always';
      },
    );

    const promise = clientDynamic.generateContent({
      modelConfigKey: { model: TEST_PRO_MODEL, isChatModel: true },
      contents: [{ role: 'user', parts: [{ text: 'hi' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'test-prompt',
      role: LlmRole.UTILITY_TOOL,
    });

    await Promise.all([
      expect(promise).rejects.toThrow('Quota exceeded for Flash'),
      vi.runAllTimersAsync(),
    ]);

    // Verify attempts
    expect(attemptsPro).toBe(3);
    expect(attemptsFlash).toBe(10);
  });

  it('should retry Pro on next turn after successful fallback to Flash', async () => {
    // Instantiate real Config in auto mode
    config = new Config({
      sessionId: 'test-session',
      targetDir: '/test',
      debugMode: false,
      cwd: '/test',
      model: TEST_AUTO_MODEL, // Trigger auto mode
    });

    // Force interactive mode to enable fallback handler in BaseLlmClient
    vi.spyOn(config, 'isInteractive').mockReturnValue(true);

    client = new BaseLlmClient(fakeGenerator, config, AuthType.USE_NVIDIA_NIM);

    let attemptsPro = 0;
    let attemptsFlash = 0;

    const mockGoogleApiError = {
      code: 429,
      message: 'Quota exceeded',
      details: [],
    };

    // Turn 1: Pro fails, Flash succeeds
    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_PRO_MODEL) {
          attemptsPro++;
          throw new RetryableQuotaError(
            'Quota exceeded for Pro',
            mockGoogleApiError,
            0,
          );
        } else if (modelId === TEST_FLASH_MODEL) {
          attemptsFlash++;
          return {
            candidates: [
              {
                content: { role: 'model', parts: [{ text: 'Flash success' }] },
              },
            ],
          } as unknown as GenerateContentResponse;
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    config.setFallbackModelHandler(
      async (_failed, _fallback, _error): Promise<FallbackIntent | null> =>
        'retry_always', // Approve switch to Flash
    );

    // Call generateContent for Turn 1
    const promise1 = client.generateContent({
      modelConfigKey: { model: TEST_PRO_MODEL, isChatModel: true },
      contents: [{ role: 'user', parts: [{ text: 'hi' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'test-prompt-1',
      role: LlmRole.UTILITY_TOOL,
    });

    await vi.runAllTimersAsync();
    const result1 = await promise1;

    expect(result1.candidates?.[0]?.content?.parts?.[0]?.text).toBe(
      'Flash success',
    );
    expect(attemptsPro).toBe(3);
    expect(attemptsFlash).toBe(1);

    // Simulate start of next turn
    config.getModelAvailabilityService().resetTurn();

    // Turn 2: Pro should be attempted again!
    // Let's make it succeed this time to verify it works!
    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_PRO_MODEL) {
          return {
            candidates: [
              { content: { role: 'model', parts: [{ text: 'Pro success' }] } },
            ],
          } as unknown as GenerateContentResponse;
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    const promise2 = client.generateContent({
      modelConfigKey: { model: TEST_PRO_MODEL, isChatModel: true }, // Request Pro again
      contents: [{ role: 'user', parts: [{ text: 'hello again' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'test-prompt-2',
      role: LlmRole.UTILITY_TOOL,
    });

    const result2 = await promise2;
    expect(result2.candidates?.[0]?.content?.parts?.[0]?.text).toBe(
      'Pro success',
    );
  });

  it('should rotate session ID on fallback and retry successfully with the Flash model', async () => {
    const originalSessionId = 'test-session-rotate-id';
    config = new Config({
      sessionId: originalSessionId,
      targetDir: '/test',
      debugMode: false,
      cwd: '/test',
      model: TEST_AUTO_MODEL,
    });

    vi.spyOn(config, 'isInteractive').mockReturnValue(true);

    client = new BaseLlmClient(fakeGenerator, config, AuthType.USE_NVIDIA_NIM);

    let attemptsPro = 0;
    let attemptsFlash = 0;

    const mockGoogleApiError = {
      code: 429,
      message:
        'Automatically switching from gemini-2.5-pro to gemini-2.5-flash for faster responses for the remainder of this session. Possible reasons for this are...',
      details: [],
    };

    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_PRO_MODEL) {
          attemptsPro++;
          throw new RetryableQuotaError(
            'Quota exceeded for Pro',
            mockGoogleApiError,
            0,
          );
        } else if (modelId === TEST_FLASH_MODEL) {
          attemptsFlash++;
          return {
            candidates: [
              {
                content: {
                  role: 'model',
                  parts: [{ text: 'Flash success after rotation' }],
                },
              },
            ],
          } as unknown as GenerateContentResponse;
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    config.setFallbackModelHandler(
      async (_failed, _fallback, _error): Promise<FallbackIntent | null> =>
        'retry_always', // Approve switch to Flash
    );

    const promise = client.generateContent({
      modelConfigKey: { model: TEST_PRO_MODEL, isChatModel: true },
      contents: [{ role: 'user', parts: [{ text: 'test query' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'test-prompt',
      role: LlmRole.UTILITY_TOOL,
    });

    await vi.runAllTimersAsync();
    const result = await promise;

    // Verify it resolved to Flash success instead of failing with Please submit a new query
    expect(result.candidates?.[0]?.content?.parts?.[0]?.text).toBe(
      'Flash success after rotation',
    );
    expect(attemptsPro).toBe(3);
    expect(attemptsFlash).toBe(1);

    // Verify session ID has been rotated
    expect(config.getSessionId()).not.toBe(originalSessionId);
    expect(config.getSessionId()).toBeDefined();
  });

  it('should fallback once to Flash on capacity error for manual customtools model and restore Pro on next turn', async () => {
    activeTestMode = 'customtools';
    config = new Config({
      sessionId: 'test-session-manual-pro-31',
      targetDir: '/test',
      debugMode: false,
      cwd: '/test',
      model: TEST_CUSTOM_TOOLS_MODEL,
    });

    vi.spyOn(config, 'isInteractive').mockReturnValue(true);

    client = new BaseLlmClient(fakeGenerator, config, AuthType.USE_NVIDIA_NIM);

    let attemptsProTurn1 = 0;
    let attemptsFlashTurn1 = 0;

    const capacityApiError = {
      code: 429,
      message: 'No capacity available for customtools model',
      details: [],
    };

    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_CUSTOM_TOOLS_MODEL) {
          attemptsProTurn1++;
          throw new TerminalQuotaError(
            'No capacity available',
            capacityApiError,
            0,
            'MODEL_CAPACITY_EXHAUSTED',
          );
        }
        if (modelId === TEST_FLASH_MODEL) {
          attemptsFlashTurn1++;
          return {
            candidates: [
              {
                content: {
                  role: 'model',
                  parts: [{ text: 'Flash temporary fallback success' }],
                },
              },
            ],
          } as unknown as GenerateContentResponse;
        }
        throw new Error(`Unexpected model: ${params.model}`);
      },
    );

    // Low error verbosity returns 'retry_once' for capacity exhaustion
    config.setFallbackModelHandler(
      async (): Promise<FallbackIntent | null> => 'retry_once',
    );

    const promise1 = client.generateContent({
      modelConfigKey: {
        model: TEST_CUSTOM_TOOLS_MODEL,
        isChatModel: true,
      },
      contents: [{ role: 'user', parts: [{ text: 'turn 1' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'prompt-1',
      role: LlmRole.MAIN,
    });

    await vi.runAllTimersAsync();
    const result1 = await promise1;

    expect(result1.candidates?.[0]?.content?.parts?.[0]?.text).toBe(
      'Flash temporary fallback success',
    );
    expect(attemptsProTurn1).toBe(3); // 3 silent capacity attempts before handler
    expect(attemptsFlashTurn1).toBe(1);
    // Configured model remains TEST_CUSTOM_TOOLS_MODEL
    expect(config.getModel()).toBe(TEST_CUSTOM_TOOLS_MODEL);

    // Start next turn (or after cancel + turn reset): Pro must be restored!
    config.resetTurn();
    expect(config.getActiveModel()).toBe(TEST_CUSTOM_TOOLS_MODEL);
    expect(
      config.getModelAvailabilityService().snapshot(TEST_CUSTOM_TOOLS_MODEL)
        .available,
    ).toBe(true);

    vi.spyOn(fakeGenerator, 'generateContent').mockImplementation(
      async (params) => {
        const modelId = normalizeModelId(params.model ?? '');
        if (modelId === TEST_CUSTOM_TOOLS_MODEL) {
          return {
            candidates: [
              {
                content: {
                  role: 'model',
                  parts: [{ text: 'Pro customtools restored' }],
                },
              },
            ],
          } as unknown as GenerateContentResponse;
        }
        throw new Error(`Unexpected model on turn 2: ${params.model}`);
      },
    );

    const result2 = await client.generateContent({
      modelConfigKey: {
        model: TEST_CUSTOM_TOOLS_MODEL,
        isChatModel: true,
      },
      contents: [{ role: 'user', parts: [{ text: 'turn 2' }] }],
      abortSignal: new AbortController().signal,
      promptId: 'prompt-2',
      role: LlmRole.MAIN,
    });

    expect(result2.candidates?.[0]?.content?.parts?.[0]?.text).toBe(
      'Pro customtools restored',
    );
  });
});
