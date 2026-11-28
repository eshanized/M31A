/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Text } from 'ink';
import { theme } from '../semantic-colors.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useSettings } from '../contexts/SettingsContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { ContextSummaryDisplay } from './ContextSummaryDisplay.js';

export interface StatusDisplayProps {
  hideContextSummary: boolean;
}

const StatusDisplayInner: React.FC<StatusDisplayProps> = ({
  hideContextSummary,
}) => {
  const uiState = useUIState();
  const settings = useSettings();
  const config = useConfig();

  if (process.env['M31A_SYSTEM_MD']) {
    return <Text color={theme.status.error}>|⌐■_■|</Text>;
  }

  if (!settings.merged.ui.hideContextSummary && !hideContextSummary) {
    return (
      <ContextSummaryDisplay
        ideContext={uiState.ideContextState}
        m31aMdFileCount={uiState.m31aMdFileCount}
        contextFileNames={uiState.contextFileNames}
        mcpServers={config.getMcpClientManager?.()?.getMcpServers() ?? {}}
        blockedMcpServers={
          config.getMcpClientManager?.()?.getBlockedMcpServers() ?? []
        }
        skillCount={
          config.getSkillManager?.()?.getDisplayableSkills().length ?? 0
        }
        backgroundProcessCount={uiState.backgroundTaskCount}
      />
    );
  }

  return null;
};

export const StatusDisplay = React.memo(StatusDisplayInner);
