/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  NIM_THINKING_EFFORT_OPTIONS,
  getNimCatalogEntry,
  type MessageActionReturn,
  type NimThinkingEffort,
} from '@m31a/core';
import { SettingScope } from '../../config/settings.js';
import {
  CommandKind,
  type CommandContext,
  type SlashCommand,
} from './types.js';

function applyEffort(
  context: CommandContext,
  effort: NimThinkingEffort,
): MessageActionReturn {
  const config = context.services.agentContext?.config;
  const activeModel = config?.getModel() ?? '';
  config?.setThinkingEffort?.(effort, true);
  context.services.settings.setValue(
    SettingScope.User,
    'model.thinkingEffort',
    effort,
  );
  const entry = getNimCatalogEntry(activeModel);
  const note = entry?.effortValues
    ? ` (catalog supported efforts for ${entry.name}: ${entry.effortValues.join(', ')})`
    : '';
  return {
    type: 'message',
    messageType: 'info',
    content: `Thinking effort set to \`${effort}\`${note}.`,
  };
}

export const thinkingCommand: SlashCommand = {
  name: 'thinking',
  description:
    'View or set thinking effort (default, off, minimal, low, medium, high)',
  kind: CommandKind.BUILT_IN,
  action: async (context, args): Promise<MessageActionReturn> => {
    const trimmed = args.trim().toLowerCase();
    const config = context.services.agentContext?.config;
    const current =
      config?.getThinkingEffort?.() ??
      context.services.settings.merged.model?.thinkingEffort ??
      'default';

    if (!trimmed) {
      return {
        type: 'message',
        messageType: 'info',
        content: `Current thinking effort: \`${current}\`.\nUsage: \`/thinking <${NIM_THINKING_EFFORT_OPTIONS.join('|')}>\``,
      };
    }

    const matched = NIM_THINKING_EFFORT_OPTIONS.find((opt) => opt === trimmed);
    if (!matched) {
      return {
        type: 'message',
        messageType: 'error',
        content: `Unknown thinking effort \`${trimmed}\`. Valid options: ${NIM_THINKING_EFFORT_OPTIONS.join(', ')}.`,
      };
    }
    return applyEffort(context, matched);
  },
  subCommands: NIM_THINKING_EFFORT_OPTIONS.map((effort) => ({
    name: effort,
    description: `Set thinking effort to ${effort}`,
    kind: CommandKind.BUILT_IN,
    autoExecute: true,
    action: (context) => applyEffort(context, effort),
  })),
};
