/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useMemo } from 'react';
import { Box, Text } from 'ink';
import {
  escapeShellArg,
  getDisplayString,
  isWindows,
  type ShellType,
} from '@m31a/core';
import {
  useSessionStats,
  type ModelMetrics,
} from '../contexts/SessionContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { computeSessionStats } from '../utils/computeStats.js';
import {
  getStatusColor,
  TOOL_SUCCESS_RATE_HIGH,
  TOOL_SUCCESS_RATE_MEDIUM,
  USER_AGREEMENT_RATE_HIGH,
  USER_AGREEMENT_RATE_MEDIUM,
} from '../utils/displayUtils.js';
import { formatDuration } from '../utils/formatters.js';
import { theme } from '../semantic-colors.js';
import { ThemedGradient } from './ThemedGradient.js';

interface SessionSummaryDisplayProps {
  duration: string;
}

interface TokenTotals {
  prompt: number;
  cached: number;
  thoughts: number;
  output: number;
  total: number;
}

function summarizeModelTokens(
  models: Record<string, ModelMetrics>,
): TokenTotals {
  return Object.values(models).reduce<TokenTotals>(
    (acc, model) => ({
      prompt: acc.prompt + model.tokens.prompt,
      cached: acc.cached + model.tokens.cached,
      thoughts: acc.thoughts + model.tokens.thoughts,
      output: acc.output + model.tokens.candidates,
      total: acc.total + model.tokens.total,
    }),
    { prompt: 0, cached: 0, thoughts: 0, output: 0, total: 0 },
  );
}

interface SummaryRowProps {
  label: string;
  children: React.ReactNode;
}

const SummaryRow: React.FC<SummaryRowProps> = ({ label, children }) => (
  <Box>
    <Box width={20}>
      <Text color={theme.text.link}>{label}</Text>
    </Box>
    {children}
  </Box>
);

const COL_MODEL = 26;
const COL_REQS = 6;
const COL_NUM = 11;
const COL_TOTAL = 12;

