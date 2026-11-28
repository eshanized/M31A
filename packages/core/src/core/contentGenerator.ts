/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  type CountTokensResponse,
  type GenerateContentResponse,
  type GenerateContentParameters,
  type CountTokensParameters,
  type EmbedContentResponse,
  type EmbedContentParameters,
} from '@m31a/genai';
import * as os from 'node:os';
import { isCloudShell } from '../ide/detect-ide.js';
import type { Config } from '../config/config.js';
import { loadApiKey } from './apiKeyCredentialStorage.js';

import type { UserTierId } from '../config/tierTypes.js';
import { LoggingContentGenerator } from './loggingContentGenerator.js';
import { FakeContentGenerator } from './fakeContentGenerator.js';
import { parseCustomHeaders } from '../utils/customHeaderUtils.js';
import { determineSurface } from '../utils/surface.js';
import { RecordingContentGenerator } from './recordingContentGenerator.js';
import { getVersion, resolveModel } from '../../index.js';
import type { LlmRole } from '../telemetry/llmRole.js';
import {
  NvidiaNimContentGenerator,
  NVIDIA_NIM_API_KEY_ENV,
  NVIDIA_NIM_BASE_URL_ENV,
  NVIDIA_NIM_DEFAULT_BASE_URL,
  NVIDIA_NIM_DEFAULT_MODEL,
  NVIDIA_NIM_MODEL_ENV,
} from './nvidiaNimContentGenerator.js';

/**
 * Interface abstracting the core functionalities for generating content and counting tokens.
 */
export interface ContentGenerator {
  generateContent(
    request: GenerateContentParameters,
    userPromptId: string,
    role: LlmRole,
  ): Promise<GenerateContentResponse>;

  generateContentStream(
    request: GenerateContentParameters,
    userPromptId: string,
    role: LlmRole,
  ): Promise<AsyncGenerator<GenerateContentResponse>>;

  countTokens(request: CountTokensParameters): Promise<CountTokensResponse>;

  embedContent(request: EmbedContentParameters): Promise<EmbedContentResponse>;

  userTier?: UserTierId;

  userTierName?: string;
}

export enum AuthType {
  /**
   * NVIDIA NIM (OpenAI-compatible). The M31A single provider.
   * API key from NVIDIA_API_KEY. No Google auth.
   */
  USE_NVIDIA_NIM = 'nvidia-nim',
}

/**
 * Detects the best authentication type based on environment variables.
 *
 * M31A uses NVIDIA NIM only (`NVIDIA_API_KEY`).
 */
export function getAuthTypeFromEnv(): AuthType | undefined {
  if (process.env[NVIDIA_NIM_API_KEY_ENV]) {
    return AuthType.USE_NVIDIA_NIM;
  }
  return undefined;
}

export type ContentGeneratorConfig = {
  apiKey?: string;
  authType?: AuthType;
  proxy?: string;
  baseUrl?: string;
  customHeaders?: Record<string, string>;
};

function validateBaseUrl(baseUrl: string): void {
  try {
    new URL(baseUrl);
  } catch {
    throw new Error(`Invalid custom base URL: ${baseUrl}`);
  }
}

export async function createContentGeneratorConfig(
  config: Config,
  authType: AuthType | undefined,
  apiKey?: string,
  baseUrl?: string,
  customHeaders?: Record<string, string>,
): Promise<ContentGeneratorConfig> {
  const contentGeneratorConfig: ContentGeneratorConfig = {
    authType,
    proxy: config?.getProxy(),
    baseUrl,
    customHeaders,
  };

  const getEnv = (key: string) => {
    if (config?.env && config.env[key] !== undefined) {
      return config.env[key];
    }
    return process.env[key];
  };

  // M31A: NVIDIA NIM single provider (API-key only via NVIDIA_API_KEY or keychain).
  if (authType === AuthType.USE_NVIDIA_NIM) {
    contentGeneratorConfig.apiKey =
      apiKey || getEnv(NVIDIA_NIM_API_KEY_ENV) || (await loadApiKey()) || '';
    contentGeneratorConfig.baseUrl =
      baseUrl || getEnv(NVIDIA_NIM_BASE_URL_ENV) || NVIDIA_NIM_DEFAULT_BASE_URL;
    return contentGeneratorConfig;
  }

  return contentGeneratorConfig;
}

