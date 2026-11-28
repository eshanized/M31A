/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box, Text } from 'ink';
import { theme } from '../../semantic-colors.js';
import type { SessionBrowserState } from '../SessionBrowser.js';

/**
 * Header component showing session count and sort information.
 */
export const SessionListHeader = ({
  state,
}: {
  state: SessionBrowserState;
}): React.JSX.Element => (
  <Box flexDirection="row" justifyContent="space-between">
    <Text color={theme.text.accent}>
      Chat Sessions ({state.totalSessions} total
      {state.searchQuery ? `, filtered` : ''})
    </Text>
    <Text color={theme.text.secondary}>
      sorted by {state.sortOrder} {state.sortReverse ? 'asc' : 'desc'}
    </Text>
  </Box>
);
