/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box, Text } from 'ink';
import chalk from 'chalk';
import type { TextBuffer } from '../shared/text-buffer.js';
import { theme } from '../../semantic-colors.js';
import {
  cpSlice,
  cpLen,
  cpIndexToOffset,
  getCachedStringWidth,
} from '../../utils/textUtils.js';
import {
  parseInputForHighlighting,
  parseSegmentsFromTokens,
} from '../../utils/highlight.js';
import type { ScrollableItem } from './inputPromptUtils.js';

export interface InputPromptLineProps {
  item: ScrollableItem;
  buffer: TextBuffer;
  focus: boolean;
  showCursor: boolean;
  cursorVisualRowAbsolute: number;
  cursorVisualColAbsolute: number;
  inlineGhost: string;
  inputWidth: number;
}

/**
 * Renders either a visual input line (with syntax/token highlighting and cursor)
 * or a trailing multiline ghost completion line.
 */
export const InputPromptLine: React.FC<InputPromptLineProps> = ({
  item,
  buffer,
  focus,
  showCursor,
  cursorVisualRowAbsolute,
  cursorVisualColAbsolute,
  inlineGhost,
  inputWidth,
}) => {
  if (item.type === 'ghostLine') {
    const padding = Math.max(
      0,
      inputWidth - getCachedStringWidth(item.ghostLine),
    );
    return (
      <Box height={1}>
        <Text color={theme.text.secondary}>
          {item.ghostLine}
          {' '.repeat(padding)}
        </Text>
      </Box>
    );
  }

  const { lineText, absoluteVisualIdx } = item;
  const mapEntry = buffer.visualToLogicalMap[absoluteVisualIdx];
  if (!mapEntry) return <Text> </Text>;

  const isOnCursorLine = focus && absoluteVisualIdx === cursorVisualRowAbsolute;
  const renderedLine: React.ReactNode[] = [];
  const [logicalLineIdx] = mapEntry;
  const logicalLine = buffer.lines[logicalLineIdx] || '';
  const transformations = buffer.transformationsByLine[logicalLineIdx] ?? [];
  const tokens = parseInputForHighlighting(
    logicalLine,
    logicalLineIdx,
    transformations,
    ...(focus && buffer.cursor[0] === logicalLineIdx ? [buffer.cursor[1]] : []),
  );
  const visualStartCol = buffer.visualToTransformedMap[absoluteVisualIdx] ?? 0;
  const visualEndCol = visualStartCol + cpLen(lineText);
  const segments = parseSegmentsFromTokens(
    tokens,
    visualStartCol,
    visualEndCol,
  );
  let charCount = 0;
  segments.forEach((seg, segIdx) => {
    const segLen = cpLen(seg.text);
    let display = seg.text;
    if (isOnCursorLine) {
      const relCol = cursorVisualColAbsolute;
      const segStart = charCount;
      const segEnd = segStart + segLen;
      if (relCol >= segStart && relCol < segEnd) {
        const charToHighlight = cpSlice(
          display,
          relCol - segStart,
          relCol - segStart + 1,
        );
        const highlighted = showCursor
          ? chalk.inverse(charToHighlight)
          : charToHighlight;
        display =
          cpSlice(display, 0, relCol - segStart) +
          highlighted +
          cpSlice(display, relCol - segStart + 1);
      }
      charCount = segEnd;
    } else {
      charCount += segLen;
    }
    const color =
      seg.type === 'command' || seg.type === 'file' || seg.type === 'paste'
        ? theme.text.accent
        : theme.text.primary;
    renderedLine.push(
      <Text key={`token-${segIdx}`} color={color}>
        {display}
      </Text>,
    );
  });

  const currentLineGhost = isOnCursorLine ? inlineGhost : '';
  if (
    isOnCursorLine &&
    cursorVisualColAbsolute === cpLen(lineText) &&
    !currentLineGhost
  ) {
    renderedLine.push(
      <Text key={`cursor-end-${cursorVisualColAbsolute}`}>
        {showCursor ? chalk.inverse(' ') : ' '}
      </Text>,
    );
  }
  const showCursorBeforeGhost =
    focus &&
    isOnCursorLine &&
    cursorVisualColAbsolute === cpLen(lineText) &&
    currentLineGhost;
  return (
    <Box height={1}>
      <Text
        terminalCursorFocus={showCursor && isOnCursorLine}
        terminalCursorPosition={cpIndexToOffset(
          lineText,
          cursorVisualColAbsolute,
        )}
      >
        {renderedLine}
        {showCursorBeforeGhost && (showCursor ? chalk.inverse(' ') : ' ')}
        {currentLineGhost && (
          <Text color={theme.text.secondary}>{currentLineGhost}</Text>
        )}
      </Text>
    </Box>
  );
};
