/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  PromptStashService,
  Storage,
  type MessageActionReturn,
} from '@m31a/core';
import {
  CommandKind,
  type CommandContext,
  type SlashCommand,
} from './types.js';

function getStashService(): PromptStashService {
  return new PromptStashService(Storage.getGlobalM31aDir());
}

async function listStashAction(): Promise<MessageActionReturn> {
  const entries = await getStashService().list();
  if (entries.length === 0) {
    return {
      type: 'message',
      messageType: 'info',
      content: 'Prompt stash is empty. Use `/stash save <prompt>` to save one.',
    };
  }
  const lines = entries.map((entry, idx) => {
    const preview =
      entry.input.length > 72
        ? `${entry.input.slice(0, 69).replace(/\s+/g, ' ')}...`
        : entry.input.replace(/\s+/g, ' ');
    return `${idx + 1}. ${preview}`;
  });
  return {
    type: 'message',
    messageType: 'info',
    content: `Stashed prompts (${entries.length}):\n${lines.join('\n')}`,
  };
}

async function saveStashAction(
  _context: CommandContext,
  args: string,
): Promise<MessageActionReturn> {
  const text = args.trim();
  if (!text) {
    return {
      type: 'message',
      messageType: 'error',
      content: 'Usage: /stash save <prompt text>',
    };
  }
  await getStashService().push(text);
  return {
    type: 'message',
    messageType: 'info',
    content: `Saved prompt to stash (${text.length} chars). Run \`/stash pop\` to restore.`,
  };
}

async function popStashAction(
  context: CommandContext,
): Promise<MessageActionReturn> {
  const entry = await getStashService().pop();
  if (!entry) {
    return {
      type: 'message',
      messageType: 'info',
      content: 'Prompt stash is empty.',
    };
  }
  context.ui.setInputText?.(entry.input);
  return {
    type: 'message',
    messageType: 'info',
    content: `Restored latest stashed prompt:\n${entry.input}`,
  };
}

async function clearStashAction(): Promise<MessageActionReturn> {
  await getStashService().clear();
  return {
    type: 'message',
    messageType: 'info',
    content: 'Cleared all stashed prompts.',
  };
}

export const stashCommand: SlashCommand = {
  name: 'stash',
  description: 'Save, list, or restore stashed prompt drafts',
  kind: CommandKind.BUILT_IN,
  action: async (context, args) => {
    const trimmed = args.trim();
    if (!trimmed || trimmed === 'list') {
      return listStashAction();
    }
    if (trimmed === 'pop' || trimmed === 'apply' || trimmed === 'restore') {
      return popStashAction(context);
    }
    if (trimmed === 'clear') {
      return clearStashAction();
    }
    if (trimmed.startsWith('save ')) {
      return saveStashAction(context, trimmed.slice(5));
    }
    if (trimmed.startsWith('push ')) {
      return saveStashAction(context, trimmed.slice(5));
    }
    return saveStashAction(context, trimmed);
  },
  subCommands: [
    {
      name: 'list',
      description: 'List saved prompt drafts',
      kind: CommandKind.BUILT_IN,
      autoExecute: true,
      action: () => listStashAction(),
    },
    {
      name: 'save',
      description: 'Save a prompt draft to the stash',
      kind: CommandKind.BUILT_IN,
      action: (context, args) => saveStashAction(context, args),
    },
    {
      name: 'pop',
      description: 'Pop and restore the most recent stashed prompt',
      kind: CommandKind.BUILT_IN,
      autoExecute: true,
      action: (context) => popStashAction(context),
    },
    {
      name: 'clear',
      description: 'Delete all stashed prompts',
      kind: CommandKind.BUILT_IN,
      autoExecute: true,
      action: () => clearStashAction(),
    },
  ],
};
