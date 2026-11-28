/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useMemo, Fragment } from 'react';
import { Box, Text } from 'ink';
import type {
  HistoryItem,
  HistoryItemWithoutId,
  IndividualToolCallDisplay,
} from '../../types.js';
import { ToolCallStatus, mapCoreStatusToDisplayStatus } from '../../types.js';
import { ToolMessage } from './ToolMessage.js';
import { ShellToolMessage } from './ShellToolMessage.js';
import { TopicMessage, isTopicTool } from './TopicMessage.js';
import { SubagentGroupDisplay } from './SubagentGroupDisplay.js';
import { DenseToolMessage } from './DenseToolMessage.js';
import { theme } from '../../semantic-colors.js';
import { useConfig } from '../../contexts/ConfigContext.js';
import { isShellTool } from './ToolShared.js';
import {
  isVisibleInToolGroup,
  CoreToolCallStatus,
  Kind,
  EDIT_DISPLAY_NAME,
  GLOB_DISPLAY_NAME,
  WEB_SEARCH_DISPLAY_NAME,
  READ_FILE_DISPLAY_NAME,
  LS_DISPLAY_NAME,
  GREP_DISPLAY_NAME,
  WEB_FETCH_DISPLAY_NAME,
  WRITE_FILE_DISPLAY_NAME,
  READ_MANY_FILES_DISPLAY_NAME,
} from '@m31a/core';
import { buildToolVisibilityContextFromDisplay } from '../../utils/historyUtils.js';
import { useUIState } from '../../contexts/UIStateContext.js';
import { getToolGroupBorderAppearance } from '../../utils/borderStyles.js';
import { useSettings } from '../../contexts/SettingsContext.js';
import {
  TOOL_RESULT_STATIC_HEIGHT,
  TOOL_RESULT_STANDARD_RESERVED_LINE_COUNT,
} from '../../utils/toolLayoutUtils.js';
import { hasDensePayload } from '../../hooks/toolMapping.js';

export { hasDensePayload };

const COMPACT_OUTPUT_ALLOWLIST = new Set([
  EDIT_DISPLAY_NAME,
  GLOB_DISPLAY_NAME,
  WEB_SEARCH_DISPLAY_NAME,
  READ_FILE_DISPLAY_NAME,
  LS_DISPLAY_NAME,
  GREP_DISPLAY_NAME,
  WEB_FETCH_DISPLAY_NAME,
  WRITE_FILE_DISPLAY_NAME,
  READ_MANY_FILES_DISPLAY_NAME,
]);

const EXPLORE_READ_TOOLS = new Set([
  READ_FILE_DISPLAY_NAME,
  READ_MANY_FILES_DISPLAY_NAME,
  LS_DISPLAY_NAME,
]);

const EXPLORE_SEARCH_TOOLS = new Set([GLOB_DISPLAY_NAME, GREP_DISPLAY_NAME]);

type GroupedToolEntry =
  | { kind: 'tool'; tool: IndividualToolCallDisplay }
  | { kind: 'agents'; tools: IndividualToolCallDisplay[] }
  | { kind: 'explore'; tools: IndividualToolCallDisplay[] };

function isExplorableCandidate(tool: IndividualToolCallDisplay): boolean {
  const isReadOrSearch =
    EXPLORE_READ_TOOLS.has(tool.name) || EXPLORE_SEARCH_TOOLS.has(tool.name);
  if (!isReadOrSearch) {
    return false;
  }
  if (tool.hasDensePayload ?? hasDensePayload(tool)) {
    return false;
  }
  return (
    tool.status === CoreToolCallStatus.Success ||
    tool.status === CoreToolCallStatus.Error
  );
}

/**
 * Summarizes a collapsed sequence of read/search tool calls into a compact single line.
 */
