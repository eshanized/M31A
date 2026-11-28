/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useMemo } from 'react';
import { Box, Text } from 'ink';
import type { ThoughtSummary } from '@m31a/core';
import { theme } from '../../semantic-colors.js';
import { normalizeEscapedNewlines } from '../../utils/textUtils.js';

interface ThinkingMessageProps {
  thought: ThoughtSummary;
  terminalWidth: number;
  isFirstThinking?: boolean;
  tokenCount?: number;
  compact?: boolean;
}

const THINKING_LEFT_PADDING = 1;

function formatThinkingTokenLabel(tokenCount?: number): string | undefined {
  if (tokenCount === undefined || tokenCount <= 0) {
    return undefined;
  }
  return `${tokenCount.toLocaleString()} ${tokenCount === 1 ? 'token' : 'tokens'}`;
}

function normalizeThoughtLines(thought: ThoughtSummary): string[] {
  const subject = normalizeEscapedNewlines(thought.subject).trim();
  const description = normalizeEscapedNewlines(thought.description).trim();

  const isNoise = (text: string) => {
    const trimmed = text.trim();
    return !trimmed || /^\.+$/.test(trimmed);
  };

  const lines: string[] = [];

  if (subject && !isNoise(subject)) {
    lines.push(subject);
  }

  if (description) {
    const descriptionLines = description
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => !isNoise(line));
    lines.push(...descriptionLines);
  }

  return lines;
}

/**
 * Renders a model's thought as a distinct bubble.
 * Leverages Ink layout for wrapping and borders.
 */
export const ThinkingMessage: React.FC<ThinkingMessageProps> = ({
  thought,
  terminalWidth,
  isFirstThinking,
  tokenCount,
  compact = false,
}) => {
  const fullLines = useMemo(() => normalizeThoughtLines(thought), [thought]);
  const tokenLabel = formatThinkingTokenLabel(tokenCount);

  if (compact) {
    if (!isFirstThinking || (fullLines.length === 0 && !tokenLabel)) {
      return null;
    }

    const normalizedSubject = normalizeEscapedNewlines(thought.subject).trim();
    const hasMeaningfulSubject =
      Boolean(normalizedSubject) &&
      !/^\.+$/.test(normalizedSubject) &&
      !/^thinking\.{0,3}$/i.test(normalizedSubject);

    return (
      <Box width={terminalWidth} flexDirection="row">
        <Text color={theme.text.secondary} italic wrap="truncate-end">
          {' ◇ '}
          {hasMeaningfulSubject ? `Thinking: ${normalizedSubject}` : 'Thinking'}
          {tokenLabel ? ` (${tokenLabel})` : '...'}
        </Text>
      </Box>
    );
  }

  if (fullLines.length === 0) {
    return null;
  }

  return (
    <Box width={terminalWidth} flexDirection="column">
      {isFirstThinking &&
        (tokenLabel ? (
          <Text color={theme.text.primary} italic>
            {' '}
            Thinking... <Text color={theme.text.secondary}>({tokenLabel})</Text>
          </Text>
        ) : (
          <Text color={theme.text.primary} italic>
            {' '}
            Thinking...{' '}
          </Text>
        ))}

      <Box
        marginLeft={THINKING_LEFT_PADDING}
        paddingLeft={1}
        borderStyle="single"
        borderLeft={true}
        borderRight={false}
        borderTop={false}
        borderBottom={false}
        borderColor={theme.text.secondary}
        flexDirection="column"
      >
        <Text> </Text>
        {fullLines.length > 0 && (
          <Text color={theme.text.primary} bold italic>
            {fullLines[0]}
          </Text>
        )}
        {fullLines.slice(1).map((line, index) => (
          <Text key={`body-line-${index}`} color={theme.text.secondary} italic>
            {line}
          </Text>
        ))}
      </Box>
    </Box>
  );
};
