/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState, useCallback } from 'react';
import { Box, Text, useIsScreenReaderEnabled } from 'ink';
import { theme } from '../semantic-colors.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useKeypress, type Key } from '../hooks/useKeypress.js';
import { Command } from '../key/keyMatchers.js';
import { useKeyMatchers } from '../hooks/useKeyMatchers.js';
import {
  useStatusColumnItems,
  type StatusColumnItem,
} from './statusColumns.js';

export interface FooterRowItem {
  key: string;
  header: string;
  element: React.ReactNode;
  flexGrow?: number;
  flexShrink?: number;
  isFocused?: boolean;
  alignItems?: 'flex-start' | 'center' | 'flex-end';
}

export const COLUMN_GAP = 3;

export const FooterRow: React.FC<{
  items: FooterRowItem[];
  showLabels: boolean;
}> = ({ items, showLabels }) => {
  const elements: React.ReactNode[] = [];

  items.forEach((item, idx) => {
    if (idx > 0) {
      elements.push(
        <Box
          key={`sep-${item.key}`}
          flexGrow={1}
          flexShrink={1}
          minWidth={showLabels ? COLUMN_GAP : 3}
          justifyContent="center"
          alignItems="center"
        >
          {!showLabels && <Text color={theme.ui.comment}> · </Text>}
        </Box>,
      );
    }

    elements.push(
      <Box
        key={item.key}
        flexDirection="column"
        flexGrow={item.flexGrow ?? 0}
        flexShrink={item.flexShrink ?? 1}
        alignItems={item.alignItems}
        backgroundColor={item.isFocused ? theme.background.focus : undefined}
        aria-state={item.isFocused ? { selected: true } : undefined}
      >
        {showLabels && (
          <Box height={1}>
            <Text
              bold={item.isFocused}
              color={item.isFocused ? theme.text.primary : theme.ui.comment}
            >
              {item.header}
            </Text>
          </Box>
        )}
        <Box height={1}>{item.element}</Box>
      </Box>,
    );
  });

  return (
    <Box flexDirection="row" flexWrap="nowrap" width="100%">
      {elements}
    </Box>
  );
};

interface FooterOverflowPopoverProps {
  droppedColumns: StatusColumnItem[];
  maxWidth: number;
}

export const FooterOverflowPopover: React.FC<FooterOverflowPopoverProps> = ({
  droppedColumns,
  maxWidth,
}) => {
  if (droppedColumns.length === 0) {
    return null;
  }

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor={theme.border.default}
      paddingX={1}
      width={Math.max(20, maxWidth)}
      aria-label={`Additional status items (${droppedColumns.length})`}
    >
      <Text bold color={theme.text.accent}>
        More status items
      </Text>
      {droppedColumns.map((col) => (
        <Box key={col.id} flexDirection="row" justifyContent="space-between">
          <Text color={theme.ui.comment}>
            {col.header ? `${col.header}: ` : `${col.id}: `}
          </Text>
          <Box flexShrink={1}>{col.element(Math.max(12, maxWidth - 16))}</Box>
        </Box>
      ))}
    </Box>
  );
};

export interface FooterProps {
  /** Optional focus state enabling keyboard toggle of overflow popover. */
  isFocused?: boolean;
  /** Controlled overflow popover visibility. */
  overflowOpen?: boolean;
  /** Optional callback when overflow popover toggles. */
  onOverflowOpenChange?: (open: boolean) => void;
}

