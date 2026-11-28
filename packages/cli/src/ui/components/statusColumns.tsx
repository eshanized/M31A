/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Text } from 'ink';
import process from 'node:process';
import os from 'node:os';
import {
  shortenPath,
  tildeifyPath,
  getDisplayString,
  checkExhaustive,
  AuthType,
  type Config,
} from '@m31a/core';
import { theme } from '../semantic-colors.js';
import { ConsoleSummaryDisplay } from './ConsoleSummaryDisplay.js';
import { MemoryUsageDisplay } from './MemoryUsageDisplay.js';
import { ContextUsageDisplay } from './ContextUsageDisplay.js';
import { QuotaDisplay } from './QuotaDisplay.js';
import { DebugProfiler } from './DebugProfiler.js';
import { isReducedMotionEnabled } from './StatusGlyph.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useQuotaState } from '../contexts/QuotaContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { useSettings } from '../contexts/SettingsContext.js';
import { useVimMode } from '../contexts/VimModeContext.js';
import { useInputState } from '../contexts/InputContext.js';
import {
  ALL_ITEMS,
  type FooterItemId,
  deriveItemsFromLegacySettings,
} from '../../config/footerItems.js';
import { isDevelopment } from '../../utils/installationInfo.js';
import { formatCompactTokenCount } from '../utils/formatters.js';

const HOSTNAME = os.hostname();

/**
 * Checks whether an NVIDIA NIM API key is configured without ever exposing the key value.
 */
export function hasNvidiaNimKey(config: Config): boolean {
  return Boolean(
    process.env['NVIDIA_API_KEY'] || config.getContentGeneratorConfig()?.apiKey,
  );
}

export interface StatusColumnItem {
  id: string;
  header: string;
  element: (maxWidth: number) => React.ReactNode;
  width: number;
  isHighPriority: boolean;
}

interface CwdIndicatorProps {
  targetDir: string;
  maxWidth: number;
  debugMode?: boolean;
  debugMessage?: string;
  color?: string;
}

export const CwdIndicator: React.FC<CwdIndicatorProps> = ({
  targetDir,
  maxWidth,
  debugMode,
  debugMessage,
  color = theme.text.primary,
}) => {
  const debugSuffix = debugMode ? ' ' + (debugMessage || '--debug') : '';
  const availableForPath = Math.max(10, maxWidth - debugSuffix.length);
  const displayPath = shortenPath(tildeifyPath(targetDir), availableForPath);

  return (
    <Text color={color}>
      {displayPath}
      {debugMode && <Text color={theme.status.error}>{debugSuffix}</Text>}
    </Text>
  );
};

export const SandboxIndicator: React.FC<{
  isTrustedFolder: boolean | undefined;
}> = ({ isTrustedFolder }) => {
  const config = useConfig();
  const sandboxEnabled = config.getSandboxEnabled();
  if (isTrustedFolder === false) {
    return <Text color={theme.status.warning}>untrusted</Text>;
  }

  const sandbox = process.env['SANDBOX'];
  if (sandbox) {
    return <Text color={theme.status.warning}>current process</Text>;
  }

  if (sandboxEnabled) {
    return <Text color={theme.status.warning}>all tools</Text>;
  }

  return <Text color={theme.status.error}>no sandbox</Text>;
};

