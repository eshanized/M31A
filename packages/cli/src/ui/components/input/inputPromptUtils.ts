/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { ApprovalMode } from '@m31a/core';
import { theme } from '../../semantic-colors.js';
import {
  type TextBuffer,
  getTransformUnderCursor,
  LARGE_PASTE_LINE_THRESHOLD,
  LARGE_PASTE_CHAR_THRESHOLD,
} from '../shared/text-buffer.js';
import {
  cpSlice,
  toCodePoints,
  getCachedStringWidth,
} from '../../utils/textUtils.js';
import {
  appEvents,
  AppEvent,
  TransientMessageType,
} from '../../../utils/events.js';

export const SCROLLBAR_GUTTER_WIDTH = 1;
export const DOUBLE_TAB_CLEAN_UI_TOGGLE_WINDOW_MS = 350;

/**
 * Returns if the terminal can be trusted to handle paste events atomically
 * rather than potentially sending multiple paste events separated by line
 * breaks which could trigger unintended command execution.
 */
export function isTerminalPasteTrusted(
  kittyProtocolSupported: boolean,
): boolean {
  // Ideally we could trust all VSCode family terminals as well but it appears
  // we cannot as Cursor users on windows reported being impacted by this
  // issue (https://github.com/m31a/M31A/issues/3763).
  return kittyProtocolSupported;
}

export type ScrollableItem =
  | { type: 'visualLine'; lineText: string; absoluteVisualIdx: number }
  | { type: 'ghostLine'; ghostLine: string; index: number };

// The input content, input container, and input suggestions list may have different widths
export const calculatePromptWidths = (mainContentWidth: number) => {
  const FRAME_PADDING_AND_BORDER = 4; // Border (2) + padding (2)
  const PROMPT_PREFIX_WIDTH = 2; // '> ' or '! '

  const FRAME_OVERHEAD = FRAME_PADDING_AND_BORDER + PROMPT_PREFIX_WIDTH;
  const suggestionsWidth = Math.max(20, mainContentWidth);

  return {
    inputWidth: Math.max(mainContentWidth - FRAME_OVERHEAD, 1),
    containerWidth: mainContentWidth,
    suggestionsWidth,
    frameOverhead: FRAME_OVERHEAD,
  } as const;
};

/**
 * Returns true if the given text exceeds the thresholds for being considered a "large paste".
 */
export function isLargePaste(text: string): boolean {
  const pasteLineCount = text.split('\n').length;
  return (
    pasteLineCount > LARGE_PASTE_LINE_THRESHOLD ||
    text.length > LARGE_PASTE_CHAR_THRESHOLD
  );
}

/**
 * Attempt to toggle expansion of a paste placeholder in the buffer.
 * Returns true if a toggle action was performed or hint was shown, false otherwise.
 */
export function tryTogglePasteExpansion(buffer: TextBuffer): boolean {
  if (!buffer.pastedContent || Object.keys(buffer.pastedContent).length === 0) {
    return false;
  }

  const [row, col] = buffer.cursor;

  // 1. Check if cursor is on or immediately after a collapsed placeholder
  const transform = getTransformUnderCursor(
    row,
    col,
    buffer.transformationsByLine,
    { includeEdge: true },
  );
  if (transform?.type === 'paste' && transform.id) {
    buffer.togglePasteExpansion(transform.id, row, col);
    return true;
  }

  // 2. Check if cursor is inside an expanded paste region — collapse it
  const expandedId = buffer.getExpandedPasteAtLine(row);
  if (expandedId) {
    buffer.togglePasteExpansion(expandedId, row, col);
    return true;
  }

  // 3. Placeholders exist but cursor isn't on one — show hint
  appEvents.emit(AppEvent.TransientMessage, {
    message: 'Move cursor within placeholder to expand',
    type: TransientMessageType.Hint,
  });
  return true;
}

export interface GhostTextLayout {
  inlineGhost: string;
  additionalLines: string[];
}

/**
 * Computes the inline ghost suggestion suffix and wrapped continuation lines.
 */
