/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { AuthType } from '@m31a/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { validateAuthMethod } from './auth.js';

vi.mock('@m31a/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@m31a/core')>();
  return {
    ...actual,
    loadApiKey: vi.fn().mockResolvedValue(null),
  };
});

vi.mock('./settings.js', () => ({
  loadEnvironment: vi.fn(),
  loadSettings: vi.fn().mockReturnValue({
    merged: vi.fn().mockReturnValue({}),
  }),
}));

describe('validateAuthMethod', () => {
  beforeEach(() => {
    vi.stubEnv('NVIDIA_API_KEY', undefined);
    vi.stubEnv('GEMINI_API_KEY', undefined);
    vi.stubEnv('GOOGLE_CLOUD_PROJECT', undefined);
    vi.stubEnv('GOOGLE_CLOUD_LOCATION', undefined);
    vi.stubEnv('GOOGLE_API_KEY', undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it.each([
    {
      description:
        'should return null for USE_NVIDIA_NIM if NVIDIA_API_KEY is set',
      authType: AuthType.USE_NVIDIA_NIM,
      envs: { NVIDIA_API_KEY: 'nvapi-test-key' },
      expected: null,
    },
    {
      description:
        'should return an error message for USE_NVIDIA_NIM if NVIDIA_API_KEY is not set',
      authType: AuthType.USE_NVIDIA_NIM,
      envs: {},
      expected:
        'When using NVIDIA NIM, you must specify the NVIDIA_API_KEY environment variable.\n' +
        'Update your environment and try again (no reload needed if using .env)!',
    },
    {
      description: 'should reject LOGIN_WITH_GOOGLE',
      authType: 'oauth-personal' as AuthType,
      envs: {},
      expected:
        'Invalid auth method selected. M31A uses NVIDIA NIM only (NVIDIA_API_KEY).',
    },
    {
      description: 'should reject COMPUTE_ADC',
      authType: 'compute-default-credentials' as AuthType,
      envs: {},
      expected:
        'Invalid auth method selected. M31A uses NVIDIA NIM only (NVIDIA_API_KEY).',
    },
    {
      description: 'should reject USE_GEMINI',
      authType: 'gemini-api-key' as AuthType,
      envs: { GEMINI_API_KEY: 'test-key' },
      expected:
        'Invalid auth method selected. M31A uses NVIDIA NIM only (NVIDIA_API_KEY).',
    },
    {
      description: 'should reject USE_VERTEX_AI',
      authType: 'vertex-ai' as AuthType,
      envs: { GOOGLE_API_KEY: 'test-api-key' },
      expected:
        'Invalid auth method selected. M31A uses NVIDIA NIM only (NVIDIA_API_KEY).',
    },
    {
      description: 'should reject an invalid auth method',
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      authType: 'invalid-method' as any,
      envs: {},
      expected:
        'Invalid auth method selected. M31A uses NVIDIA NIM only (NVIDIA_API_KEY).',
    },
  ])('$description', async ({ authType, envs, expected }) => {
    for (const [key, value] of Object.entries(envs)) {
      vi.stubEnv(key, value as string);
    }
    expect(await validateAuthMethod(authType)).toBe(expected);
  });
});
