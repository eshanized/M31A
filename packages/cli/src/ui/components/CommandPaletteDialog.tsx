/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useMemo, useState, useEffect } from 'react';
import { Box, Text } from 'ink';
import { theme } from '../semantic-colors.js';
import { useKeypress, type Key } from '../hooks/useKeypress.js';
import { fuzzyFilter } from '../utils/fuzzySearch.js';
import { type SlashCommand } from '../commands/types.js';

export interface CommandPaletteAction {
  id: string;
  label: string;
  description?: string;
  category?: string;
  shortcut?: string;
  suggested?: boolean;
  onSelect: () => void;
}

export interface CommandPaletteDialogProps {
  actions: readonly CommandPaletteAction[];
  onClose: () => void;
  terminalWidth?: number;
}

const MAX_VISIBLE_ITEMS = 12;

export function buildCommandPaletteActions(
  slashCommands: readonly SlashCommand[],
  onRunCommand: (commandName: string) => void,
  quickActions: readonly CommandPaletteAction[] = [],
): CommandPaletteAction[] {
  const result: CommandPaletteAction[] = [...quickActions];
  const seenIds = new Set(quickActions.map((a) => a.id));

  for (const cmd of slashCommands) {
    if (cmd.hidden) {
      continue;
    }
    const cmdId = `slash:${cmd.name}`;
    if (!seenIds.has(cmdId)) {
      seenIds.add(cmdId);
      result.push({
        id: cmdId,
        label: `/${cmd.name}`,
        description: cmd.description,
        category: cmd.suggestionGroup ?? 'Commands',
        onSelect: () => onRunCommand(cmd.name),
      });
    }

    if (cmd.subCommands) {
      for (const sub of cmd.subCommands) {
        if (sub.hidden) {
          continue;
        }
        const subId = `slash:${cmd.name}:${sub.name}`;
        if (seenIds.has(subId)) {
          continue;
        }
        seenIds.add(subId);
        result.push({
          id: subId,
          label: `/${cmd.name} ${sub.name}`,
          description: sub.description,
          category: sub.suggestionGroup ?? cmd.name,
          onSelect: () => onRunCommand(`${cmd.name} ${sub.name}`),
        });
      }
    }
  }

  return result;
}

