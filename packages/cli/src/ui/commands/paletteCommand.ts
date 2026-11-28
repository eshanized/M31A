/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  CommandKind,
  type OpenDialogActionReturn,
  type SlashCommand,
} from './types.js';

export const paletteCommand: SlashCommand = {
  name: 'palette',
  description: 'Open the fuzzy command palette (Alt+P / Ctrl+Shift+P)',
  kind: CommandKind.BUILT_IN,
  autoExecute: true,
  action: (_context, _args): OpenDialogActionReturn => ({
    type: 'dialog',
    dialog: 'commandPalette',
  }),
};