export function formatExplorationSummary(
  tools: readonly IndividualToolCallDisplay[],
): string {
  let reads = 0;
  let searches = 0;
  for (const t of tools) {
    if (EXPLORE_SEARCH_TOOLS.has(t.name)) {
      searches++;
    } else {
      reads++;
    }
  }
  const parts: string[] = [];
  if (reads > 0) parts.push(`${reads} read${reads === 1 ? '' : 's'}`);
  if (searches > 0) {
    parts.push(`${searches} search${searches === 1 ? '' : 'es'}`);
  }
  return `Explored: ${parts.join(', ')} (${tools.length} calls)`;
}

// Helper to identify if a tool should use the compact view
export const isCompactTool = (
  tool: IndividualToolCallDisplay,
  isCompactModeEnabled: boolean,
): boolean => {
  const hasCompactOutputSupport = COMPACT_OUTPUT_ALLOWLIST.has(tool.name);
  const displayStatus = mapCoreStatusToDisplayStatus(tool.status);
  return (
    isCompactModeEnabled &&
    hasCompactOutputSupport &&
    displayStatus !== ToolCallStatus.Confirming
  );
};

interface ToolGroupMessageProps {
  item: HistoryItem | HistoryItemWithoutId;
  toolCalls: IndividualToolCallDisplay[];
  availableTerminalHeight?: number;
  terminalWidth: number;
  onShellInputSubmit?: (input: string) => void;
  borderTop?: boolean;
  borderBottom?: boolean;
  isExpandable?: boolean;
}

// Main component renders the border and maps the tools using ToolMessage
const TOOL_MESSAGE_HORIZONTAL_MARGIN = 4;