function CommandPaletteDialogInner({
  actions,
  onClose,
  terminalWidth = 80,
}: CommandPaletteDialogProps): React.JSX.Element {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const orderedActions = useMemo(() => {
    if (!query.trim()) {
      const suggested = actions.filter((a) => a.suggested);
      const rest = actions.filter((a) => !a.suggested);
      return [...suggested, ...rest];
    }
    return fuzzyFilter(actions, query, (action) => [
      action.label,
      action.description ?? '',
      action.category ?? '',
    ]).map((result) => result.item);
  }, [actions, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const clampedIndex = Math.min(
    selectedIndex,
    Math.max(0, orderedActions.length - 1),
  );

  useKeypress(
    (key: Key) => {
      if (key.name === 'escape') {
        onClose();
        return true;
      }

      if (
        key.name === 'up' ||
        (key.ctrl && key.name === 'p') ||
        (key.ctrl && key.name === 'k')
      ) {
        setSelectedIndex((prev) =>
          orderedActions.length === 0
            ? 0
            : (prev - 1 + orderedActions.length) % orderedActions.length,
        );
        return true;
      }

      if (
        key.name === 'down' ||
        (key.ctrl && key.name === 'n') ||
        (key.ctrl && key.name === 'j')
      ) {
        setSelectedIndex((prev) =>
          orderedActions.length === 0 ? 0 : (prev + 1) % orderedActions.length,
        );
        return true;
      }

      if (key.name === 'return' || key.name === 'enter') {
        const chosen = orderedActions[clampedIndex];
        onClose();
        chosen?.onSelect();
        return true;
      }

      if (key.name === 'backspace' || key.name === 'delete') {
        setQuery((prev) => prev.slice(0, -1));
        return true;
      }

      if (key.ctrl && key.name === 'u') {
        setQuery('');
        return true;
      }

      if (
        key.sequence &&
        !key.ctrl &&
        !key.alt &&
        !key.cmd &&
        key.name !== 'tab' &&
        key.name !== 'left' &&
        key.name !== 'right' &&
        key.sequence >= ' '
      ) {
        setQuery((prev) => prev + key.sequence);
        return true;
      }

      return false;
    },
    { isActive: true },
  );

  const halfWindow = Math.floor(MAX_VISIBLE_ITEMS / 2);
  let windowStart = Math.max(0, clampedIndex - halfWindow);
  if (windowStart + MAX_VISIBLE_ITEMS > orderedActions.length) {
    windowStart = Math.max(0, orderedActions.length - MAX_VISIBLE_ITEMS);
  }
  const visibleActions = orderedActions.slice(
    windowStart,
    windowStart + MAX_VISIBLE_ITEMS,
  );
  const dialogWidth = Math.max(40, Math.min(terminalWidth - 4, 84));
  const hasSuggestedSection =
    !query.trim() && visibleActions.some((a) => a.suggested);
  const firstNonSuggestedVisibleIdx = visibleActions.findIndex(
    (a) => !a.suggested,
  );

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor={theme.ui.focus}
      width={dialogWidth}
      paddingX={1}
    >
      <Box justifyContent="space-between">
        <Text bold color={theme.text.accent}>
          Command Palette
        </Text>
        <Text color={theme.text.secondary}>
          ↑↓ select · Enter run · Esc close
        </Text>
      </Box>

      <Box marginTop={1} marginBottom={1}>
        <Text color={theme.text.accent}>{'> '}</Text>
        <Text color={query ? theme.text.primary : theme.text.secondary}>
          {query || 'Type to filter commands and actions...'}
        </Text>
      </Box>

      {orderedActions.length === 0 ? (
        <Text color={theme.text.secondary}>No matching commands found.</Text>
      ) : (
        <Box flexDirection="column">
          {visibleActions.map((action, offset) => {
            const index = windowStart + offset;
            const isSelected = index === clampedIndex;
            const showSuggestedHeader =
              hasSuggestedSection && offset === 0 && action.suggested;
            const showAllCommandsHeader =
              !query.trim() &&
              hasSuggestedSection &&
              offset === firstNonSuggestedVisibleIdx;

            return (
              <React.Fragment key={action.id}>
                {showSuggestedHeader && (
                  <Text bold color={theme.text.secondary}>
                    Suggested
                  </Text>
                )}
                {showAllCommandsHeader && (
                  <Box marginTop={offset > 0 ? 1 : 0}>
                    <Text bold color={theme.text.secondary}>
                      Commands
                    </Text>
                  </Box>
                )}
                <Box justifyContent="space-between">
                  <Box>
                    <Text
                      color={isSelected ? theme.ui.active : theme.text.primary}
                      bold={isSelected}
                    >
                      {isSelected ? '❯ ' : '  '}
                      {action.label}
                    </Text>
                    {action.description ? (
                      <Text color={theme.text.secondary}>
                        {' '}
                        — {action.description}
                      </Text>
                    ) : null}
                  </Box>
                  {action.shortcut ? (
                    <Text color={theme.text.secondary}>{action.shortcut}</Text>
                  ) : null}
                </Box>
              </React.Fragment>
            );
          })}
          {orderedActions.length > MAX_VISIBLE_ITEMS && (
            <Box marginTop={1}>
              <Text color={theme.text.secondary}>
                Showing {windowStart + 1}–{windowStart + visibleActions.length}{' '}
                of {orderedActions.length}
              </Text>
            </Box>
          )}
        </Box>
      )}
    </Box>
  );
}

export const CommandPaletteDialog = React.memo(CommandPaletteDialogInner);
