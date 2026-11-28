/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box, Text } from 'ink';
import { theme } from '../../semantic-colors.js';
import type { SessionBrowserState } from '../SessionBrowser.js';

const Kbd = ({ name, shortcut }: { name: string; shortcut: string }) => (
  <>
    {name}: <Text bold>{shortcut}</Text>
  </>
);

/**
 * Navigation help component showing keyboard shortcuts.
 */
export const NavigationHelpDisplay = (): React.JSX.Element => (
  <Box flexDirection="column">
    <Text color={theme.text.secondary}>
      <Kbd name="Navigate" shortcut="↑/↓" />
      {'   '}
      <Kbd name="Resume" shortcut="Enter" />
      {'   '}
      <Kbd name="Search" shortcut="/" />
      {'   '}
      <Kbd name="Delete" shortcut="x" />
      {'   '}
      <Kbd name="Quit" shortcut="q" />
    </Text>
    <Text color={theme.text.secondary}>
      <Kbd name="Sort" shortcut="s" />
      {'         '}
      <Kbd name="Reverse" shortcut="r" />
      {'      '}
      <Kbd name="First/Last" shortcut="g/G" />
    </Text>
  </Box>
);

/**
 * Search input display component.
 */
export const SearchModeDisplay = ({
  state,
}: {
  state: SessionBrowserState;
}): React.JSX.Element => (
  <Box marginTop={1}>
    <Text color={theme.text.secondary}>Search: </Text>
    <Text color={theme.text.accent}>{state.searchQuery}</Text>
    <Text color={theme.text.secondary}> (Esc to cancel)</Text>
  </Box>
);

/**
 * No results display component for empty search results.
 */
export const NoResultsDisplay = ({
  state,
}: {
  state: SessionBrowserState;
}): React.JSX.Element => (
  <Box marginTop={1}>
    <Text color={theme.ui.comment}>
      No sessions found matching &apos;{state.searchQuery}&apos;.
    </Text>
  </Box>
);
