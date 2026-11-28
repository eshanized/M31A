/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { type AgentLoopContext, AuthType, getVersion } from '@m31a/core';
import * as acp from '@agentclientprotocol/sdk';
import { z } from 'zod';
import { SettingScope, type LoadedSettings } from '../config/settings.js';
import type { CliArgs } from '../config/config.js';
import { getAcpErrorMessage } from './acpErrors.js';
import { AcpSessionManager, type AuthDetails } from './acpSessionManager.js';
import { hasMeta } from './acpUtils.js';

export class GeminiAgent {
  private apiKey: string | undefined;
  private baseUrl: string | undefined;
  private customHeaders: Record<string, string> | undefined;
  private sessionManager: AcpSessionManager;

  constructor(
    private context: AgentLoopContext,
    private settings: LoadedSettings,
    argv: CliArgs,
    connection: acp.AgentSideConnection,
  ) {
    this.sessionManager = new AcpSessionManager(settings, argv, connection);
  }

  async dispose(): Promise<void> {
    await this.sessionManager.dispose();
  }

  async initialize(
    args: acp.InitializeRequest,
  ): Promise<acp.InitializeResponse> {
    if (args.clientCapabilities) {
      this.sessionManager.setClientCapabilities(args.clientCapabilities);
    }

    const authMethods = [
      {
        id: AuthType.USE_NVIDIA_NIM,
        name: 'NVIDIA NIM API key',
        description:
          'Use an NVIDIA_API_KEY with NVIDIA NIM (https://integrate.api.nvidia.com/v1)',
        _meta: {
          'api-key': {
            provider: 'nvidia-nim',
          },
        },
      },
    ];

    await this.context.config.initialize();
    const version = await getVersion();
    return {
      protocolVersion: acp.PROTOCOL_VERSION,
      authMethods,
      agentInfo: {
        name: 'm31a',
        title: 'M31A',
        version,
      },
      agentCapabilities: {
        loadSession: true,
        promptCapabilities: {
          image: true,
          audio: true,
          embeddedContext: true,
        },
        mcpCapabilities: {
          http: true,
          sse: true,
        },
      },
    };
  }

  async authenticate(req: acp.AuthenticateRequest): Promise<void> {
    const { methodId } = req;
    const method = z.nativeEnum(AuthType).parse(methodId);
    // Check for api-key in _meta
    const meta = hasMeta(req) ? req._meta : undefined;
    const apiKey =
      typeof meta?.['api-key'] === 'string' ? meta['api-key'] : undefined;

    // Refresh auth with the requested method
    // This will reuse existing credentials if they're valid,
    // or perform new authentication if needed
    try {
      if (apiKey) {
        this.apiKey = apiKey;
      }

      // Extract gateway details if present
      const gatewaySchema = z.object({
        baseUrl: z.string().optional(),
        headers: z.record(z.string()).optional(),
      });

      let baseUrl: string | undefined;
      let headers: Record<string, string> | undefined;

      if (meta?.['gateway']) {
        const result = gatewaySchema.safeParse(meta['gateway']);
        if (result.success) {
          baseUrl = result.data.baseUrl;
          headers = result.data.headers;
        } else {
          throw new acp.RequestError(
            -32602,
            `Malformed gateway payload: ${result.error.message}`,
          );
        }
      }

      this.baseUrl = baseUrl;
      this.customHeaders = headers;

      await this.context.config.refreshAuth(
        method,
        apiKey ?? this.apiKey,
        baseUrl,
        headers,
      );
    } catch (e) {
      throw new acp.RequestError(-32000, getAcpErrorMessage(e));
    }
    this.settings.setValue(
      SettingScope.User,
      'security.auth.selectedType',
      method,
    );
  }

  private getAuthDetails(): AuthDetails {
    return {
      apiKey: this.apiKey,
      baseUrl: this.baseUrl,
      customHeaders: this.customHeaders,
    };
  }

  async newSession(
    params: acp.NewSessionRequest,
  ): Promise<acp.NewSessionResponse> {
    return this.sessionManager.newSession(params, this.getAuthDetails());
  }

  async loadSession(
    params: acp.LoadSessionRequest,
  ): Promise<acp.LoadSessionResponse> {
    return this.sessionManager.loadSession(params, this.getAuthDetails());
  }

  async cancel(params: acp.CancelNotification): Promise<void> {
    const session = this.sessionManager.getSession(params.sessionId);
    if (!session) {
      throw new acp.RequestError(
        -32602,
        `Session not found: ${params.sessionId}`,
      );
    }
    await session.cancelPendingPrompt();
  }

  async prompt(params: acp.PromptRequest): Promise<acp.PromptResponse> {
    const session = this.sessionManager.getSession(params.sessionId);
    if (!session) {
      throw new acp.RequestError(
        -32602,
        `Session not found: ${params.sessionId}`,
      );
    }
    return session.prompt(params);
  }

  async setSessionMode(
    params: acp.SetSessionModeRequest,
  ): Promise<acp.SetSessionModeResponse> {
    const session = this.sessionManager.getSession(params.sessionId);
    if (!session) {
      throw new acp.RequestError(
        -32602,
        `Session not found: ${params.sessionId}`,
      );
    }
    return session.setMode(params.modeId);
  }

  async unstable_setSessionModel(
    params: acp.SetSessionModelRequest,
  ): Promise<acp.SetSessionModelResponse> {
    const session = this.sessionManager.getSession(params.sessionId);
    if (!session) {
      throw new acp.RequestError(
        -32602,
        `Session not found: ${params.sessionId}`,
      );
    }
    return session.setModel(params.modelId);
  }
}