export async function createContentGenerator(
  config: ContentGeneratorConfig,
  gcConfig: Config,
  _sessionId?: string,
): Promise<ContentGenerator> {
  const generator = await (async () => {
    if (gcConfig.fakeResponsesNonStrict) {
      const fakeGenerator = await FakeContentGenerator.fromFile(
        gcConfig.fakeResponsesNonStrict,
        { nonStrict: true },
      );
      return new LoggingContentGenerator(fakeGenerator, gcConfig);
    }
    if (gcConfig.fakeResponses) {
      const fakeGenerator = await FakeContentGenerator.fromFile(
        gcConfig.fakeResponses,
      );
      return new LoggingContentGenerator(fakeGenerator, gcConfig);
    }
    const version = await getVersion();
    const model = resolveModel(gcConfig.getModel(), gcConfig);
    const customHeadersEnv = process.env['M31A_CUSTOM_HEADERS'] || undefined;
    const clientName = gcConfig.getClientName();
    const surface = determineSurface();

    let userAgent: string;
    if (clientName === 'acp-vscode' || surface === 'vscode') {
      const osTypeMap: Record<string, string> = {
        darwin: 'macOS',
        win32: 'Windows',
        linux: 'Linux',
      };
      const osType = osTypeMap[process.platform] || process.platform;
      const osVersion = os.release();
      const arch = process.arch;

      const vscodeVersion = process.env['TERM_PROGRAM_VERSION'] || 'unknown';
      let hostPath = `VSCode/${vscodeVersion}`;
      if (isCloudShell()) {
        const cloudShellVersion =
          process.env['CLOUD_SHELL_VERSION'] || 'unknown';
        hostPath += ` > CloudShell/${cloudShellVersion}`;
      }

      userAgent = `CloudCodeVSCode/${version} (aidev_client; os_type=${osType}; os_version=${osVersion}; arch=${arch}; host_path=${hostPath}; proxy_client=m31a)`;
    } else {
      const userAgentPrefix = clientName ? `M31A-${clientName}` : 'M31A';
      userAgent = `${userAgentPrefix}/${version}/${model} (${process.platform}; ${process.arch}; ${surface})`;
    }

    const customHeadersMap = parseCustomHeaders(customHeadersEnv);
    const baseHeaders: Record<string, string> = {
      'User-Agent': userAgent,
      ...customHeadersMap,
    };

    // M31A: NVIDIA NIM single provider (OpenAI-compatible).
    if (config.authType === AuthType.USE_NVIDIA_NIM) {
      const resolvedBaseUrl =
        config.baseUrl ||
        process.env[NVIDIA_NIM_BASE_URL_ENV] ||
        NVIDIA_NIM_DEFAULT_BASE_URL;
      validateBaseUrl(resolvedBaseUrl);
      const nimCustomHeaders: Record<string, string> = {
        ...baseHeaders,
        ...(config.customHeaders ?? {}),
      };
      delete nimCustomHeaders['x-goog-api-key'];
      const nim = new NvidiaNimContentGenerator({
        apiKey: config.apiKey ?? process.env[NVIDIA_NIM_API_KEY_ENV] ?? '',
        baseUrl: resolvedBaseUrl,
        model:
          process.env[NVIDIA_NIM_MODEL_ENV] ||
          (gcConfig.getModel().startsWith('nvidia/')
            ? gcConfig.getModel()
            : NVIDIA_NIM_DEFAULT_MODEL),
        customHeaders: nimCustomHeaders,
        proxy: config.proxy,
      });
      return new LoggingContentGenerator(nim, gcConfig);
    }

    throw new Error(
      `Error creating contentGenerator: Unsupported authType: ${config.authType} (M31A uses NVIDIA NIM only)`,
    );
  })();

  if (gcConfig.recordResponses) {
    return new RecordingContentGenerator(generator, gcConfig.recordResponses);
  }

  return generator;
}
