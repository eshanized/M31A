/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Box, Text, useIsScreenReaderEnabled } from 'ink';
import { getDisplayString } from '@m31a/core';
import { theme } from '../semantic-colors.js';
import { HorizontalLine } from './shared/HorizontalLine.js';
import { useStatusColumnItems, hasNvidiaNimKey } from './statusColumns.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useInputState } from '../contexts/InputContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { TodoTray } from './messages/Todo.js';
import { StatusDisplay } from './StatusDisplay.js';
import { ContextUsageDisplay } from './ContextUsageDisplay.js';
import { isReducedMotionEnabled } from './StatusGlyph.js';
import { formatCompactTokenCount } from '../utils/formatters.js';

interface SidebarProps {
  width: number;
  compact?: boolean;
}

const COMPACT_MAX_ITEMS = 5;

/**
 * Right-hand workspace sidebar displaying M31A branding and vertical telemetry sections.
 */
const SidebarInner: React.FC<SidebarProps> = ({ width, compact = false }) => {
  const isScreenReaderEnabled = useIsScreenReaderEnabled();
  const uiState = useUIState();
  const { copyModeEnabled, shellModeActive } = useInputState();
  const config = useConfig();
  const innerWidth = Math.max(10, width - 4);
  const { columns, showLabels } = useStatusColumnItems(innerWidth);

  if (isScreenReaderEnabled) {
    return null;
  }

  const maxCompactItems =
    (uiState.terminalHeight ?? 24) < 18
      ? 3
      : width < 28
        ? 4
        : COMPACT_MAX_ITEMS;

  const visibleColumns = compact ? columns.slice(0, maxCompactItems) : columns;
  const visibleIds = new Set(visibleColumns.map((col) => col.id));

  const isNimConfigured = Boolean(config.getContentGeneratorConfig()?.authType);
  const nimKeyPresent = hasNvidiaNimKey(config);
  const reducedMotion = isReducedMotionEnabled();
  const model = uiState.currentModel || config.getModel();
  const sessionId = (
    uiState.sessionStats?.sessionId ||
    config.getSessionId() ||
    ''
  ).slice(0, 8);
  const promptTokenCount = uiState.sessionStats?.lastPromptTokenCount ?? 0;
  let totalTokens = 0;
  let thoughtTokens = 0;
  for (const m of Object.values(uiState.sessionStats?.metrics?.models ?? {})) {
    totalTokens += m.tokens.total;
    thoughtTokens += m.tokens.thoughts;
  }
  const formattedTokens = formatCompactTokenCount(totalTokens);
  const formattedThoughtTokens =
    thoughtTokens > 0 ? formatCompactTokenCount(thoughtTokens) : undefined;
  const sectionCount = uiState.history.length;

  return (
    <Box
      flexDirection="column"
      width={width}
      height={compact ? undefined : '100%'}
      flexShrink={0}
      flexGrow={0}
      borderStyle="round"
      borderColor={theme.border.default}
      paddingX={1}
      justifyContent="space-between"
      overflow="hidden"
    >
      <Box flexDirection="column" width="100%">
        <Box flexDirection="row" justifyContent="space-between" width="100%">
          <Text bold color={theme.text.accent}>
            ◆ M31A
          </Text>
          <Text
            color={
              !isNimConfigured
                ? theme.text.secondary
                : nimKeyPresent
                  ? theme.status.success
                  : theme.status.warning
            }
          >
            {!isNimConfigured
              ? 'SETUP'
              : nimKeyPresent
                ? 'NIM · key ✓'
                : 'NIM · no key'}
          </Text>
        </Box>

        <HorizontalLine dim />

        <Box flexDirection="column" width="100%">
          {visibleColumns.map((col, idx) => (
            <Box
              key={col.id}
              flexDirection={compact ? 'row' : 'column'}
              justifyContent={compact ? 'space-between' : 'flex-start'}
              marginTop={compact ? 0 : idx > 0 ? 1 : 0}
              width="100%"
            >
              {showLabels && col.header && (
                <Text color={theme.ui.comment} wrap="truncate-end">
                  {compact ? `${col.header.split(' ')[0]}:` : col.header}
                </Text>
              )}
              <Box
                flexShrink={1}
                marginLeft={compact && showLabels && col.header ? 1 : 0}
                overflow="hidden"
              >
                {col.element(innerWidth)}
              </Box>
            </Box>
          ))}

          {!visibleIds.has('model-name') && model && (
            <Box
              flexDirection={compact ? 'row' : 'column'}
              justifyContent={compact ? 'space-between' : 'flex-start'}
              marginTop={compact ? 0 : visibleColumns.length > 0 ? 1 : 0}
              width="100%"
            >
              {showLabels && (
                <Text color={theme.ui.comment} wrap="truncate-end">
                  {compact ? '/model:' : '/model'}
                </Text>
              )}
              <Text color={theme.text.primary} wrap="truncate-end">
                {getDisplayString(model)}
              </Text>
            </Box>
          )}

          {!visibleIds.has('context-used') && (
            <Box
              flexDirection={compact ? 'row' : 'column'}
              justifyContent={compact ? 'space-between' : 'flex-start'}
              marginTop={compact ? 0 : 1}
              width="100%"
            >
              {showLabels && (
                <Text color={theme.ui.comment} wrap="truncate-end">
                  {compact ? 'context:' : 'context'}
                </Text>
              )}
              <ContextUsageDisplay
                promptTokenCount={promptTokenCount}
                model={model}
                terminalWidth={innerWidth}
              />
            </Box>
          )}

          {compact && !visibleIds.has('token-count') && (
            <Box
              flexDirection="row"
              justifyContent="space-between"
              width="100%"
            >
              {showLabels && (
                <Text color={theme.ui.comment} wrap="truncate-end">
                  tokens:
                </Text>
              )}
              <Text color={theme.text.secondary} wrap="truncate-end">
                {formattedTokens} tok
              </Text>
            </Box>
          )}

          {compact && formattedThoughtTokens && (
            <Box
              flexDirection="row"
              justifyContent="space-between"
              width="100%"
            >
              {showLabels && (
                <Text color={theme.ui.comment} wrap="truncate-end">
                  think:
                </Text>
              )}
              <Text color={theme.text.secondary} wrap="truncate-end">
                {formattedThoughtTokens} tok
              </Text>
            </Box>
          )}
        </Box>

        {!compact && (
          <Box flexDirection="column" width="100%" marginTop={1}>
            <StatusDisplay hideContextSummary={false} />
            <TodoTray />
          </Box>
        )}
      </Box>

      {!compact && (
        <Box flexDirection="column" width="100%" marginTop={1}>
          <HorizontalLine dim />
          <Box flexDirection="row" flexWrap="wrap" columnGap={1}>
            {uiState.showApprovalModeIndicator && (
              <Text color={theme.text.accent}>
                [{uiState.showApprovalModeIndicator}]
              </Text>
            )}
            {shellModeActive && !visibleIds.has('shell-mode') && (
              <Text color={theme.status.warning}>[! shell]</Text>
            )}
            {copyModeEnabled && !visibleIds.has('copy-mode') && (
              <Text color={theme.status.warning}>[copy]</Text>
            )}
            {reducedMotion && !visibleIds.has('reduced-motion') && (
              <Text color={theme.text.secondary}>[reduced-motion]</Text>
            )}
          </Box>
          {(!visibleIds.has('session-id') && sessionId) ||
          !visibleIds.has('token-count') ? (
            <Box justifyContent="space-between" width="100%">
              {!visibleIds.has('session-id') && sessionId ? (
                <Text color={theme.text.secondary} wrap="truncate-end">
                  ses:{sessionId}
                </Text>
              ) : (
                <Box />
              )}
              {!visibleIds.has('token-count') && (
                <Text color={theme.text.secondary}>{formattedTokens} tok</Text>
              )}
            </Box>
          ) : null}
          {formattedThoughtTokens && (
            <Box justifyContent="space-between" width="100%">
              <Text color={theme.ui.comment}>think:</Text>
              <Text color={theme.text.secondary}>
                {formattedThoughtTokens} tok
              </Text>
            </Box>
          )}
          <Box justifyContent="space-between" width="100%">
            <Text color={theme.text.secondary}>turns: {sectionCount}</Text>
            <Text color={theme.text.secondary}>Alt+S hide</Text>
          </Box>
          <Text color={theme.ui.comment} wrap="truncate-end">
            /model · /settings · /sidebar
          </Text>
        </Box>
      )}
    </Box>
  );
};

export const Sidebar = React.memo(SidebarInner);
