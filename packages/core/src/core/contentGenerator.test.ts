/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createContentGenerator,
  AuthType,
  createContentGeneratorConfig,
  getAuthTypeFromEnv,
  type ContentGenerator,
} from './contentGenerator.js';
import type { Config } from '../config/config.js';
import { LoggingContentGenerator } from './loggingContentGenerator.js';
import { loadApiKey } from './apiKeyCredentialStorage.js';
import { FakeContentGenerator } from './fakeContentGenerator.js';
import { RecordingContentGenerator } from './recordingContentGenerator.js';
import { resetVersionCache } from '../utils/version.js';
import {
  NvidiaNimContentGenerator,
  NVIDIA_NIM_DEFAULT_BASE_URL,
  NVIDIA_NIM_DEFAULT_MODEL,
} from './nvidiaNimContentGenerator.js';

vi.mock('./apiKeyCredentialStorage.js', () => ({
  loadApiKey: vi.fn(),
}));

vi.mock('./fakeContentGenerator.js');

vi.mock('./nvidiaNimContentGenerator.js', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('./nvidiaNimContentGenerator.js')>();
  return {
    ...actual,
    NvidiaNimContentGenerator: vi.fn().mockImplementation((cfg) => ({
      config: cfg,
      generateContent: vi.fn(),
      generateContentStream: vi.fn(),
      countTokens: vi.fn(),
      embedContent: vi.fn(),
    })),
  };
});

const mockConfig = {
  getModel: vi.fn().mockReturnValue(NVIDIA_NIM_DEFAULT_MODEL),
  getProxy: vi.fn().mockReturnValue(undefined),
  getUsageStatisticsEnabled: vi.fn().mockReturnValue(true),
  getClientName: vi.fn().mockReturnValue(undefined),
  getTelemetryLogPromptsEnabled: vi.fn().mockReturnValue(true),
  getTelemetryTracesEnabled: vi.fn().mockReturnValue(true),
  getSessionId: vi.fn().mockReturnValue('test-session-id'),
  setLatestApiRequest: vi.fn(),
  getContentGeneratorConfig: vi.fn().mockReturnValue({}),
  isInteractive: vi.fn().mockReturnValue(false),
  getExperiments: vi.fn().mockReturnValue(undefined),
} as unknown as Config;

