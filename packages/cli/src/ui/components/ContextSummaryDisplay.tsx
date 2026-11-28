/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box, Text } from 'ink';
import { theme } from '../semantic-colors.js';
import { type IdeContext, type MCPServerConfig } from '@m31a/core';
import { Command } from '../key/keyMatchers.js';
import { formatCommand } from '../key/keybindingUtils.js';

interface ContextSummaryDisplayProps {
  m31aMdFileCount: number;
  contextFileNames: string[];
  mcpServers?: Record<string, MCPServerConfig>;
  blockedMcpServers?: Array<{ name: string; extensionName: string }>;
  ideContext?: IdeContext;
  skillCount: number;
  backgroundProcessCount?: number;
}

export const ContextSummaryDisplay: React.FC<ContextSummaryDisplayProps> = ({
  m31aMdFileCount = 0,
  contextFileNames = [],
  mcpServers,
  blockedMcpServers,
  ideContext,
  skillCount = 0,
  backgroundProcessCount = 0,
}) => {
  const mcpServerCount = Object.keys(mcpServers || {}).length;
  const blockedMcpServerCount = blockedMcpServers?.length || 0;
  const openFileCount = ideContext?.workspaceState?.openFiles?.length ?? 0;

  if (
    m31aMdFileCount === 0 &&
    mcpServerCount === 0 &&
    blockedMcpServerCount === 0 &&
    openFileCount === 0 &&
    skillCount === 0 &&
    backgroundProcessCount === 0
  ) {
    return null;
  }

  const openFilesText = (() => {
    if (openFileCount === 0) {
      return '';
    }
    return `${openFileCount} open file${
      openFileCount > 1 ? 's' : ''
    } (${formatCommand(Command.SHOW_IDE_CONTEXT_DETAIL)} to view)`;
  })();

  const m31aMdText = (() => {
    if (!m31aMdFileCount || m31aMdFileCount <= 0) {
      return '';
    }
    const safeNames = contextFileNames ?? [];
    const allNamesTheSame = safeNames.length > 0 && new Set(safeNames).size < 2;
    const name = allNamesTheSame ? safeNames[0] : 'context';
    return `${m31aMdFileCount} ${name} file${m31aMdFileCount > 1 ? 's' : ''}`;
  })();

  const mcpText = (() => {
    if (mcpServerCount === 0 && blockedMcpServerCount === 0) {
      return '';
    }

    const parts = [];
    if (mcpServerCount > 0) {
      parts.push(
        `${mcpServerCount} MCP server${mcpServerCount > 1 ? 's' : ''}`,
      );
    }

    if (blockedMcpServerCount > 0) {
      let blockedText = `${blockedMcpServerCount} Blocked`;
      if (mcpServerCount === 0) {
        blockedText += ` MCP server${blockedMcpServerCount > 1 ? 's' : ''}`;
      }
      parts.push(blockedText);
    }
    return parts.join(', ');
  })();

  const skillText = (() => {
    if (skillCount === 0) {
      return '';
    }
    return `${skillCount} skill${skillCount > 1 ? 's' : ''}`;
  })();

  const backgroundText = (() => {
    if (backgroundProcessCount === 0) {
      return '';
    }
    return `${backgroundProcessCount} Background process${
      backgroundProcessCount > 1 ? 'es' : ''
    }`;
  })();

  const summaryParts = [
    openFilesText,
    m31aMdText,
    mcpText,
    skillText,
    backgroundText,
  ].filter(Boolean);

  return (
    <Box paddingX={1} flexDirection="row" flexWrap="wrap">
      {summaryParts.map((part, index) => (
        <Box key={index} flexDirection="row">
          {index > 0 && <Text color={theme.text.secondary}>{' · '}</Text>}
          <Text color={theme.text.secondary}>{part}</Text>
        </Box>
      ))}
    </Box>
  );
};
