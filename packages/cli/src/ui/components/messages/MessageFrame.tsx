/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box, Text } from 'ink';

export interface MessageFrameProps {
  prefix: string;
  prefixWidth?: number;
  prefixColor: string;
  ariaLabel?: string;
  marginTop?: number;
  marginBottom?: number;
  children: React.ReactNode;
}

/**
 * Shared transcript message layout with an accessible prefix column and flex-grow body.
 */
export const MessageFrame: React.FC<MessageFrameProps> = ({
  prefix,
  prefixWidth = prefix.length,
  prefixColor,
  ariaLabel,
  marginTop,
  marginBottom,
  children,
}) => (
  <Box flexDirection="row" marginTop={marginTop} marginBottom={marginBottom}>
    <Box width={prefixWidth} flexShrink={0}>
      <Text color={prefixColor} aria-label={ariaLabel}>
        {prefix}
      </Text>
    </Box>
    <Box flexGrow={1} flexDirection="column">
      {children}
    </Box>
  </Box>
);
