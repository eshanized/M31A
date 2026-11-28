/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act } from 'react';
import { renderHook } from '../../test-utils/render.js';
import { useAuthCommand, validateAuthMethodWithSettings } from './useAuth.js';
import { AuthType, type Config } from '@m31a/core';
import { AuthState } from '../types.js';
import type { LoadedSettings } from '../../config/settings.js';

// Mock dependencies
const mockLoadApiKey = vi.fn();
const mockValidateAuthMethod = vi.fn();

vi.mock('@m31a/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@m31a/core')>();
  return {
    ...actual,
    loadApiKey: () => mockLoadApiKey(),
  };
});

vi.mock('../../config/auth.js', () => ({
  validateAuthMethod: (authType: AuthType) => mockValidateAuthMethod(authType),
}));

describe('useAuth', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    delete process.env['NVIDIA_API_KEY'];
    delete process.env['NVIDIA_API_KEY'];
    delete process.env['M31A_DEFAULT_AUTH_TYPE'];
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('validateAuthMethodWithSettings', () => {
    it('should return error if auth type is enforced and does not match', async () => {
      const settings = {
        merged: {
          security: {
            auth: {
              enforcedType: AuthType.USE_NVIDIA_NIM,
            },
          },
        },
      } as LoadedSettings;

      const error = await validateAuthMethodWithSettings(
        'oauth-personal' as AuthType,
        settings,
      );
      expect(error).toContain('Authentication is enforced to be nvidia-nim');
    });

    it('should return null if useExternal is true', async () => {
      const settings = {
        merged: {
          security: {
            auth: {
              useExternal: true,
            },
          },
        },
      } as LoadedSettings;

      const error = await validateAuthMethodWithSettings(
        AuthType.USE_NVIDIA_NIM,
        settings,
      );
      expect(error).toBeNull();
    });

    it('should return null if authType is USE_NVIDIA_NIM', async () => {
      const settings = {
        merged: {
          security: {
            auth: {},
          },
        },
      } as LoadedSettings;

      const error = await validateAuthMethodWithSettings(
        AuthType.USE_NVIDIA_NIM,
        settings,
      );
      expect(error).toBeNull();
    });

    it('should call validateAuthMethod for other auth types', async () => {
      const settings = {
        merged: {
          security: {
            auth: {},
          },
        },
      } as LoadedSettings;

      mockValidateAuthMethod.mockResolvedValue('Validation Error');
      const error = await validateAuthMethodWithSettings(
        'other-auth-type' as AuthType,
        settings,
      );
      expect(error).toBe('Validation Error');
      expect(mockValidateAuthMethod).toHaveBeenCalledWith('other-auth-type');
    });
  });

  describe('useAuthCommand', () => {
    const mockConfig = {
      refreshAuth: vi.fn(),
    } as unknown as Config;

    const createSettings = (selectedType?: AuthType) =>
      ({
        merged: {
          security: {
            auth: {
              selectedType,
            },
          },
        },
      }) as LoadedSettings;

    let deferredRefreshAuth: {
      resolve: () => void;
      reject: (e: Error) => void;
    };

    beforeEach(() => {
      mockLoadApiKey.mockResolvedValue('stored-key');
      vi.mocked(mockConfig.refreshAuth).mockImplementation(
        () =>
          new Promise((resolve, reject) => {
            deferredRefreshAuth = { resolve, reject };
          }),
      );
    });

    it('should initialize with Unauthenticated state', async () => {
      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );
      // Because we defer refreshAuth, the initial state is safely caught here
      expect(result.current.authState).toBe(AuthState.Unauthenticated);

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(result.current.authState).toBe(AuthState.Authenticated);
    });

    it('should authenticate directly with stored key when no selectedType is set', async () => {
      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(undefined), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(mockConfig.refreshAuth).toHaveBeenCalledWith(
        AuthType.USE_NVIDIA_NIM,
      );
      expect(result.current.authState).toBe(AuthState.Authenticated);
      expect(result.current.authError).toBeNull();
    });

    it('should authenticate directly when NVIDIA_API_KEY env var is set and no selectedType is set', async () => {
      process.env['NVIDIA_API_KEY'] = 'env-key';
      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(undefined), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(mockConfig.refreshAuth).toHaveBeenCalledWith(
        AuthType.USE_NVIDIA_NIM,
      );
      expect(result.current.authState).toBe(AuthState.Authenticated);
      expect(result.current.apiKeyDefaultValue).toBe('env-key');
    });

    it('should transition to AwaitingApiKeyInput when no selectedType and no key found', async () => {
      let deferredLoadKey: { resolve: (k: string | null) => void };
      mockLoadApiKey.mockImplementation(
        () =>
          new Promise((resolve) => {
            deferredLoadKey = { resolve };
          }),
      );

      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(undefined), mockConfig),
      );

      await act(async () => {
        deferredLoadKey.resolve(null);
      });

      expect(result.current.authState).toBe(AuthState.AwaitingApiKeyInput);
    });

    it('should transition to AwaitingApiKeyInput if USE_NVIDIA_NIM and no key found', async () => {
      let deferredLoadKey: { resolve: (k: string | null) => void };
      mockLoadApiKey.mockImplementation(
        () =>
          new Promise((resolve) => {
            deferredLoadKey = { resolve };
          }),
      );

      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredLoadKey.resolve(null);
      });

      expect(result.current.authState).toBe(AuthState.AwaitingApiKeyInput);
    });

    it('should authenticate if USE_NVIDIA_NIM and key is found', async () => {
      let deferredLoadKey: { resolve: (k: string | null) => void };
      mockLoadApiKey.mockImplementation(
        () =>
          new Promise((resolve) => {
            deferredLoadKey = { resolve };
          }),
      );

      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredLoadKey.resolve('stored-key');
      });

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(mockConfig.refreshAuth).toHaveBeenCalledWith(
        AuthType.USE_NVIDIA_NIM,
      );
      expect(result.current.authState).toBe(AuthState.Authenticated);
      expect(result.current.apiKeyDefaultValue).toBe('stored-key');
    });

    it('should authenticate if USE_NVIDIA_NIM and env key is found', async () => {
      process.env['NVIDIA_API_KEY'] = 'env-key';

      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(mockConfig.refreshAuth).toHaveBeenCalledWith(
        AuthType.USE_NVIDIA_NIM,
      );
      expect(result.current.authState).toBe(AuthState.Authenticated);
      expect(result.current.apiKeyDefaultValue).toBe('env-key');
    });

    it('should prioritize env key over stored key when both are present', async () => {
      process.env['NVIDIA_API_KEY'] = 'env-key';

      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(mockConfig.refreshAuth).toHaveBeenCalledWith(
        AuthType.USE_NVIDIA_NIM,
      );
      expect(result.current.authState).toBe(AuthState.Authenticated);
      expect(result.current.apiKeyDefaultValue).toBe('env-key');
    });

    it('should set error if validation fails', async () => {
      mockValidateAuthMethod.mockResolvedValue('Validation Failed');
      const { result } = await renderHook(() =>
        useAuthCommand(
          createSettings('other-auth-type' as AuthType),
          mockConfig,
        ),
      );

      expect(result.current.authError).toBe('Validation Failed');
      expect(result.current.authState).toBe(AuthState.AwaitingApiKeyInput);
    });

    it('should authenticate successfully for valid auth type', async () => {
      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.resolve();
      });

      expect(mockConfig.refreshAuth).toHaveBeenCalledWith(
        AuthType.USE_NVIDIA_NIM,
      );
      expect(result.current.authState).toBe(AuthState.Authenticated);
      expect(result.current.authError).toBeNull();
    });

    it('should handle refreshAuth failure', async () => {
      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.reject(new Error('Auth Failed'));
      });

      expect(result.current.authError).toContain('Failed to sign in');
      expect(result.current.authState).toBe(AuthState.AwaitingApiKeyInput);
    });

    it('should transition to AwaitingApiKeyInput without setting authError on ChangeAuthRequestedError', async () => {
      const { ChangeAuthRequestedError } = await import('@m31a/core');
      const { result } = await renderHook(() =>
        useAuthCommand(createSettings(AuthType.USE_NVIDIA_NIM), mockConfig),
      );

      await act(async () => {
        deferredRefreshAuth.reject(new ChangeAuthRequestedError());
      });

      expect(result.current.authError).toBeNull();
      expect(result.current.authState).toBe(AuthState.AwaitingApiKeyInput);
    });
  });
});
