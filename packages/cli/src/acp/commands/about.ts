/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { IdeClient, getVersion } from '@m31a/core';
import type {
  Command,
  CommandContext,
  CommandExecutionResponse,
} from './types.js';
import process from 'node:process';

export class AboutCommand implements Command {
  readonly name = 'about';
  readonly description = 'Show version and environment info';

  async execute(
    context: CommandContext,
    _args: string[] = [],
  ): Promise<CommandExecutionResponse> {
    const osVersion = process.platform;
    let sandboxEnv = 'no sandbox';
    if (process.env['SANDBOX'] && process.env['SANDBOX'] !== 'sandbox-exec') {
      sandboxEnv = process.env['SANDBOX'];
    } else if (process.env['SANDBOX'] === 'sandbox-exec') {
      sandboxEnv = `sandbox-exec (${
        process.env['SEATBELT_PROFILE'] || 'unknown'
      })`;
    }
    const modelVersion = context.agentContext.config.getModel() || 'Unknown';
    const cliVersion = await getVersion();
    const selectedAuthType =
      context.settings.merged?.security?.auth?.selectedType ?? '';
    const ideClient = await getIdeClientName(context);

    const tier = context.agentContext.config.getUserTierName() || 'Unknown';

    const info = [
      `- Version: ${cliVersion}`,
      `- OS: ${osVersion}`,
      `- Sandbox: ${sandboxEnv}`,
      `- Model: ${modelVersion}`,
      `- Auth Type: ${selectedAuthType}`,
      `- IDE Client: ${ideClient}`,
      `- Tier: ${tier}`,
    ].join('\n');

    return {
      name: this.name,
      data: `M31A Info:\n${info}`,
    };
  }
}

async function getIdeClientName(context: CommandContext) {
  if (!context.agentContext.config.getIdeMode()) {
    return '';
  }
  const ideClient = await IdeClient.getInstance();
  return ideClient?.getDetectedIdeDisplayName() ?? '';
}
