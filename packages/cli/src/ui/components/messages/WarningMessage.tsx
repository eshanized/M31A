/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Text } from 'ink';
import { theme } from '../../semantic-colors.js';
import { RenderInline } from '../../utils/InlineMarkdownRenderer.js';
import { MessageFrame } from './MessageFrame.js';

interface WarningMessageProps {
  text: string;
}

export const WarningMessage: React.FC<WarningMessageProps> = ({ text }) => {
  const prefix = '⚠ ';

  return (
    <MessageFrame
      prefix={prefix}
      prefixWidth={3}
      prefixColor={theme.status.warning}
      ariaLabel="Warning: "
      marginTop={1}
    >
      <Text wrap="wrap">
        <RenderInline text={text} defaultColor={theme.status.warning} />
      </Text>
    </MessageFrame>
  );
};
