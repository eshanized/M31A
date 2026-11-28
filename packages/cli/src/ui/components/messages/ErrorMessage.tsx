/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Text } from 'ink';
import { theme } from '../../semantic-colors.js';
import { MessageFrame } from './MessageFrame.js';

interface ErrorMessageProps {
  text: string;
}

export const ErrorMessage: React.FC<ErrorMessageProps> = ({ text }) => {
  const prefix = '✕ ';

  return (
    <MessageFrame
      prefix={prefix}
      prefixWidth={prefix.length}
      prefixColor={theme.status.error}
      ariaLabel="Error: "
      marginBottom={1}
    >
      <Text wrap="wrap" color={theme.status.error}>
        {text}
      </Text>
    </MessageFrame>
  );
};
