/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  describe,
  it,
  expect,
  vi,
  beforeEach,
  afterEach,
  type MockInstance,
} from 'vitest';
import { validateNonInteractiveAuth } from './validateNonInterActiveAuth.js';
import {
  AuthType,
  OutputFormat,
  makeFakeConfig,
  debugLogger,
  ExitCodes,
  coreEvents,
} from '@m31a/core';
import type { Config } from '@m31a/core';
import * as auth from './config/auth.js';
import { type LoadedSettings } from './config/settings.js';

function createLocalMockConfig(overrides: Partial<Config> = {}): Config {
  const config = makeFakeConfig();
  Object.assign(config, overrides);
  return config;
}

describe('validateNonInterActiveAuth', () => {
  let originalEnvNvidiaApiKey: string | undefined;
  let debugLoggerErrorSpy: ReturnType<typeof vi.spyOn>;
  let coreEventsEmitFeedbackSpy: MockInstance;
  let processExitSpy: MockInstance;
  let mockSettings: LoadedSettings;

  beforeEach(() => {
    originalEnvNvidiaApiKey = process.env['NVIDIA_API_KEY'];
    delete process.env['NVIDIA_API_KEY'];
    debugLoggerErrorSpy = vi
      .spyOn(debugLogger, 'error')
      .mockImplementation(() => {});
    coreEventsEmitFeedbackSpy = vi
      .spyOn(coreEvents, 'emitFeedback')
      .mockImplementation(() => {});
    processExitSpy = vi
      .spyOn(process, 'exit')
      .mockImplementation((code?: string | number | null | undefined) => {
        throw new Error(`process.exit(${code}) called`);
      });
    vi.spyOn(auth, 'validateAuthMethod').mockResolvedValue(null);
    mockSettings = {
      system: { path: '', settings: {} },
      systemDefaults: { path: '', settings: {} },
      user: { path: '', settings: {} },
      workspace: { path: '', settings: {} },
      errors: [],
      setValue: vi.fn(),
      merged: {
        security: {
          auth: {
            enforcedType: undefined,
          },
        },
      },
      isTrusted: true,
      migratedInMemoryScopes: new Set(),
      forScope: vi.fn(),
      computeMergedSettings: vi.fn(),
    } as unknown as LoadedSettings;
  });

  afterEach(() => {
    if (originalEnvNvidiaApiKey !== undefined) {
      process.env['NVIDIA_API_KEY'] = originalEnvNvidiaApiKey;
    } else {
      delete process.env['NVIDIA_API_KEY'];
    }
    vi.restoreAllMocks();
  });

  it('exits if no NVIDIA_API_KEY is available', async () => {
    vi.spyOn(auth, 'validateAuthMethod').mockResolvedValueOnce(
      'When using NVIDIA NIM, you must specify the NVIDIA_API_KEY environment variable.',
    );
    const nonInteractiveConfig = createLocalMockConfig({
      getOutputFormat: vi.fn().mockReturnValue(OutputFormat.TEXT),
      getContentGeneratorConfig: vi
        .fn()
        .mockReturnValue({ authType: undefined }),
    });
    try {
      await validateNonInteractiveAuth(
        undefined,
        undefined,
        nonInteractiveConfig,
        mockSettings,
      );
      expect.fail('Should have exited');
    } catch (e) {
      expect((e as Error).message).toContain(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
    }
    expect(debugLoggerErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('NVIDIA_API_KEY'),
    );
    expect(processExitSpy).toHaveBeenCalledWith(
      ExitCodes.FATAL_AUTHENTICATION_ERROR,
    );
  });

  it('uses USE_NVIDIA_NIM if NVIDIA_API_KEY is set', async () => {
    process.env['NVIDIA_API_KEY'] = 'nvapi-fake-key';
    const nonInteractiveConfig = createLocalMockConfig({});
    await validateNonInteractiveAuth(
      undefined,
      undefined,
      nonInteractiveConfig,
      mockSettings,
    );
    expect(processExitSpy).not.toHaveBeenCalled();
    expect(debugLoggerErrorSpy).not.toHaveBeenCalled();
  });

  it('uses configuredAuthType over environment variables', async () => {
    process.env['NVIDIA_API_KEY'] = 'nvapi-fake-key';
    const nonInteractiveConfig = createLocalMockConfig({});
    await validateNonInteractiveAuth(
      AuthType.USE_NVIDIA_NIM,
      undefined,
      nonInteractiveConfig,
      mockSettings,
    );
    expect(processExitSpy).not.toHaveBeenCalled();
    expect(debugLoggerErrorSpy).not.toHaveBeenCalled();
  });

  it('exits if validateAuthMethod returns error', async () => {
    // Mock validateAuthMethod to return error
    vi.spyOn(auth, 'validateAuthMethod').mockResolvedValue('Auth error!');
    const nonInteractiveConfig = createLocalMockConfig({
      getOutputFormat: vi.fn().mockReturnValue(OutputFormat.TEXT),
      getContentGeneratorConfig: vi
        .fn()
        .mockReturnValue({ authType: undefined }),
    });
    try {
      await validateNonInteractiveAuth(
        AuthType.USE_NVIDIA_NIM,
        undefined,
        nonInteractiveConfig,
        mockSettings,
      );
      expect.fail('Should have exited');
    } catch (e) {
      expect((e as Error).message).toContain(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
    }
    expect(debugLoggerErrorSpy).toHaveBeenCalledWith('Auth error!');
    expect(processExitSpy).toHaveBeenCalledWith(
      ExitCodes.FATAL_AUTHENTICATION_ERROR,
    );
  });

  it('skips validation if useExternalAuth is true', async () => {
    // Mock validateAuthMethod to return error to ensure it's not being called
    const validateAuthMethodSpy = vi
      .spyOn(auth, 'validateAuthMethod')
      .mockResolvedValue('Auth error!');
    const nonInteractiveConfig = createLocalMockConfig({});
    // Even with an invalid auth type, it should not exit
    // because validation is skipped.
    await validateNonInteractiveAuth(
      'invalid-auth-type' as AuthType,
      true, // useExternalAuth = true
      nonInteractiveConfig,
      mockSettings,
    );

    expect(validateAuthMethodSpy).not.toHaveBeenCalled();
    expect(debugLoggerErrorSpy).not.toHaveBeenCalled();
    expect(coreEventsEmitFeedbackSpy).not.toHaveBeenCalled();
    expect(processExitSpy).not.toHaveBeenCalled();
  });

  it('succeeds if effectiveAuthType matches enforcedType', async () => {
    mockSettings.merged.security.auth.enforcedType = AuthType.USE_NVIDIA_NIM;
    process.env['NVIDIA_API_KEY'] = 'nvapi-fake-key';
    const nonInteractiveConfig = createLocalMockConfig({});
    await validateNonInteractiveAuth(
      undefined,
      undefined,
      nonInteractiveConfig,
      mockSettings,
    );
    expect(processExitSpy).not.toHaveBeenCalled();
    expect(debugLoggerErrorSpy).not.toHaveBeenCalled();
  });

  it('exits if configuredAuthType does not match enforcedType', async () => {
    mockSettings.merged.security.auth.enforcedType = AuthType.USE_NVIDIA_NIM;
    const nonInteractiveConfig = createLocalMockConfig({
      getOutputFormat: vi.fn().mockReturnValue(OutputFormat.TEXT),
    });
    try {
      await validateNonInteractiveAuth(
        'oauth-personal' as AuthType,
        undefined,
        nonInteractiveConfig,
        mockSettings,
      );
      expect.fail('Should have exited');
    } catch (e) {
      expect((e as Error).message).toContain(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
    }
    expect(debugLoggerErrorSpy).toHaveBeenCalledWith(
      "The enforced authentication type is 'nvidia-nim', but the current type is 'oauth-personal'. Please re-authenticate with the correct type.",
    );
    expect(processExitSpy).toHaveBeenCalledWith(
      ExitCodes.FATAL_AUTHENTICATION_ERROR,
    );
  });

  it('exits if enforcedType is set and no NVIDIA_API_KEY is configured', async () => {
    vi.spyOn(auth, 'validateAuthMethod').mockResolvedValueOnce(
      'When using NVIDIA NIM, you must specify the NVIDIA_API_KEY environment variable.',
    );
    mockSettings.merged.security.auth.enforcedType = AuthType.USE_NVIDIA_NIM;
    const nonInteractiveConfig = createLocalMockConfig({
      getOutputFormat: vi.fn().mockReturnValue(OutputFormat.TEXT),
    });
    try {
      await validateNonInteractiveAuth(
        undefined,
        undefined,
        nonInteractiveConfig,
        mockSettings,
      );
      expect.fail('Should have exited');
    } catch (e) {
      expect((e as Error).message).toContain(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
    }
    expect(debugLoggerErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('NVIDIA_API_KEY'),
    );
    expect(processExitSpy).toHaveBeenCalledWith(
      ExitCodes.FATAL_AUTHENTICATION_ERROR,
    );
  });

  describe('JSON output mode', () => {
    it(`prints JSON error when no NVIDIA_API_KEY is configured and exits with code ${ExitCodes.FATAL_AUTHENTICATION_ERROR}`, async () => {
      vi.spyOn(auth, 'validateAuthMethod').mockResolvedValueOnce(
        'When using NVIDIA NIM, you must specify the NVIDIA_API_KEY environment variable.',
      );
      const nonInteractiveConfig = createLocalMockConfig({
        getOutputFormat: vi.fn().mockReturnValue(OutputFormat.JSON),
        getContentGeneratorConfig: vi
          .fn()
          .mockReturnValue({ authType: undefined }),
      });

      let thrown: Error | undefined;
      try {
        await validateNonInteractiveAuth(
          undefined,
          undefined,
          nonInteractiveConfig,
          mockSettings,
        );
      } catch (e) {
        thrown = e as Error;
      }

      expect(thrown?.message).toBe(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
      // Checking coreEventsEmitFeedbackSpy arguments
      const errorArg = coreEventsEmitFeedbackSpy.mock.calls[0]?.[1] as string;
      const payload = JSON.parse(errorArg);
      expect(payload.error.type).toBe('Error');
      expect(payload.error.code).toBe(ExitCodes.FATAL_AUTHENTICATION_ERROR);
      expect(payload.error.message).toContain('NVIDIA_API_KEY');
    });

    it(`prints JSON error when enforced auth mismatches current auth and exits with code ${ExitCodes.FATAL_AUTHENTICATION_ERROR}`, async () => {
      mockSettings.merged.security.auth.enforcedType = AuthType.USE_NVIDIA_NIM;
      const nonInteractiveConfig = createLocalMockConfig({
        getOutputFormat: vi.fn().mockReturnValue(OutputFormat.JSON),
        getContentGeneratorConfig: vi
          .fn()
          .mockReturnValue({ authType: undefined }),
      });

      let thrown: Error | undefined;
      try {
        await validateNonInteractiveAuth(
          'oauth-personal' as AuthType,
          undefined,
          nonInteractiveConfig,
          mockSettings,
        );
      } catch (e) {
        thrown = e as Error;
      }

      expect(thrown?.message).toBe(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
      {
        // Checking coreEventsEmitFeedbackSpy arguments
        const errorArg = coreEventsEmitFeedbackSpy.mock.calls[0]?.[1] as string;
        const payload = JSON.parse(errorArg);
        expect(payload.error.type).toBe('Error');
        expect(payload.error.code).toBe(ExitCodes.FATAL_AUTHENTICATION_ERROR);
        expect(payload.error.message).toContain(
          "The enforced authentication type is 'nvidia-nim', but the current type is 'oauth-personal'. Please re-authenticate with the correct type.",
        );
      }
    });

    it(`prints JSON error when validateAuthMethod fails and exits with code ${ExitCodes.FATAL_AUTHENTICATION_ERROR}`, async () => {
      vi.spyOn(auth, 'validateAuthMethod').mockResolvedValue('Auth error!');
      process.env['NVIDIA_API_KEY'] = 'nvapi-fake-key';

      const nonInteractiveConfig = createLocalMockConfig({
        getOutputFormat: vi.fn().mockReturnValue(OutputFormat.JSON),
        getContentGeneratorConfig: vi
          .fn()
          .mockReturnValue({ authType: undefined }),
      });

      let thrown: Error | undefined;
      try {
        await validateNonInteractiveAuth(
          AuthType.USE_NVIDIA_NIM,
          undefined,
          nonInteractiveConfig,
          mockSettings,
        );
      } catch (e) {
        thrown = e as Error;
      }

      expect(thrown?.message).toBe(
        `process.exit(${ExitCodes.FATAL_AUTHENTICATION_ERROR}) called`,
      );
      {
        // Checking coreEventsEmitFeedbackSpy arguments
        const errorArg = coreEventsEmitFeedbackSpy.mock.calls[0]?.[1] as string;
        const payload = JSON.parse(errorArg);
        expect(payload.error.type).toBe('Error');
        expect(payload.error.code).toBe(ExitCodes.FATAL_AUTHENTICATION_ERROR);
        expect(payload.error.message).toBe('Auth error!');
      }
    });
  });
});
