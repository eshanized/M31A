/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  vi,
  describe,
  it,
  expect,
  beforeEach,
  afterEach,
  type Mock,
} from 'vitest';
import { renderHook, mockSettings } from '../../test-utils/render.js';
import {
  type Config,
  type FallbackModelHandler,
  type ValidationHandler,
  UserTierId,
  AuthType,
  TerminalQuotaError,
  makeFakeConfig,
  type StructuredApiError,
  ModelNotFoundError,
} from '@m31a/core';
import { useQuotaAndFallback } from './useQuotaAndFallback.js';
import type { UseHistoryManagerReturn } from './useHistoryManager.js';
import { MessageType } from '../types.js';

type SpyInstance = ReturnType<typeof vi.spyOn>;

describe('useQuotaAndFallback', () => {
  let mockConfig: Config;
  let mockHistoryManager: UseHistoryManagerReturn;
  let mockSetModelSwitchedFromQuotaError: Mock;
  let mockOnShowAuthSelection: Mock;
  let setFallbackHandlerSpy: SpyInstance;
  let setValidationHandlerSpy: SpyInstance;
  let mockApiError: StructuredApiError;

  beforeEach(() => {
    mockConfig = makeFakeConfig();
    mockApiError = {
      code: 429,
      message: 'rate limit exceeded',
      details: [],
    };

    vi.spyOn(mockConfig, 'getContentGeneratorConfig').mockReturnValue({
      authType: AuthType.USE_NVIDIA_NIM,
    });

    mockHistoryManager = {
      addItem: vi.fn(),
      history: [],
      updateItem: vi.fn(),
      clearItems: vi.fn(),
      loadHistory: vi.fn(),
    };
    mockSetModelSwitchedFromQuotaError = vi.fn();
    mockOnShowAuthSelection = vi.fn();

    setFallbackHandlerSpy = vi.spyOn(mockConfig, 'setFallbackModelHandler');
    setValidationHandlerSpy = vi.spyOn(mockConfig, 'setValidationHandler');
    vi.spyOn(mockConfig, 'setQuotaErrorOccurred');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('registers fallback and validation handlers on initialization', async () => {
    await renderHook(() =>
      useQuotaAndFallback({
        config: mockConfig,
        historyManager: mockHistoryManager,
        userTier: UserTierId.FREE,
        setModelSwitchedFromQuotaError: mockSetModelSwitchedFromQuotaError,
        onShowAuthSelection: mockOnShowAuthSelection,
        settings: mockSettings,
      }),
    );

    expect(setFallbackHandlerSpy).toHaveBeenCalledTimes(1);
    expect(setValidationHandlerSpy).toHaveBeenCalledTimes(1);
  });

  it('logs an error item and returns stop for TerminalQuotaError in full verbosity', async () => {
    await renderHook(() =>
      useQuotaAndFallback({
        config: mockConfig,
        historyManager: mockHistoryManager,
        userTier: UserTierId.FREE,
        setModelSwitchedFromQuotaError: mockSetModelSwitchedFromQuotaError,
        onShowAuthSelection: mockOnShowAuthSelection,
        settings: mockSettings,
      }),
    );

    const handler = setFallbackHandlerSpy.mock
      .calls[0][0] as FallbackModelHandler;
    const error = new TerminalQuotaError(
      'limit reached',
      mockApiError,
      1000 * 30,
    );

    const intent = await handler(
      'nvidia/nemotron-3-ultra-550b-a55b',
      'nvidia/nemotron-3-ultra-550b-a55b',
      error,
    );

    expect(intent).toBe('stop');
    expect(mockHistoryManager.addItem).toHaveBeenCalledWith(
      expect.objectContaining({
        type: MessageType.ERROR,
        text: expect.stringContaining(
          '/model to choose another NVIDIA NIM model',
        ),
      }),
      expect.any(Number),
    );
  });

  it('logs a ModelNotFoundError message directing user to /model', async () => {
    await renderHook(() =>
      useQuotaAndFallback({
        config: mockConfig,
        historyManager: mockHistoryManager,
        userTier: UserTierId.FREE,
        setModelSwitchedFromQuotaError: mockSetModelSwitchedFromQuotaError,
        onShowAuthSelection: mockOnShowAuthSelection,
        settings: mockSettings,
      }),
    );

    const handler = setFallbackHandlerSpy.mock
      .calls[0][0] as FallbackModelHandler;
    const intent = await handler(
      'unknown/model',
      'nvidia/nemotron-3-ultra-550b-a55b',
      new ModelNotFoundError('missing', 404),
    );

    expect(intent).toBe('stop');
    expect(mockHistoryManager.addItem).toHaveBeenCalledWith(
      expect.objectContaining({
        type: MessageType.ERROR,
        text: expect.stringContaining('not found or is unavailable'),
      }),
      expect.any(Number),
    );
  });

  it('auto-retries transient capacity failures in low verbosity mode', async () => {
    await renderHook(() =>
      useQuotaAndFallback({
        config: mockConfig,
        historyManager: mockHistoryManager,
        userTier: UserTierId.FREE,
        setModelSwitchedFromQuotaError: mockSetModelSwitchedFromQuotaError,
        onShowAuthSelection: mockOnShowAuthSelection,
        settings: mockSettings,
        errorVerbosity: 'low',
      }),
    );

    const handler = setFallbackHandlerSpy.mock
      .calls[0][0] as FallbackModelHandler;
    const intent = await handler(
      'nvidia/nemotron-3-ultra-550b-a55b',
      'nvidia/nemotron-3-ultra-550b-a55b',
      new Error('capacity exceeded'),
    );

    expect(intent).toBe('retry_once');
    expect(mockHistoryManager.addItem).not.toHaveBeenCalled();
  });

  it('routes validation errors to API key selection without opening ValidationDialog', async () => {
    await renderHook(() =>
      useQuotaAndFallback({
        config: mockConfig,
        historyManager: mockHistoryManager,
        userTier: UserTierId.FREE,
        setModelSwitchedFromQuotaError: mockSetModelSwitchedFromQuotaError,
        onShowAuthSelection: mockOnShowAuthSelection,
        settings: mockSettings,
      }),
    );

    const handler = setValidationHandlerSpy.mock
      .calls[0][0] as ValidationHandler;
    const intent = await handler(undefined, 'Invalid NVIDIA_API_KEY');

    expect(intent).toBe('cancel');
    expect(mockHistoryManager.addItem).toHaveBeenCalledWith(
      expect.objectContaining({
        type: MessageType.ERROR,
        text: 'Invalid NVIDIA_API_KEY',
      }),
      expect.any(Number),
    );
    expect(mockOnShowAuthSelection).toHaveBeenCalled();
  });
});
