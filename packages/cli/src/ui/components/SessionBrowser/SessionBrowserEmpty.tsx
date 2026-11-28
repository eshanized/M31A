/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box, Text } from 'ink';
import { theme } from '../../semantic-colors.js';

/**
 * Empty state component displayed when no sessions are found.
 */
export const SessionBrowserEmpty = (): React.JSX.Element => (
  <Box flexDirection="column" paddingX={1}>
    <Text color={theme.text.secondary}>No auto-saved conversations found.</Text>
    <Text color={theme.text.secondary}>Press q to exit</Text>
  </Box>
);
