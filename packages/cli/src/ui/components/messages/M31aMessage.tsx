/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { MarkdownDisplay } from '../../utils/MarkdownDisplay.js';
import { theme } from '../../semantic-colors.js';
import { SCREEN_READER_MODEL_PREFIX } from '../../textConstants.js';
import { useUIState } from '../../contexts/UIStateContext.js';
import { MessageFrame } from './MessageFrame.js';

interface M31aMessageProps {
  text: string;
  isPending: boolean;
  availableTerminalHeight?: number;
  terminalWidth: number;
}

export const M31aMessage: React.FC<M31aMessageProps> = ({
  text,
  isPending,
  availableTerminalHeight,
  terminalWidth,
}) => {
  const { renderMarkdown } = useUIState();
  const prefix = '◆ ';
  const prefixWidth = prefix.length;

  return (
    <MessageFrame
      prefix={prefix}
      prefixWidth={prefixWidth}
      prefixColor={theme.text.accent}
      ariaLabel={SCREEN_READER_MODEL_PREFIX}
    >
      <MarkdownDisplay
        text={text}
        isPending={isPending}
        availableTerminalHeight={
          availableTerminalHeight === undefined
            ? undefined
            : Math.max(availableTerminalHeight - 1, 1)
        }
        terminalWidth={Math.max(terminalWidth - prefixWidth, 0)}
        renderMarkdown={renderMarkdown}
      />
    </MessageFrame>
  );
};