export const SessionSummaryDisplay: React.FC<SessionSummaryDisplayProps> = ({
  duration,
}) => {
  const { stats } = useSessionStats();
  const config = useConfig();
  const shell: ShellType = isWindows() ? 'powershell' : 'bash';

  const { metrics } = stats;
  const { tools, files, models } = metrics;
  const computed = useMemo(() => computeSessionStats(metrics), [metrics]);
  const tokenTotals = useMemo(() => summarizeModelTokens(models), [models]);
  const modelEntries = Object.entries(models);

  const worktreeSettings = config.getWorktreeSettings();

  const escapedSessionId = escapeShellArg(stats.sessionId, shell);
  const footerSessionId =
    isWindows() &&
    !escapedSessionId.startsWith('"') &&
    !escapedSessionId.startsWith("'")
      ? `"${escapedSessionId}"`
      : escapedSessionId;

  const footer = worktreeSettings
    ? `To resume work in this worktree: cd ${escapeShellArg(worktreeSettings.path, shell)} && m31a --resume ${footerSessionId}\n` +
      `To remove manually: git worktree remove ${escapeShellArg(worktreeSettings.path, shell)}`
    : `To resume this session: m31a --resume ${footerSessionId}`;

  const successColor = getStatusColor(computed.successRate, {
    green: TOOL_SUCCESS_RATE_HIGH,
    yellow: TOOL_SUCCESS_RATE_MEDIUM,
  });
  const agreementColor = getStatusColor(computed.agreementRate, {
    green: USER_AGREEMENT_RATE_HIGH,
    yellow: USER_AGREEMENT_RATE_MEDIUM,
  });

  const hasTools = tools.totalCalls > 0;
  const hasCodeChanges =
    Boolean(files) &&
    (files.totalLinesAdded > 0 || files.totalLinesRemoved > 0);
  const hasModels = modelEntries.length > 0;

  return (
    <Box
      borderStyle="round"
      borderColor={theme.border.default}
      flexDirection="column"
      paddingY={1}
      paddingX={2}
      overflow="hidden"
    >
      <ThemedGradient bold>Agent powering down. Goodbye!</ThemedGradient>

      <Box flexDirection="column" marginTop={1}>
        <SummaryRow label="Duration:">
          <Text color={theme.text.primary}>
            {duration}
            {computed.agentActiveTime > 0 && (
              <Text color={theme.text.secondary}>
                {' '}
                (active {formatDuration(computed.agentActiveTime)})
              </Text>
            )}
          </Text>
        </SummaryRow>

        {stats.promptCount > 0 && (
          <SummaryRow label="Turns:">
            <Text color={theme.text.primary}>{stats.promptCount}</Text>
          </SummaryRow>
        )}

        {hasTools && (
          <SummaryRow label="Tools:">
            <Text color={theme.text.primary}>
              {tools.totalCalls} ({' '}
              <Text color={theme.status.success}>✓ {tools.totalSuccess}</Text>{' '}
              <Text color={theme.status.error}>x {tools.totalFail}</Text> ) ·{' '}
              <Text color={successColor}>
                {computed.successRate.toFixed(1)}%
              </Text>
            </Text>
          </SummaryRow>
        )}

        {computed.totalDecisions > 0 && (
          <SummaryRow label="User Agreement:">
            <Text color={agreementColor}>
              {computed.agreementRate.toFixed(1)}%{' '}
              <Text color={theme.text.secondary}>
                ({computed.totalDecisions} reviewed)
              </Text>
            </Text>
          </SummaryRow>
        )}

        {hasCodeChanges && (
          <SummaryRow label="Code Changes:">
            <Text>
              <Text color={theme.status.success}>+{files.totalLinesAdded}</Text>{' '}
              <Text color={theme.status.error}>-{files.totalLinesRemoved}</Text>
            </Text>
          </SummaryRow>
        )}
      </Box>

      {hasModels && (
        <Box flexDirection="column" marginTop={1}>
          <SummaryRow label="Total Tokens:">
            <Text bold color={theme.text.primary}>
              {tokenTotals.total.toLocaleString()}
            </Text>
          </SummaryRow>
          <SummaryRow label="Breakdown:">
            <Text color={theme.text.secondary}>
              in {tokenTotals.prompt.toLocaleString()} · out{' '}
              {tokenTotals.output.toLocaleString()}
              {tokenTotals.thoughts > 0
                ? ` · thinking ${tokenTotals.thoughts.toLocaleString()}`
                : ''}
              {tokenTotals.cached > 0
                ? ` · cache ${tokenTotals.cached.toLocaleString()} (${computed.cacheEfficiency.toFixed(1)}% hit)`
                : ''}
            </Text>
          </SummaryRow>

          <Box
            marginTop={1}
            borderBottom
            borderStyle="single"
            borderColor={theme.border.default}
            borderTop={false}
            borderLeft={false}
            borderRight={false}
          >
            <Box width={COL_MODEL}>
              <Text bold color={theme.text.secondary}>
                Model
              </Text>
            </Box>
            <Box width={COL_REQS} justifyContent="flex-end">
              <Text bold color={theme.text.secondary}>
                Reqs
              </Text>
            </Box>
            <Box width={COL_NUM} justifyContent="flex-end">
              <Text bold color={theme.text.secondary}>
                Input
              </Text>
            </Box>
            <Box width={COL_NUM} justifyContent="flex-end">
              <Text bold color={theme.text.secondary}>
                Cache
              </Text>
            </Box>
            <Box width={COL_NUM} justifyContent="flex-end">
              <Text bold color={theme.text.secondary}>
                Thinking
              </Text>
            </Box>
            <Box width={COL_NUM} justifyContent="flex-end">
              <Text bold color={theme.text.secondary}>
                Output
              </Text>
            </Box>
            <Box width={COL_TOTAL} justifyContent="flex-end">
              <Text bold color={theme.text.secondary}>
                Total
              </Text>
            </Box>
          </Box>

          {modelEntries.map(([name, modelMetrics]) => (
            <Box key={name}>
              <Box width={COL_MODEL}>
                <Text color={theme.text.primary} wrap="truncate-end">
                  {getDisplayString(name)}
                </Text>
              </Box>
              <Box width={COL_REQS} justifyContent="flex-end">
                <Text color={theme.text.primary}>
                  {modelMetrics.api.totalRequests}
                </Text>
              </Box>
              <Box width={COL_NUM} justifyContent="flex-end">
                <Text color={theme.text.primary}>
                  {modelMetrics.tokens.prompt.toLocaleString()}
                </Text>
              </Box>
              <Box width={COL_NUM} justifyContent="flex-end">
                <Text color={theme.text.primary}>
                  {modelMetrics.tokens.cached.toLocaleString()}
                </Text>
              </Box>
              <Box width={COL_NUM} justifyContent="flex-end">
                <Text color={theme.text.primary}>
                  {modelMetrics.tokens.thoughts.toLocaleString()}
                </Text>
              </Box>
              <Box width={COL_NUM} justifyContent="flex-end">
                <Text color={theme.text.primary}>
                  {modelMetrics.tokens.candidates.toLocaleString()}
                </Text>
              </Box>
              <Box width={COL_TOTAL} justifyContent="flex-end">
                <Text bold color={theme.text.primary}>
                  {modelMetrics.tokens.total.toLocaleString()}
                </Text>
              </Box>
            </Box>
          ))}
        </Box>
      )}

      <Box marginTop={1}>
        <ThemedGradient bold>{footer}</ThemedGradient>
      </Box>
    </Box>
  );
};