describe('getAuthTypeFromEnv', () => {
  beforeEach(() => {
    vi.stubEnv('NVIDIA_API_KEY', '');
    vi.stubEnv('GEMINI_API_KEY', '');
    vi.stubEnv('GOOGLE_GENAI_USE_GCA', '');
    vi.stubEnv('GOOGLE_GENAI_USE_VERTEXAI', '');
    vi.stubEnv('GOOGLE_GEMINI_BASE_URL', '');
    vi.stubEnv('CLOUD_SHELL', '');
    vi.stubEnv('M31A_USE_COMPUTE_ADC', '');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('should detect USE_NVIDIA_NIM when NVIDIA_API_KEY is present', () => {
    vi.stubEnv('NVIDIA_API_KEY', 'nvapi-test-key');
    expect(getAuthTypeFromEnv()).toBe(AuthType.USE_NVIDIA_NIM);
  });

  it('should prefer USE_NVIDIA_NIM even if Google env vars are also set', () => {
    vi.stubEnv('NVIDIA_API_KEY', 'nvapi-test-key');
    vi.stubEnv('GEMINI_API_KEY', 'fake-key');
    expect(getAuthTypeFromEnv()).toBe(AuthType.USE_NVIDIA_NIM);
  });

  it('should not return Google AuthTypes even if Google env vars are set', () => {
    vi.stubEnv('GOOGLE_GENAI_USE_GCA', 'true');
    vi.stubEnv('GOOGLE_GENAI_USE_VERTEXAI', 'true');
    vi.stubEnv('GOOGLE_GEMINI_BASE_URL', 'https://gateway.example.com');
    vi.stubEnv('GEMINI_API_KEY', 'fake-key');
    vi.stubEnv('CLOUD_SHELL', 'true');
    expect(getAuthTypeFromEnv()).toBeUndefined();
  });

  it('should return undefined when no matching env variables are set', () => {
    expect(getAuthTypeFromEnv()).toBeUndefined();
  });
});

describe('createContentGenerator', () => {
  beforeEach(() => {
    resetVersionCache();
    vi.clearAllMocks();
    vi.stubEnv('ANTIGRAVITY_CLI_ALIAS', '');
    vi.stubEnv('NVIDIA_API_KEY', '');
    vi.stubEnv('NVIDIA_NIM_BASE_URL', '');
    vi.stubEnv('NVIDIA_NIM_MODEL', '');
    vi.stubEnv('M31A_CUSTOM_HEADERS', '');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('should create a FakeContentGenerator', async () => {
    const mockGenerator = {} as unknown as ContentGenerator;
    vi.mocked(FakeContentGenerator.fromFile).mockResolvedValue(
      mockGenerator as never,
    );
    const fakeResponsesFile = 'fake/responses.yaml';
    const mockConfigWithFake = {
      fakeResponses: fakeResponsesFile,
      getClientName: vi.fn().mockReturnValue(undefined),
    } as unknown as Config;
    const generator = await createContentGenerator(
      {
        authType: AuthType.USE_NVIDIA_NIM,
      },
      mockConfigWithFake,
    );
    expect(FakeContentGenerator.fromFile).toHaveBeenCalledWith(
      fakeResponsesFile,
    );
    expect(generator).toEqual(
      new LoggingContentGenerator(mockGenerator, mockConfigWithFake),
    );
  });

  it('should create a RecordingContentGenerator', async () => {
    const fakeResponsesFile = 'fake/responses.yaml';
    const recordResponsesFile = 'record/responses.yaml';
    const mockConfigWithRecordResponses = {
      fakeResponses: fakeResponsesFile,
      recordResponses: recordResponsesFile,
      getClientName: vi.fn().mockReturnValue(undefined),
    } as unknown as Config;
    const generator = await createContentGenerator(
      {
        authType: AuthType.USE_NVIDIA_NIM,
      },
      mockConfigWithRecordResponses,
    );
    expect(generator).toBeInstanceOf(RecordingContentGenerator);
  });

  it('should create an NvidiaNimContentGenerator for AuthType.USE_NVIDIA_NIM', async () => {
    const nimConfig = {
      getModel: vi.fn().mockReturnValue(NVIDIA_NIM_DEFAULT_MODEL),
      getProxy: vi.fn().mockReturnValue(undefined),
      getUsageStatisticsEnabled: () => false,
      getClientName: vi.fn().mockReturnValue(undefined),
    } as unknown as Config;

    vi.stubEnv('CLI_VERSION', '1.2.3');
    vi.stubEnv('TERM_PROGRAM', 'iTerm.app');
    vi.stubEnv('VSCODE_PID', '');
    vi.stubEnv('GITHUB_SHA', '');
    vi.stubEnv('M31A_SURFACE', '');

    const generator = await createContentGenerator(
      {
        apiKey: 'nvapi-test-key',
        authType: AuthType.USE_NVIDIA_NIM,
      },
      nimConfig,
    );

    expect(NvidiaNimContentGenerator).toHaveBeenCalledWith(
      expect.objectContaining({
        apiKey: 'nvapi-test-key',
        baseUrl: NVIDIA_NIM_DEFAULT_BASE_URL,
        model: NVIDIA_NIM_DEFAULT_MODEL,
        customHeaders: expect.objectContaining({
          'User-Agent': expect.stringMatching(
            new RegExp(`M31A/1\\.2\\.3/.* \\(.*; .*; terminal\\)`),
          ),
        }),
      }),
    );
    expect(generator).toBeInstanceOf(LoggingContentGenerator);
  });

  it('should include clientName prefix in User-Agent when specified', async () => {
    const nimConfig = {
      getModel: vi.fn().mockReturnValue(NVIDIA_NIM_DEFAULT_MODEL),
      getProxy: vi.fn().mockReturnValue(undefined),
      getUsageStatisticsEnabled: () => false,
      getClientName: vi.fn().mockReturnValue('my-client'),
    } as unknown as Config;

    vi.stubEnv('CLI_VERSION', '1.2.3');
    vi.stubEnv('TERM_PROGRAM', 'iTerm.app');
    vi.stubEnv('VSCODE_PID', '');
    vi.stubEnv('GITHUB_SHA', '');
    vi.stubEnv('M31A_SURFACE', '');

    await createContentGenerator(
      { apiKey: 'nvapi-test-key', authType: AuthType.USE_NVIDIA_NIM },
      nimConfig,
    );

    expect(NvidiaNimContentGenerator).toHaveBeenCalledWith(
      expect.objectContaining({
        customHeaders: expect.objectContaining({
          'User-Agent': expect.stringMatching(
            /M31A-my-client\/1\.2\.3\/.* \(.*; .*; terminal\)/,
          ),
        }),
      }),
    );
  });

  it('should include custom headers from M31A_CUSTOM_HEADERS and strip x-goog-api-key', async () => {
    const nimConfig = {
      getModel: vi.fn().mockReturnValue(NVIDIA_NIM_DEFAULT_MODEL),
      getProxy: vi.fn().mockReturnValue(undefined),
      getUsageStatisticsEnabled: () => false,
      getClientName: vi.fn().mockReturnValue(undefined),
    } as unknown as Config;

    vi.stubEnv('M31A_CUSTOM_HEADERS', 'X-Test-Header: test, Another: value');

    await createContentGenerator(
      {
        apiKey: 'nvapi-test-key',
        authType: AuthType.USE_NVIDIA_NIM,
        customHeaders: { 'x-goog-api-key': 'should-be-stripped' },
      },
      nimConfig,
    );

    const callArg = vi.mocked(NvidiaNimContentGenerator).mock.calls[0]?.[0];
    expect(callArg?.customHeaders).toMatchObject({
      'X-Test-Header': 'test',
      Another: 'value',
    });
    expect(callArg?.customHeaders).not.toHaveProperty('x-goog-api-key');
  });

  it('should pass custom baseUrl and model from config/env to NvidiaNimContentGenerator', async () => {
    vi.stubEnv('NVIDIA_NIM_BASE_URL', 'https://nim.env.local/v1');
    vi.stubEnv('NVIDIA_NIM_MODEL', 'nvidia/custom-model');

    await createContentGenerator(
      {
        apiKey: 'nvapi-test-key',
        authType: AuthType.USE_NVIDIA_NIM,
        baseUrl: 'https://nim.explicit.local/v1',
      },
      mockConfig,
    );

    expect(NvidiaNimContentGenerator).toHaveBeenCalledWith(
      expect.objectContaining({
        baseUrl: 'https://nim.explicit.local/v1',
        model: 'nvidia/custom-model',
      }),
    );
  });

  it('should reject invalid custom baseUrl values', async () => {
    await expect(
      createContentGenerator(
        {
          apiKey: 'nvapi-test-key',
          authType: AuthType.USE_NVIDIA_NIM,
          baseUrl: 'not-a-url',
        },
        mockConfig,
      ),
    ).rejects.toThrow('Invalid custom base URL: not-a-url');
  });

  it.each([
    'oauth-personal',
    'gemini-api-key',
    'vertex-ai',
    'cloud-shell',
    'compute-default-credentials',
  ] as unknown as AuthType[])(
    'should reject unsupported Google authType %s (NIM only)',
    async (authType) => {
      await expect(
        createContentGenerator({ authType }, mockConfig),
      ).rejects.toThrow('Unsupported authType');
    },
  );
});

describe('createContentGeneratorConfig', () => {
  const mockCfg = {
    getModel: vi.fn().mockReturnValue(NVIDIA_NIM_DEFAULT_MODEL),
    setModel: vi.fn(),
    flashFallbackHandler: vi.fn(),
    getProxy: vi.fn(),
    getClientName: vi.fn().mockReturnValue(undefined),
  } as unknown as Config;

  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    vi.stubEnv('NVIDIA_API_KEY', '');
    vi.stubEnv('NVIDIA_NIM_BASE_URL', '');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('should configure for NVIDIA NIM using NVIDIA_API_KEY and default base URL', async () => {
    vi.stubEnv('NVIDIA_API_KEY', 'nvapi-env-key');
    const config = await createContentGeneratorConfig(
      mockCfg,
      AuthType.USE_NVIDIA_NIM,
    );
    expect(config.apiKey).toBe('nvapi-env-key');
    expect(config.baseUrl).toBe(NVIDIA_NIM_DEFAULT_BASE_URL);
  });

  it('should prefer explicit apiKey and baseUrl for NVIDIA NIM', async () => {
    vi.stubEnv('NVIDIA_API_KEY', 'nvapi-env-key');
    vi.stubEnv('NVIDIA_NIM_BASE_URL', 'https://nim.env.local/v1');
    const config = await createContentGeneratorConfig(
      mockCfg,
      AuthType.USE_NVIDIA_NIM,
      'nvapi-explicit-key',
      'https://nim.explicit.local/v1',
    );
    expect(config.apiKey).toBe('nvapi-explicit-key');
    expect(config.baseUrl).toBe('https://nim.explicit.local/v1');
  });

  it('should fall back to stored API key when NVIDIA_API_KEY env var is unset', async () => {
    vi.mocked(loadApiKey).mockResolvedValue('nvapi-keychain-key');
    const config = await createContentGeneratorConfig(
      mockCfg,
      AuthType.USE_NVIDIA_NIM,
    );
    expect(config.apiKey).toBe('nvapi-keychain-key');
  });

  it('should ignore Google auth types in createContentGeneratorConfig', async () => {
    vi.stubEnv('GEMINI_API_KEY', 'env-gemini-key');
    const config = await createContentGeneratorConfig(
      mockCfg,
      'gemini-api-key' as unknown as AuthType,
    );
    expect(config.apiKey).toBeUndefined();
  });
});
