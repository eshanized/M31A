/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  AuthType,
  IdeClient,
  IdeConnectionEvent,
  IdeConnectionType,
  loadApiKey,
  logIdeConnection,
  type Config,
  StartSessionEvent,
  logCliConfiguration,
  startupProfiler,
  debugLogger,
} from '@m31a/core';
import { type LoadedSettings } from '../config/settings.js';
import { performInitialAuth } from './auth.js';
import { validateTheme } from './theme.js';

export interface InitializationResult {
  authError: string | null;
  themeError: string | null;
  shouldOpenAuthDialog: boolean;
  m31aMdFileCount: number;
}

/**
 * Orchestrates the application's startup initialization.
 * This runs BEFORE the React UI is rendered.
 * @param config The application config.
 * @param settings The loaded application settings.
 * @returns The results of the initialization.
 */
export async function initializeApp(
  config: Config,
  settings: LoadedSettings,
): Promise<InitializationResult> {
  const authHandle = startupProfiler.start('authenticate');
  const configuredAuthType = settings.merged.security.auth.selectedType;
  const hasApiKey = Boolean(
    process.env['NVIDIA_API_KEY'] || (await loadApiKey()),
  );
  const authTypeToUse =
    configuredAuthType || (hasApiKey ? AuthType.USE_NVIDIA_NIM : undefined);
  const { authError } = await performInitialAuth(config, authTypeToUse);
  authHandle?.end();
  const themeError = validateTheme(settings);

  const shouldOpenAuthDialog = !!authError;

  logCliConfiguration(
    config,
    new StartSessionEvent(config, config.getToolRegistry()),
  );

  if (config.getIdeMode()) {
    IdeClient.getInstance()
      .then(async (ideClient) => {
        await ideClient.connect();
        logIdeConnection(
          config,
          new IdeConnectionEvent(IdeConnectionType.START),
        );
      })
      .catch((e) => {
        // We log locally if IDE connection setup fails in the background.
        debugLogger.error('Failed to initialize IDE client:', e);
      });
  }

  return {
    authError,
    themeError,
    shouldOpenAuthDialog,
    m31aMdFileCount: config.getGeminiMdFileCount(),
  };
}
