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

interface InfoMessageProps {
  text: string;
  secondaryText?: string;
  source?: string;
  icon?: string;
  color?: string;
  marginBottom?: number;
}

export const InfoMessage: React.FC<InfoMessageProps> = ({
  text,
  secondaryText,
  source,
  icon,
  color,
  marginBottom,
}) => {
  color ??= theme.status.warning;
  const prefix = icon ?? 'ℹ ';
  const lines = text.split('\n');

  return (
    <MessageFrame
      prefix={prefix}
      prefixWidth={prefix.length}
      prefixColor={color}
      ariaLabel="Info: "
      marginTop={1}
      marginBottom={marginBottom ?? 0}
    >
      {lines.map((line, index) => (
        <Text wrap="wrap" key={index}>
          <RenderInline text={line} defaultColor={color} />
          {index === lines.length - 1 && secondaryText && (
            <Text color={theme.text.secondary}> {secondaryText}</Text>
          )}
          {index === lines.length - 1 && source && (
            <Text color={theme.text.secondary}> [{source}]</Text>
          )}
        </Text>
      ))}
    </MessageFrame>
  );
};