export function computeGhostTextLines({
  promptCompletionText,
  bufferText,
  bufferLines,
  cursor,
  inputWidth,
}: {
  promptCompletionText: string | undefined;
  bufferText: string;
  bufferLines: string[];
  cursor: [number, number];
  inputWidth: number;
}): GhostTextLayout {
  if (
    inputWidth <= 0 ||
    !promptCompletionText ||
    !bufferText ||
    !promptCompletionText.startsWith(bufferText)
  ) {
    return { inlineGhost: '', additionalLines: [] };
  }

  const ghostSuffix = promptCompletionText.slice(bufferText.length);
  if (!ghostSuffix) {
    return { inlineGhost: '', additionalLines: [] };
  }

  const currentLogicalLine = bufferLines[cursor[0]] || '';
  const cursorCol = cursor[1];

  const textBeforeCursor = cpSlice(currentLogicalLine, 0, cursorCol);
  const usedWidth = getCachedStringWidth(textBeforeCursor);
  const remainingWidth = Math.max(0, inputWidth - usedWidth);

  const ghostTextLinesRaw = ghostSuffix.split('\n');
  const firstLineRaw = ghostTextLinesRaw.shift() || '';

  let inlineGhost = '';
  let remainingFirstLine = '';

  if (getCachedStringWidth(firstLineRaw) <= remainingWidth) {
    inlineGhost = firstLineRaw;
  } else {
    const words = firstLineRaw.split(' ');
    let currentLine = '';
    let wordIdx = 0;
    for (const word of words) {
      const prospectiveLine = currentLine ? `${currentLine} ${word}` : word;
      if (getCachedStringWidth(prospectiveLine) > remainingWidth) {
        break;
      }
      currentLine = prospectiveLine;
      wordIdx++;
    }
    inlineGhost = currentLine;
    if (words.length > wordIdx) {
      remainingFirstLine = words.slice(wordIdx).join(' ');
    }
  }

  const linesToWrap = [];
  if (remainingFirstLine) {
    linesToWrap.push(remainingFirstLine);
  }
  linesToWrap.push(...ghostTextLinesRaw);
  const remainingGhostText = linesToWrap.join('\n');

  const additionalLines: string[] = [];
  if (remainingGhostText) {
    const textLines = remainingGhostText.split('\n');
    for (const textLine of textLines) {
      const words = textLine.split(' ');
      let currentLine = '';

      for (const word of words) {
        const prospectiveLine = currentLine ? `${currentLine} ${word}` : word;
        const prospectiveWidth = getCachedStringWidth(prospectiveLine);

        if (prospectiveWidth > inputWidth) {
          if (currentLine) {
            additionalLines.push(currentLine);
          }

          let wordToProcess = word;
          while (getCachedStringWidth(wordToProcess) > inputWidth) {
            let part = '';
            const wordCP = toCodePoints(wordToProcess);
            let partWidth = 0;
            let splitIndex = 0;
            for (let i = 0; i < wordCP.length; i++) {
              const char = wordCP[i];
              const charWidth = getCachedStringWidth(char);
              if (partWidth + charWidth > inputWidth) {
                break;
              }
              part += char;
              partWidth += charWidth;
              splitIndex = i + 1;
            }
            if (splitIndex === 0) {
              part = wordCP[0];
              splitIndex = 1;
            }
            additionalLines.push(part);
            wordToProcess = cpSlice(wordToProcess, splitIndex);
          }
          currentLine = wordToProcess;
        } else {
          currentLine = prospectiveLine;
        }
      }
      if (currentLine) {
        additionalLines.push(currentLine);
      }
    }
  }

  return { inlineGhost, additionalLines };
}

export interface InputPromptStatusPresentation {
  statusColor: string | undefined;
  statusText: string;
  showYoloStyling: boolean;
}

/**
 * Maps shell mode and approval mode to the uniform prompt badge styling.
 */
export function getInputPromptStatusPresentation(
  shellModeActive: boolean,
  approvalMode: ApprovalMode,
): InputPromptStatusPresentation {
  const showAutoAcceptStyling =
    !shellModeActive && approvalMode === ApprovalMode.AUTO_EDIT;
  const showYoloStyling =
    !shellModeActive && approvalMode === ApprovalMode.YOLO;
  const showPlanStyling =
    !shellModeActive && approvalMode === ApprovalMode.PLAN;

  if (shellModeActive) {
    return {
      statusColor: theme.ui.symbol,
      statusText: 'Shell mode',
      showYoloStyling,
    };
  }
  if (showYoloStyling) {
    return {
      statusColor: theme.status.error,
      statusText: 'YOLO mode',
      showYoloStyling,
    };
  }
  if (showPlanStyling) {
    return {
      statusColor: theme.status.success,
      statusText: 'Plan mode',
      showYoloStyling,
    };
  }
  if (showAutoAcceptStyling) {
    return {
      statusColor: theme.status.warning,
      statusText: 'Accepting edits',
      showYoloStyling,
    };
  }
  return {
    statusColor: undefined,
    statusText: '',
    showYoloStyling,
  };
}
