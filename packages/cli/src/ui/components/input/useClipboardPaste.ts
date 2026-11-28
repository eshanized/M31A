/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as path from 'node:path';
import { useCallback } from 'react';
import clipboardy from 'clipboardy';
import { debugLogger, type Config } from '@m31a/core';
import type { TextBuffer } from '../shared/text-buffer.js';
import { escapeAtSymbols } from '../../hooks/atCommandProcessor.js';
import {
  clipboardHasImage,
  saveClipboardImage,
  cleanupOldClipboardImages,
} from '../../utils/clipboardUtils.js';
import { Command } from '../../key/keyMatchers.js';
import { formatCommand } from '../../key/keybindingUtils.js';
import {
  appEvents,
  AppEvent,
  TransientMessageType,
} from '../../../utils/events.js';
import type { MergedSettings } from '../../../config/settings.js';
import { isLargePaste } from './inputPromptUtils.js';

export interface UseClipboardPasteOptions {
  buffer: TextBuffer;
  config: Config;
  stdout: NodeJS.WriteStream;
  settings: MergedSettings;
  shortcutsHelpVisible: boolean;
  setShortcutsHelpVisible: (visible: boolean) => void;
}

/**
 * Handles clipboard pasting of both images (@path insertion) and text (with large-paste hints).
 */
export function useClipboardPaste({
  buffer,
  config,
  stdout,
  settings,
  shortcutsHelpVisible,
  setShortcutsHelpVisible,
}: UseClipboardPasteOptions): () => Promise<void> {
  return useCallback(async () => {
    if (shortcutsHelpVisible) {
      setShortcutsHelpVisible(false);
    }
    try {
      if (await clipboardHasImage()) {
        const imagePath = await saveClipboardImage(config.getTargetDir());
        if (imagePath) {
          // Clean up old images
          cleanupOldClipboardImages(config.getTargetDir()).catch(() => {
            // Ignore cleanup errors
          });

          // Get relative path from current directory
          const relativePath = path.relative(config.getTargetDir(), imagePath);

          // Insert @path reference at cursor position
          const insertText = `@${relativePath}`;
          const currentText = buffer.text;
          const offset = buffer.getOffset();

          // Add spaces around the path if needed
          let textToInsert = insertText;
          const charBefore = offset > 0 ? currentText[offset - 1] : '';
          const charAfter =
            offset < currentText.length ? currentText[offset] : '';

          if (charBefore && charBefore !== ' ' && charBefore !== '\n') {
            textToInsert = ' ' + textToInsert;
          }
          if (!charAfter || (charAfter !== ' ' && charAfter !== '\n')) {
            textToInsert = textToInsert + ' ';
          }

          // Insert at cursor position
          buffer.replaceRangeByOffset(offset, offset, textToInsert);
        }
      }

      if (settings.experimental?.useOSC52Paste) {
        stdout.write('\x1b]52;c;?\x07');
      } else {
        const textToInsert = await clipboardy.read();
        const escapedText = settings.ui?.escapePastedAtSymbols
          ? escapeAtSymbols(textToInsert)
          : textToInsert;
        buffer.insert(escapedText, { paste: true });

        if (isLargePaste(textToInsert)) {
          appEvents.emit(AppEvent.TransientMessage, {
            message: `Press ${formatCommand(Command.EXPAND_PASTE)} to expand pasted text`,
            type: TransientMessageType.Hint,
          });
        }
      }
    } catch (error) {
      debugLogger.error('Error handling paste:', error);
    }
  }, [
    buffer,
    config,
    stdout,
    settings,
    shortcutsHelpVisible,
    setShortcutsHelpVisible,
  ]);
}