const FooterInner: React.FC<FooterProps> = ({
  isFocused = false,
  overflowOpen,
  onOverflowOpenChange,
}) => {
  const uiState = useUIState();
  const keyMatchers = useKeyMatchers();
  const isScreenReaderEnabled = useIsScreenReaderEnabled();
  const [internalOverflowOpen, setInternalOverflowOpen] = useState(false);
  const isOverflowOpen = overflowOpen ?? internalOverflowOpen;

  const setOverflowOpen = useCallback(
    (next: boolean) => {
      if (overflowOpen === undefined) {
        setInternalOverflowOpen(next);
      }
      onOverflowOpenChange?.(next);
    },
    [overflowOpen, onOverflowOpenChange],
  );

  const terminalWidth = uiState.terminalWidth;
  const { columns: potentialColumns, showLabels } =
    useStatusColumnItems(terminalWidth);

  const columnsToRender: StatusColumnItem[] = [];
  const droppedColumns: StatusColumnItem[] = [];
  let currentUsedWidth = 2; // Initial padding

  for (const col of potentialColumns) {
    const gap = columnsToRender.length > 0 ? (showLabels ? COLUMN_GAP : 3) : 0;
    const budgetWidth = col.id === 'workspace' ? 20 : col.width;

    if (
      col.isHighPriority ||
      currentUsedWidth + gap + budgetWidth <= terminalWidth - 2
    ) {
      columnsToRender.push(col);
      currentUsedWidth += gap + budgetWidth;
    } else {
      droppedColumns.push(col);
    }
  }

  const droppedAny = droppedColumns.length > 0;

  useKeypress(
    (key: Key) => {
      if (!isFocused || !droppedAny) {
        return false;
      }
      if (keyMatchers[Command.RETURN](key) || key.name === 'space') {
        setOverflowOpen(!isOverflowOpen);
        return true;
      }
      if (keyMatchers[Command.ESCAPE](key) && isOverflowOpen) {
        setOverflowOpen(false);
        return true;
      }
      return false;
    },
    { isActive: isFocused && droppedAny },
  );

  const rowItems: FooterRowItem[] = columnsToRender.map((col, index) => {
    const isWorkspace = col.id === 'workspace';
    const isLast = index === columnsToRender.length - 1;

    const otherItemsWidth = columnsToRender
      .filter((c) => c.id !== 'workspace')
      .reduce((sum, c) => sum + c.width, 0);
    const numItems = columnsToRender.length + (droppedAny ? 1 : 0);
    const numGaps = numItems > 1 ? numItems - 1 : 0;
    const gapsWidth = numGaps * (showLabels ? COLUMN_GAP : 3);
    const ellipsisWidth = droppedAny ? 1 : 0;

    const availableForWorkspace = Math.max(
      20,
      terminalWidth - 2 - gapsWidth - otherItemsWidth - ellipsisWidth,
    );

    const estimatedWidth = isWorkspace ? availableForWorkspace : col.width;

    return {
      key: col.id,
      header: col.header,
      element: col.element(estimatedWidth),
      flexGrow: 0,
      flexShrink: isWorkspace ? 1 : 0,
      alignItems:
        isLast && !droppedAny && index > 0 ? 'flex-end' : 'flex-start',
    };
  });

  if (droppedAny) {
    const overflowLabel = `${droppedColumns.length} hidden status items: ${droppedColumns
      .map((c) => c.header || c.id)
      .join(', ')}`;
    rowItems.push({
      key: 'ellipsis',
      header: '',
      isFocused: isFocused && isOverflowOpen,
      element: (
        <Text
          color={isOverflowOpen ? theme.ui.active : theme.ui.comment}
          aria-label={overflowLabel}
          aria-state={{ expanded: isOverflowOpen }}
        >
          …
        </Text>
      ),
      flexGrow: 0,
      flexShrink: 0,
      alignItems: 'flex-end',
    });
  }

  return (
    <Box flexDirection="column" width={terminalWidth}>
      {droppedAny && (isOverflowOpen || isScreenReaderEnabled) && (
        <FooterOverflowPopover
          droppedColumns={droppedColumns}
          maxWidth={terminalWidth - 2}
        />
      )}
      <Box
        width={terminalWidth}
        paddingX={1}
        overflow="hidden"
        flexWrap="nowrap"
      >
        <FooterRow items={rowItems} showLabels={showLabels} />
      </Box>
    </Box>
  );
};

export const Footer = React.memo(FooterInner);
