/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useMemo } from 'react';
import { Box, Text } from 'ink';
import { ApprovalMode, shortenPath, tildeifyPath } from '@m31a/core';
import { useSettings } from '../contexts/SettingsContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useUIActions } from '../contexts/UIActionsContext.js';
import { useInputState } from '../contexts/InputContext.js';
import { useBanner } from '../hooks/useBanner.js';
import { useKeypress, type Key } from '../hooks/useKeypress.js';
import { StreamingState } from '../types.js';
import { theme } from '../semantic-colors.js';
import { ThemedGradient } from './ThemedGradient.js';
import { CliSpinner } from './CliSpinner.js';
import { Banner } from './Banner.js';
import { getProviderSummary } from './UserIdentity.js';
import { getMascotIcon, longAsciiLogoCompactText } from './AsciiArt.js';
import { getAsciiArtWidth } from '../utils/textUtils.js';
import { isCompactWidth } from '../utils/isNarrowWidth.js';
import { formatCommand } from '../key/keybindingUtils.js';
import { Command } from '../key/keyBindings.js';

export interface StarterAction {
  key: '1' | '2' | '3' | '4';
  label: string;
  description: string;
  prompt: string;
}

export const STARTER_ACTIONS: readonly StarterAction[] = [
  {
    key: '1',
    label: 'Explore codebase',
    description: 'Summarize architecture and key entry points',
    prompt:
      'Summarize the codebase architecture, main entry points, and how the key modules fit together.',
  },
  {
    key: '2',
    label: 'Inspect git changes',
    description: 'Review working tree status and recent commits',
    prompt:
      'Inspect git status and recent commits in this repository, and summarize what has changed.',
  },
  {
    key: '3',
    label: 'Run project checks',
    description: 'Verify build, lint, typecheck, and test health',
    prompt:
      'Check project health by inspecting test, lint, typecheck, and build configuration/status.',
  },
  {
    key: '4',
    label: 'Find & fix a bug',
    description: 'Trace an error or failing test to its root cause',
    prompt:
      'Help me diagnose an error or failing test in this repository and propose a clean fix.',
  },
];

interface StartScreenProps {
  version: string;
  showDetails?: boolean;
  centered?: boolean;
}

interface ModeDescriptor {
  mode: ApprovalMode;
  label: string;
  activeColor: string;
}

