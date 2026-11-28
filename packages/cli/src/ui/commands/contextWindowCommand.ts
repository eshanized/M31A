/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  DEFAULT_M31A_MODEL,
  formatTokenCountShort,
  tokenLimit,
  type MessageActionReturn,
} from '@m31a/core';
import { SettingScope } from '../../config/settings.js';
import { CommandKind, type SlashCommand } from './types.js';

function parseContextWindowArg(raw: string): number | 'auto' | null {
  const clean = raw.trim().toLowerCase();
  if (!clean) return null;
  if (clean === 'auto' || clean === 'default' || clean === 'reset') {
    return 'auto';
  }
  const mMatch = /^(\d+(?:\.\d+)?)m$/i.exec(clean);
  if (mMatch) {
    return Math.round(Number(mMatch[1]) * 1_048_576);
  }
  const kMatch = /^(\d+(?:\.\d+)?)k$/i.exec(clean);
  if (kMatch) {
    return Math.round(Number(kMatch[1]) * 1_024);
  }
  if (/^\d+$/.test(clean)) {
    return Number.parseInt(clean, 10);
  }
  return null;
}

export const contextWindowCommand: SlashCommand = {
  name: 'context-window',
  description:
    'View or set context window override (auto, 32k, 64k, 128k, 256k, 512k, 1m, or exact tokens)',
  kind: CommandKind.BUILT_IN,
  action: async (context, args): Promise<MessageActionReturn> => {
    const trimmed = args.trim();
    const config = context.services.agentContext?.config;
    const model = config?.getModel() ?? DEFAULT_M31A_MODEL;
    const currentOverride =
      config?.getContextWindowOverride?.() ??
      context.services.settings.merged.model?.contextWindow;

    if (!trimmed) {
      const effective = tokenLimit(model, currentOverride);
      return {
        type: 'message',
        messageType: 'info',
        content: `Effective context window for \`${model}\`: **${formatTokenCountShort(effective)}** (${effective.toLocaleString()} tokens, ${
          currentOverride ? 'custom override' : 'catalog default'
        }).\nUsage: \`/context-window <auto|32k|64k|128k|256k|512k|1m|tokens>\``,
      };
    }

    const parsed = parseContextWindowArg(trimmed);
    if (parsed === null) {
      return {
        type: 'message',
        messageType: 'error',
        content: `Invalid context window "${trimmed}". Use \`auto\`, shorthand like \`128k\` / \`1m\`, or an integer >= 1024.`,
      };
    }

    if (parsed === 'auto') {
      config?.setContextWindowOverride?.(undefined, true);
      context.services.settings.setValue(
        SettingScope.User,
        'model.contextWindow',
        undefined,
      );
      const effective = tokenLimit(model);
      return {
        type: 'message',
        messageType: 'info',
        content: `Context window reset to catalog default (**${formatTokenCountShort(effective)}**, ${effective.toLocaleString()} tokens).`,
      };
    }

    if (parsed < 1024) {
      return {
        type: 'message',
        messageType: 'error',
        content: `Context window must be at least 1024 tokens (got ${parsed}).`,
      };
    }

    config?.setContextWindowOverride?.(parsed, true);
    context.services.settings.setValue(
      SettingScope.User,
      'model.contextWindow',
      parsed,
    );
    return {
      type: 'message',
      messageType: 'info',
      content: `Context window override set to **${formatTokenCountShort(parsed)}** (${parsed.toLocaleString()} tokens).`,
    };
  },
};