export const CorgiIndicator: React.FC = () => (
  <Text>
    <Text color={theme.status.error}>▼</Text>
    <Text color={theme.text.primary}>(´</Text>
    <Text color={theme.status.error}>ᴥ</Text>
    <Text color={theme.text.primary}>`)</Text>
    <Text color={theme.status.error}>▼</Text>
  </Text>
);

function isFooterItemId(id: string): id is FooterItemId {
  return ALL_ITEMS.some((i) => i.id === id);
}

/**
 * Collects ordered workspace telemetry items shared between the Sidebar and narrow Footer fallback.
 */
export function useStatusColumnItems(widthForContext: number): {
  columns: StatusColumnItem[];
  showLabels: boolean;
} {
  const uiState = useUIState();
  const quotaState = useQuotaState();
  const { copyModeEnabled, shellModeActive } = useInputState();
  const config = useConfig();
  const settings = useSettings();
  const { vimEnabled, vimMode } = useVimMode();

  const authType = config.getContentGeneratorConfig()?.authType;
  const model = uiState.currentModel;
  const targetDir = config.getTargetDir();
  const debugMode = config.getDebugMode();
  const branchName = uiState.branchName;
  const debugMessage = uiState.debugMessage;
  const corgiMode = uiState.corgiMode;
  const errorCount = uiState.errorCount;
  const showErrorDetails = uiState.showErrorDetails;
  const promptTokenCount = uiState.sessionStats?.lastPromptTokenCount ?? 0;
  const isTrustedFolder = uiState.isTrustedFolder;
  const quotaStats = quotaState.stats;

  const isFullErrorVerbosity = settings.merged.ui.errorVerbosity === 'full';
  const showErrorSummary =
    !showErrorDetails &&
    errorCount > 0 &&
    (isFullErrorVerbosity || debugMode || isDevelopment);
  const displayVimMode = vimEnabled ? vimMode : undefined;

  const items =
    settings.merged.ui.footer.items ??
    deriveItemsFromLegacySettings(settings.merged);
  const showLabels = settings.merged.ui.footer.showLabels !== false;
  const itemColor = showLabels ? theme.text.primary : theme.ui.comment;

  const columns: StatusColumnItem[] = [];

  const addCol = (
    id: string,
    header: string,
    element: (maxWidth: number) => React.ReactNode,
    dataWidth: number,
    isHighPriority = false,
  ) => {
    columns.push({
      id,
      header: showLabels ? header : '',
      element,
      width: Math.max(dataWidth, showLabels ? header.length : 0),
      isHighPriority,
    });
  };

  if (uiState.showDebugProfiler) {
    addCol('debug', '', () => <DebugProfiler />, 45, true);
  }
  if (displayVimMode) {
    const vimStr = `[${displayVimMode}]`;
    addCol(
      'vim',
      '',
      () => <Text color={theme.text.accent}>{vimStr}</Text>,
      vimStr.length,
      true,
    );
  }
  if (shellModeActive) {
    addCol(
      'shell-mode',
      '',
      () => <Text color={theme.status.warning}>[! shell]</Text>,
      9,
      true,
    );
  }
  if (copyModeEnabled) {
    addCol(
      'copy-mode',
      '',
      () => <Text color={theme.status.warning}>[copy]</Text>,
      6,
      true,
    );
  }
  if (isReducedMotionEnabled()) {
    addCol(
      'reduced-motion',
      '',
      () => <Text color={theme.text.secondary}>[reduced-motion]</Text>,
      16,
      true,
    );
  }

  for (const id of items) {
    if (!isFooterItemId(id)) continue;
    const itemConfig = ALL_ITEMS.find((i) => i.id === id);
    const header = itemConfig?.header ?? id;

    switch (id) {
      case 'workspace': {
        const fullPath = tildeifyPath(targetDir);
        const debugSuffix = debugMode ? ' ' + (debugMessage || '--debug') : '';
        addCol(
          id,
          header,
          (maxWidth) => (
            <CwdIndicator
              targetDir={targetDir}
              maxWidth={maxWidth}
              debugMode={debugMode}
              debugMessage={debugMessage}
              color={itemColor}
            />
          ),
          fullPath.length + debugSuffix.length,
        );
        break;
      }
      case 'git-branch': {
        if (branchName) {
          addCol(
            id,
            header,
            () => (
              <Text color={itemColor} wrap="truncate-end">
                {branchName}
              </Text>
            ),
            branchName.length,
          );
        }
        break;
      }
      case 'sandbox': {
        let str = 'no sandbox';
        const sandbox = process.env['SANDBOX'];
        if (isTrustedFolder === false) str = 'untrusted';
        else if (sandbox) str = 'current process';
        else if (config.getSandboxEnabled()) str = 'all tools';

        addCol(
          id,
          header,
          () => <SandboxIndicator isTrustedFolder={isTrustedFolder} />,
          str.length,
        );
        break;
      }
      case 'model-name': {
        const str = getDisplayString(model);
        addCol(
          id,
          header,
          () => (
            <Text color={itemColor} wrap="truncate-end">
              {str}
            </Text>
          ),
          str.length,
        );
        break;
      }
      case 'context-used': {
        addCol(
          id,
          header,
          () => (
            <ContextUsageDisplay
              promptTokenCount={promptTokenCount}
              model={model}
              terminalWidth={widthForContext}
            />
          ),
          10,
        );
        break;
      }
      case 'quota': {
        if (quotaStats?.remaining !== undefined && quotaStats.limit) {
          addCol(
            id,
            header,
            () => (
              <QuotaDisplay
                remaining={quotaStats.remaining}
                limit={quotaStats.limit}
                forceShow={true}
                lowercase={true}
              />
            ),
            9,
          );
        }
        break;
      }
      case 'memory-usage': {
        addCol(
          id,
          header,
          () => (
            <MemoryUsageDisplay
              color={itemColor}
              isActive={!copyModeEnabled && uiState.constrainHeight}
            />
          ),
          10,
        );
        break;
      }
      case 'session-id': {
        addCol(
          id,
          header,
          () => (
            <Text color={itemColor}>
              {(uiState.sessionStats?.sessionId ?? '').slice(0, 8)}
            </Text>
          ),
          8,
        );
        break;
      }
      case 'hostname': {
        addCol(
          id,
          header,
          () => (
            <Text color={itemColor} wrap="truncate-end">
              {HOSTNAME}
            </Text>
          ),
          HOSTNAME.length,
        );
        break;
      }
      case 'auth': {
        if (!settings.merged.ui.showUserIdentity) break;
        if (!authType) break;
        const displayStr =
          authType === AuthType.USE_NVIDIA_NIM
            ? `${authType} (${hasNvidiaNimKey(config) ? 'key ✓' : 'no key'})`
            : authType;
        addCol(
          id,
          header,
          () => (
            <Text color={itemColor} wrap="truncate-end">
              {displayStr}
            </Text>
          ),
          displayStr.length,
        );
        break;
      }
      case 'code-changes': {
        const added =
          uiState.sessionStats?.metrics?.files?.totalLinesAdded ?? 0;
        const removed =
          uiState.sessionStats?.metrics?.files?.totalLinesRemoved ?? 0;
        if (added > 0 || removed > 0) {
          const str = `+${added} -${removed}`;
          addCol(
            id,
            header,
            () => (
              <Text>
                <Text color={theme.status.success}>+{added}</Text>{' '}
                <Text color={theme.status.error}>-{removed}</Text>
              </Text>
            ),
            str.length,
          );
        }
        break;
      }
      case 'token-count': {
        let total = 0;
        for (const m of Object.values(
          uiState.sessionStats?.metrics?.models ?? {},
        ))
          total += m.tokens.total;
        if (total > 0) {
          const formatted = formatCompactTokenCount(total);
          addCol(
            id,
            header,
            () => <Text color={itemColor}>{formatted} tokens</Text>,
            formatted.length + 7,
          );
        }
        break;
      }
      default:
        checkExhaustive(id);
        break;
    }
  }

  if (corgiMode) addCol('corgi', '', () => <CorgiIndicator />, 5);
  if (showErrorSummary) {
    addCol(
      'error-count',
      '',
      () => <ConsoleSummaryDisplay errorCount={errorCount} />,
      12,
      true,
    );
  }

  return { columns, showLabels };
}