export const StartScreen: React.FC<StartScreenProps> = ({
  version,
  showDetails = true,
  centered = false,
}) => {
  const settings = useSettings();
  const config = useConfig();
  const uiState = useUIState();
  const uiActions = useUIActions();
  const inputState = useInputState();

  const { bannerText } = useBanner(uiState.bannerData);

  const contentWidth = uiState.mainAreaWidth || uiState.terminalWidth || 80;
  const cardWidth = Math.max(24, Math.min(contentWidth, 88));
  const isNarrow = isCompactWidth(cardWidth);

  const approvalMode =
    uiState.showApprovalModeIndicator ??
    config.getApprovalMode?.() ??
    ApprovalMode.DEFAULT;

  const isStarterShortcutActive =
    showDetails &&
    !uiState.dialogsVisible &&
    !inputState.copyModeEnabled &&
    uiState.streamingState !== StreamingState.Responding &&
    (uiState.isInputActive ?? true);

  useKeypress(
    (key: Key) => {
      if (!key.alt || key.ctrl || key.cmd) {
        return;
      }
      const keyChar = key.name || key.sequence;
      const action = STARTER_ACTIONS.find((item) => item.key === keyChar);
      if (action) {
        void uiActions.handleFinalSubmit(action.prompt);
      }
    },
    { isActive: isStarterShortcutActive },
  );

  const showBannerHero = !(
    settings.merged.ui.hideBanner || config.getScreenReader()
  );

  const mascotIcon = getMascotIcon();
  const compactWordmark = useMemo(() => {
    const wordmark = longAsciiLogoCompactText.trim();
    const minHeroWidth = getAsciiArtWidth(wordmark) + 12;
    return contentWidth >= minHeroWidth ? wordmark : '';
  }, [contentWidth]);

  const modeDescriptors = useMemo<ModeDescriptor[]>(() => {
    const items: ModeDescriptor[] = [
      {
        mode: ApprovalMode.DEFAULT,
        label: 'Act',
        activeColor: theme.text.accent,
      },
      {
        mode: ApprovalMode.AUTO_EDIT,
        label: 'Auto-Edit',
        activeColor: theme.status.warning,
      },
    ];
    if (uiState.allowPlanMode !== false || approvalMode === ApprovalMode.PLAN) {
      items.push({
        mode: ApprovalMode.PLAN,
        label: 'Plan',
        activeColor: theme.status.success,
      });
    }
    if (approvalMode === ApprovalMode.YOLO) {
      items.push({
        mode: ApprovalMode.YOLO,
        label: '⚡ YOLO',
        activeColor: theme.status.error,
      });
    }
    return items;
  }, [uiState.allowPlanMode, approvalMode]);

  const targetDir = config.getTargetDir?.() || '';
  const dirDisplay = targetDir
    ? shortenPath(tildeifyPath(targetDir), isNarrow ? 28 : 42)
    : 'Current directory';

  const activeModel =
    uiState.currentModel || config.getModel?.() || 'Default model';
  const thinkingEffort =
    config.getThinkingEffort?.() ?? settings.merged.model?.thinkingEffort;
  const providerSummary = getProviderSummary(config);
  const sandboxEnabled = config.getSandboxEnabled?.() ?? false;

  const mcpServers = config.getMcpClientManager?.()?.getMcpServers() ?? {};
  const mcpCount = Object.keys(mcpServers).length;
  const skillsCount =
    config.getSkillManager?.()?.getDisplayableSkills().length ?? 0;
  const m31aMdCount =
    uiState.m31aMdFileCount || uiState.contextFileNames?.length || 0;

  const contextParts: string[] = [];
  if (m31aMdCount > 0) {
    contextParts.push(`${m31aMdCount} M31A.md`);
  }
  if (mcpCount > 0) {
    contextParts.push(`${mcpCount} MCP`);
  }
  if (skillsCount > 0) {
    contextParts.push(`${skillsCount} skills`);
  }
  const contextDisplay =
    contextParts.length > 0 ? contextParts.join(' · ') : 'No context files';

  const cycleHint = formatCommand(Command.CYCLE_APPROVAL_MODE);

  return (
    <Box
      flexDirection="column"
      alignItems="center"
      justifyContent={centered ? 'center' : 'flex-start'}
      flexGrow={centered ? 1 : undefined}
      width="100%"
      paddingY={1}
    >
      {showBannerHero && (
        <Box flexDirection="column" alignItems="center" marginBottom={1}>
          <Box flexDirection="row" alignItems="center">
            <ThemedGradient>
              <Text>{mascotIcon}</Text>
            </ThemedGradient>
            {compactWordmark ? (
              <Box marginLeft={2}>
                <ThemedGradient>
                  <Text>{compactWordmark}</Text>
                </ThemedGradient>
              </Box>
            ) : null}
          </Box>

          <Box marginTop={1} flexDirection="row" alignItems="center">
            {uiState.updateInfo?.isUpdating ? (
              <>
                <CliSpinner />
                <Text color={theme.text.secondary}> Updating M31A...</Text>
              </>
            ) : (
              <Text>
                <Text bold color={theme.text.accent}>
                  ◆ M31A v{version}
                </Text>
                {!isNarrow && (
                  <Text color={theme.text.secondary}>
                    {'  ·  What can I help you build today?'}
                  </Text>
                )}
              </Text>
            )}
          </Box>
          {isNarrow && !uiState.updateInfo?.isUpdating && (
            <Text color={theme.text.secondary}>
              What can I help you build today?
            </Text>
          )}
        </Box>
      )}

      {/* Plan / Act Mode Toggle Bar */}
      <Box
        width={cardWidth}
        flexDirection={isNarrow ? 'column' : 'row'}
        justifyContent="space-between"
        alignItems={isNarrow ? 'flex-start' : 'center'}
        paddingX={1}
        marginBottom={showDetails ? 1 : 0}
      >
        <Box flexDirection="row" flexWrap="wrap">
          {modeDescriptors.map((item, idx) => {
            const isActive = item.mode === approvalMode;
            return (
              <Box
                key={item.mode}
                marginRight={idx < modeDescriptors.length - 1 ? 2 : 0}
              >
                <Text
                  bold={isActive}
                  color={isActive ? item.activeColor : theme.text.secondary}
                >
                  {isActive ? `[● ${item.label}]` : `[  ${item.label} ]`}
                </Text>
              </Box>
            );
          })}
        </Box>
        <Text color={theme.text.secondary}>{cycleHint} switch mode</Text>
      </Box>

      {/* Structured Workspace & Quick-Start Card */}
      {showDetails && (
        <Box
          width={cardWidth}
          flexDirection="column"
          borderStyle="round"
          borderColor={theme.border.default}
          paddingX={1}
        >
          <Box flexDirection={isNarrow ? 'column' : 'row'} width="100%">
            <Box
              flexDirection="column"
              width={isNarrow ? '100%' : '52%'}
              paddingRight={isNarrow ? 0 : 1}
            >
              <Text bold color={theme.text.accent}>
                Workspace
              </Text>
              <Box>
                <Text color={theme.text.secondary}>Dir: </Text>
                <Text color={theme.text.primary} wrap="truncate-end">
                  {dirDisplay}
                </Text>
              </Box>
              {uiState.branchName ? (
                <Box>
                  <Text color={theme.text.secondary}>Branch: </Text>
                  <Text color={theme.text.link} wrap="truncate-end">
                    {uiState.branchName}
                  </Text>
                </Box>
              ) : null}
              <Box>
                <Text color={theme.text.secondary}>Context: </Text>
                <Text color={theme.text.primary} wrap="truncate-end">
                  {contextDisplay}
                </Text>
              </Box>
            </Box>

            <Box
              flexDirection="column"
              width={isNarrow ? '100%' : '48%'}
              marginTop={isNarrow ? 1 : 0}
            >
              <Text bold color={theme.text.accent}>
                Session
              </Text>
              <Box>
                <Text color={theme.text.secondary}>Model: </Text>
                <Text color={theme.text.primary} wrap="truncate-end">
                  {activeModel}
                </Text>
              </Box>
              {thinkingEffort && thinkingEffort !== 'default' ? (
                <Box>
                  <Text color={theme.text.secondary}>Thinking: </Text>
                  <Text color={theme.text.primary} wrap="truncate-end">
                    {thinkingEffort}
                  </Text>
                </Box>
              ) : null}
              <Box>
                <Text color={theme.text.secondary}>Auth: </Text>
                <Text
                  color={
                    !providerSummary || providerSummary.isWarning
                      ? theme.status.warning
                      : theme.status.success
                  }
                  wrap="truncate-end"
                >
                  {providerSummary ? providerSummary.label : 'Not configured'}
                </Text>
              </Box>
              {(providerSummary?.tierName || sandboxEnabled) && (
                <Box>
                  <Text color={theme.text.secondary}>Env: </Text>
                  <Text color={theme.text.primary} wrap="truncate-end">
                    {[
                      providerSummary?.tierName,
                      sandboxEnabled ? 'sandbox' : null,
                    ]
                      .filter(Boolean)
                      .join(' · ')}
                  </Text>
                </Box>
              )}
            </Box>
          </Box>

          <Box
            flexDirection="column"
            marginTop={1}
            borderTop
            borderBottom={false}
            borderLeft={false}
            borderRight={false}
            borderStyle="single"
            borderColor={theme.border.default}
            paddingTop={1}
          >
            <Text bold color={theme.text.accent}>
              Quick start
            </Text>
            {STARTER_ACTIONS.map((action) => (
              <Box key={action.key} flexDirection="row">
                <Text color={theme.text.link} bold>
                  [Alt+{action.key}]{' '}
                </Text>
                <Text color={theme.text.primary} bold>
                  {action.label}
                </Text>
                {!isNarrow && (
                  <Text color={theme.text.secondary} wrap="truncate-end">
                    {' — '}
                    {action.description}
                  </Text>
                )}
              </Box>
            ))}
          </Box>

          <Box marginTop={1}>
            <Text color={theme.text.secondary} wrap="truncate-end">
              @file attach · /commands · !shell · {cycleHint} mode · Alt+1..4
              start · ? help
            </Text>
          </Box>
        </Box>
      )}

      {uiState.bannerVisible && bannerText ? (
        <Box width={cardWidth} marginTop={1}>
          <Banner
            bannerText={bannerText}
            isWarning={Boolean(uiState.bannerData?.warningText)}
            width={cardWidth}
          />
        </Box>
      ) : null}
    </Box>
  );
};
