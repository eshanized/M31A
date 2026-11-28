/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  type Config,
  type FallbackModelHandler,
  type FallbackIntent,
  type ValidationHandler,
  type ValidationIntent,
  TerminalQuotaError,
  ModelNotFoundError,
  isSpendingCapMessage,
  type UserTierId,
  getDisplayString,
} from '@m31a/core';
import { useEffect } from 'react';
import { type UseHistoryManagerReturn } from './useHistoryManager.js';
import { MessageType } from '../types.js';
import type { LoadedSettings } from '../../config/settings.js';

interface UseQuotaAndFallbackArgs {
  config: Config;
  historyManager: UseHistoryManagerReturn;
  userTier: UserTierId | undefined;
  settings: LoadedSettings;
  setModelSwitchedFromQuotaError: (value: boolean) => void;
  onShowAuthSelection: () => void;
  errorVerbosity?: 'low' | 'full';
}

const isObject = (val: unknown): val is Record<string, unknown> =>
  typeof val === 'object' && val !== null;

export function useQuotaAndFallback({
  config,
  historyManager,
  userTier: _userTier,
  settings: _settings,
  setModelSwitchedFromQuotaError,
  onShowAuthSelection,
  errorVerbosity = 'full',
}: UseQuotaAndFallbackArgs) {
  useEffect(() => {
    const fallbackHandler: FallbackModelHandler = async (
      failedModel,
      _fallbackModel,
      error,
    ): Promise<FallbackIntent | null> => {
      const errorObj = isObject(error) ? error : null;
      const errorReasonValue = errorObj?.['reason'];
      const errorReason =
        typeof errorReasonValue === 'string' ? errorReasonValue : undefined;
      const errorMessageValue = errorObj?.['message'];
      const errorMessage =
        typeof errorMessageValue === 'string' ? errorMessageValue : undefined;

      const isCapacityExceeded =
        errorReason === 'MODEL_CAPACITY_EXHAUSTED' ||
        errorReason === 'MODEL_CAPACITY_EXCEEDED' ||
        (typeof errorMessage === 'string' &&
          /exhausted your capacity|capacity exceeded|MODEL_CAPACITY_EXHAUSTED/i.test(
            errorMessage,
          )) ||
        (typeof error === 'string' &&
          /exhausted your capacity|capacity exceeded|MODEL_CAPACITY_EXHAUSTED/i.test(
            error,
          ));

      let isTerminalQuotaError = false;
      let isModelNotFoundError = false;
      let message: string;

      if (error instanceof TerminalQuotaError) {
        isTerminalQuotaError = true;
        if (isSpendingCapMessage(error.message)) {
          message = error.message;
        } else {
          message = [
            `Rate or quota limit reached for ${getDisplayString(failedModel)}.`,
            error.retryDelayMs
              ? `Retry window resets in ~${Math.ceil(error.retryDelayMs / 1000)}s.`
              : null,
            'Use /model to choose another NVIDIA NIM model.',
          ]
            .filter(Boolean)
            .join('\n');
        }
      } else if (error instanceof ModelNotFoundError) {
        isModelNotFoundError = true;
        message = [
          `Model "${failedModel}" was not found or is unavailable on the configured NVIDIA NIM endpoint.`,
          'Use /model to select or enter a valid NIM model ID.',
        ].join('\n');
      } else {
        message = [
          `Temporary capacity issue on ${getDisplayString(failedModel)}.`,
          'Use /model to switch NVIDIA NIM models or retry.',
        ].join('\n');
      }

      if (
        errorVerbosity === 'low' &&
        (!isTerminalQuotaError || isCapacityExceeded) &&
        !isModelNotFoundError
      ) {
        return 'retry_once';
      }

      setModelSwitchedFromQuotaError(true);
      config.setQuotaErrorOccurred(true);
      historyManager.addItem(
        {
          type: MessageType.ERROR,
          text: message,
        },
        Date.now(),
      );

      return 'stop';
    };

    config.setFallbackModelHandler(fallbackHandler);
  }, [config, historyManager, setModelSwitchedFromQuotaError, errorVerbosity]);

  useEffect(() => {
    const validationHandler: ValidationHandler = async (
      _validationLink,
      validationDescription,
    ): Promise<ValidationIntent> => {
      if (validationDescription) {
        historyManager.addItem(
          {
            type: MessageType.ERROR,
            text: validationDescription,
          },
          Date.now(),
        );
      }
      onShowAuthSelection();
      return 'cancel';
    };

    config.setValidationHandler(validationHandler);
  }, [config, historyManager, onShowAuthSelection]);

  return {};
}