export const ToolGroupMessage: React.FC<ToolGroupMessageProps> = ({
  item,
  toolCalls: allToolCalls,
  availableTerminalHeight,
  terminalWidth,
  borderTop: borderTopOverride,
  borderBottom: borderBottomOverride,
  isExpandable,
}) => {
  const settings = useSettings();
  const isLowErrorVerbosity = settings.merged.ui?.errorVerbosity !== 'full';
  const isCompactModeEnabled = settings.merged.ui?.compactToolOutput === true;

  // Filter out tool calls that should be hidden (e.g. in-progress Ask User, or Plan Mode operations).
  const visibleToolCalls = useMemo(
    () =>
      allToolCalls.filter((t) =>
        // Use the unified visibility utility
        isVisibleInToolGroup(
          buildToolVisibilityContextFromDisplay(t),
          isLowErrorVerbosity ? 'low' : 'full',
        ),
      ),
    [allToolCalls, isLowErrorVerbosity],
  );

  const {
    activePtyId,
    embeddedShellFocused,
    backgroundTasks,
    pendingHistoryItems,
  } = useUIState();

  const config = useConfig();

  const { borderColor, borderDimColor } = useMemo(
    () =>
      getToolGroupBorderAppearance(
        item,
        activePtyId,
        embeddedShellFocused,
        pendingHistoryItems,
        backgroundTasks,
      ),
    [
      item,
      activePtyId,
      embeddedShellFocused,
      pendingHistoryItems,
      backgroundTasks,
    ],
  );

  const groupedTools = useMemo((): GroupedToolEntry[] => {
    const rawGroups: GroupedToolEntry[] = [];
    for (const tool of visibleToolCalls) {
      if (tool.kind === Kind.Agent) {
        const lastGroup = rawGroups[rawGroups.length - 1];
        if (lastGroup?.kind === 'agents') {
          lastGroup.tools.push(tool);
        } else {
          rawGroups.push({ kind: 'agents', tools: [tool] });
        }
      } else if (isCompactModeEnabled && isExplorableCandidate(tool)) {
        const lastGroup = rawGroups[rawGroups.length - 1];
        if (lastGroup?.kind === 'explore') {
          lastGroup.tools.push(tool);
        } else {
          rawGroups.push({ kind: 'explore', tools: [tool] });
        }
      } else {
        rawGroups.push({ kind: 'tool', tool });
      }
    }

    // Do not collapse a lone read/search call into an exploration summary.
    const normalized: GroupedToolEntry[] = [];
    for (const entry of rawGroups) {
      if (entry.kind === 'explore' && entry.tools.length === 1) {
        normalized.push({ kind: 'tool', tool: entry.tools[0] });
      } else {
        normalized.push(entry);
      }
    }
    return normalized;
  }, [visibleToolCalls, isCompactModeEnabled]);

  const staticHeight = useMemo(() => {
    let height = 0;

    for (let i = 0; i < groupedTools.length; i++) {
      const group = groupedTools[i];
      const isLast = i === groupedTools.length - 1;

      const prevGroup = i > 0 ? groupedTools[i - 1] : null;
      const prevIsCompact =
        prevGroup !== null &&
        (prevGroup.kind === 'explore' ||
          (prevGroup.kind === 'tool' &&
            isCompactTool(prevGroup.tool, isCompactModeEnabled)));

      const prevIsTopic =
        prevGroup !== null &&
        prevGroup.kind === 'tool' &&
        isTopicTool(prevGroup.tool.name);

      const nextGroup = !isLast ? groupedTools[i + 1] : null;
      const nextIsCompact =
        nextGroup !== null &&
        (nextGroup.kind === 'explore' ||
          (nextGroup.kind === 'tool' &&
            isCompactTool(nextGroup.tool, isCompactModeEnabled)));

      const nextIsTopicToolCall =
        nextGroup !== null &&
        nextGroup.kind === 'tool' &&
        isTopicTool(nextGroup.tool.name);

      const isAgentGroup = group.kind === 'agents';
      const isExploreGroup = group.kind === 'explore';
      const isCompact =
        isExploreGroup ||
        (group.kind === 'tool' &&
          isCompactTool(group.tool, isCompactModeEnabled));
      const isTopicToolCall =
        group.kind === 'tool' && isTopicTool(group.tool.name);

      // Align isFirst logic with rendering
      let isFirst = i === 0;
      if (!isFirst) {
        // Check if all previous tools were topics (matches rendering logic exactly)
        let allPreviousTopics = true;
        for (let j = 0; j < i; j++) {
          const prevGroupItem = groupedTools[j];
          if (
            prevGroupItem.kind !== 'tool' ||
            !isTopicTool(prevGroupItem.tool.name)
          ) {
            allPreviousTopics = false;
            break;
          }
        }
        isFirst = allPreviousTopics;
      }

      const isFirstProp = !!(isFirst
        ? (borderTopOverride ?? true)
        : prevIsCompact || prevIsTopic);

      const showClosingBorder =
        !isCompact &&
        !isTopicToolCall &&
        (nextIsCompact || nextIsTopicToolCall || isLast);

      if (isAgentGroup) {
        // Agent Group Spacing Breakdown:
        // 1. Top Boundary (0 or 1): Only present via borderTop if isFirstProp is true.
        // 2. Header Content (1): The "≡ Running Agent..." status text.
        // 3. Agent List (group.tools.length lines): One line per agent in the group.
        // 4. Closing Border (1): Added if transition logic (showClosingBorder) requires it.
        height +=
          (isFirstProp ? 1 : 0) +
          1 +
          group.tools.length +
          (showClosingBorder ? 1 : 0);
      } else if (isTopicToolCall) {
        // Topic Message Spacing Breakdown:
        // 1. Topic Content (1).
        // 2. Bottom Margin (1): Always present around TopicMessage for breathing room.
        // 3. Closing Border (1): Added if transition logic (showClosingBorder) requires it.
        height += 1 + 1 + (showClosingBorder ? 1 : 0);
      } else if (isCompact) {
        // Compact Tool / Explore summary: Always renders as a single dense line.
        height += 1;
      } else if (group.kind === 'tool') {
        // Standard Tool (ToolMessage / ShellToolMessage) Spacing Breakdown:
        // 1. TOOL_RESULT_STANDARD_RESERVED_LINE_COUNT (4) accounts for the top boundary,
        // internal separator, header padding, and the group closing border.
        // (Subtract 1 to isolate the group-level closing border.)
        // 2. Header Content (1): TOOL_RESULT_STATIC_HEIGHT (the tool name/status).
        // 3. Output File Message (1): (conditional) if outputFile is present.
        // 4. Group Closing Border (1): (conditional) if transition logic (showClosingBorder) requires it.
        height +=
          TOOL_RESULT_STANDARD_RESERVED_LINE_COUNT -
          1 +
          TOOL_RESULT_STATIC_HEIGHT +
          (group.tool.outputFile ? 1 : 0) +
          (showClosingBorder ? 1 : 0);
      }
    }
    return height;
  }, [groupedTools, isCompactModeEnabled, borderTopOverride]);

  let countToolCallsWithResults = 0;
  for (const tool of visibleToolCalls) {
    if (tool.kind !== Kind.Agent) {
      if (isCompactTool(tool, isCompactModeEnabled)) {
        if (tool.hasDensePayload ?? hasDensePayload(tool)) {
          countToolCallsWithResults++;
        }
      } else if (
        tool.resultDisplay !== undefined &&
        tool.resultDisplay !== ''
      ) {
        countToolCallsWithResults++;
      }
    }
  }

  const availableTerminalHeightPerToolMessage = availableTerminalHeight
    ? Math.max(
        Math.floor(
          (availableTerminalHeight - staticHeight) /
            Math.max(1, countToolCallsWithResults),
        ),
        1,
      )
    : undefined;

  const safeTerminalWidth = Math.max(0, Math.floor(terminalWidth || 0));
  const contentWidth = Math.max(
    0,
    safeTerminalWidth - TOOL_MESSAGE_HORIZONTAL_MARGIN,
  );

  // If all tools are filtered out (e.g., in-progress AskUser tools, low-verbosity
  // internal errors, plan-mode hidden write/edit), we should not emit standalone
  // border fragments. The only case where an empty group should render is the
  // explicit "closing slice" (tools: []) used to bridge static/pending sections,
  // and only if it's actually continuing an open box from above.
  const isExplicitClosingSlice = allToolCalls.length === 0;
  const shouldShowGroup =
    visibleToolCalls.length > 0 ||
    (isExplicitClosingSlice && borderBottomOverride === true);

  if (!shouldShowGroup) {
    return null;
  }

  const content = (
    <Box
      flexDirection="column"
      /*
      This width constraint is highly important and protects us from an Ink rendering bug.
      Since the ToolGroup can typically change rendering states frequently, it can cause
      Ink to render the border of the box incorrectly and span multiple lines and even
      cause tearing.
    */
      width={safeTerminalWidth}
      minWidth={0}
      paddingRight={TOOL_MESSAGE_HORIZONTAL_MARGIN}
      marginBottom={0}
    >
      {visibleToolCalls.length === 0 &&
        isExplicitClosingSlice &&
        borderBottomOverride === true && (
          <Box
            width={contentWidth}
            minWidth={0}
            borderLeft={true}
            borderRight={true}
            borderTop={false}
            borderBottom={true}
            borderColor={borderColor}
            borderDimColor={borderDimColor}
            borderStyle="round"
          />
        )}
      {groupedTools.map((group, index) => {
        let isFirst = index === 0;
        if (!isFirst) {
          // Check if all previous tools were topics
          let allPreviousWereTopics = true;
          for (let i = 0; i < index; i++) {
            const prevGroup = groupedTools[i];
            if (
              prevGroup.kind !== 'tool' ||
              !isTopicTool(prevGroup.tool.name)
            ) {
              allPreviousWereTopics = false;
              break;
            }
          }
          isFirst = allPreviousWereTopics;
        }

        const isLast = index === groupedTools.length - 1;

        const prevGroup = index > 0 ? groupedTools[index - 1] : null;
        const prevIsCompact =
          prevGroup !== null &&
          (prevGroup.kind === 'explore' ||
            (prevGroup.kind === 'tool' &&
              isCompactTool(prevGroup.tool, isCompactModeEnabled)));
        const prevIsTopic =
          prevGroup !== null &&
          prevGroup.kind === 'tool' &&
          isTopicTool(prevGroup.tool.name);

        const nextGroup = !isLast ? groupedTools[index + 1] : null;
        const nextIsCompact =
          nextGroup !== null &&
          (nextGroup.kind === 'explore' ||
            (nextGroup.kind === 'tool' &&
              isCompactTool(nextGroup.tool, isCompactModeEnabled)));
        const nextIsTopicToolCall =
          nextGroup !== null &&
          nextGroup.kind === 'tool' &&
          isTopicTool(nextGroup.tool.name);

        const isAgentGroup = group.kind === 'agents';
        const isExploreGroup = group.kind === 'explore';
        const isCompact =
          isExploreGroup ||
          (group.kind === 'tool' &&
            isCompactTool(group.tool, isCompactModeEnabled));
        const isTopicToolCall =
          group.kind === 'tool' && isTopicTool(group.tool.name);

        const isFirstProp = !!(isFirst
          ? (borderTopOverride ?? true)
          : prevIsCompact || prevIsTopic);

        const showClosingBorder =
          !isCompact &&
          !isTopicToolCall &&
          (nextIsCompact || nextIsTopicToolCall || isLast);

        if (group.kind === 'explore') {
          return (
            <Box
              key={group.tools[0].callId}
              flexDirection="row"
              width={contentWidth}
              minWidth={0}
            >
              <Text color={theme.text.accent}>◆ </Text>
              <Text color={theme.text.primary} wrap="truncate-end">
                {formatExplorationSummary(group.tools)}
              </Text>
            </Box>
          );
        }

        if (isAgentGroup) {
          return (
            <Box
              key={group.tools[0].callId}
              flexDirection="column"
              width={contentWidth}
              minWidth={0}
            >
              <SubagentGroupDisplay
                toolCalls={group.tools}
                availableTerminalHeight={availableTerminalHeight}
                terminalWidth={contentWidth}
                borderColor={borderColor}
                borderDimColor={borderDimColor}
                isFirst={isFirstProp}
                isExpandable={isExpandable}
              />
              {showClosingBorder && (
                <Box
                  width={contentWidth}
                  minWidth={0}
                  borderLeft={true}
                  borderRight={true}
                  borderTop={false}
                  borderBottom={isLast ? (borderBottomOverride ?? true) : true}
                  borderColor={borderColor}
                  borderDimColor={borderDimColor}
                  borderStyle="round"
                />
              )}
            </Box>
          );
        }

        const tool = group.tool;
        const isShellToolCall = isShellTool(tool.name);

        const commonProps = {
          ...tool,
          availableTerminalHeight: availableTerminalHeightPerToolMessage,
          terminalWidth: contentWidth,
          emphasis: 'medium' as const,
          isFirst: isCompact ? false : isFirstProp,
          borderColor,
          borderDimColor,
          isExpandable,
        };

        return (
          <Fragment key={tool.callId}>
            <Box
              flexDirection="column"
              minHeight={1}
              width={contentWidth}
              minWidth={0}
            >
              {isCompact ? (
                <DenseToolMessage {...commonProps} />
              ) : isTopicToolCall ? (
                <Box marginBottom={1}>
                  <TopicMessage {...commonProps} />
                </Box>
              ) : isShellToolCall ? (
                <ShellToolMessage {...commonProps} config={config} />
              ) : (
                <ToolMessage {...commonProps} />
              )}
              {!isCompact && tool.outputFile && (
                <Box
                  borderLeft={true}
                  borderRight={true}
                  borderTop={false}
                  borderBottom={false}
                  borderColor={borderColor}
                  borderDimColor={borderDimColor}
                  flexDirection="column"
                  borderStyle="round"
                  paddingLeft={1}
                  paddingRight={1}
                >
                  <Box>
                    <Text color={theme.text.primary}>
                      Output too long and was saved to: {tool.outputFile}
                    </Text>
                  </Box>
                </Box>
              )}
            </Box>
            {showClosingBorder && (
              <Box
                width={contentWidth}
                borderLeft={true}
                borderRight={true}
                borderTop={false}
                borderBottom={isLast ? (borderBottomOverride ?? true) : true}
                borderColor={borderColor}
                borderDimColor={borderDimColor}
                borderStyle="round"
              />
            )}
          </Fragment>
        );
      })}
    </Box>
  );

  return content;
};
