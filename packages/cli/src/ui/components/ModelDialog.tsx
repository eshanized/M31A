/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { Box, Text } from 'ink';
import {
  DEFAULT_M31A_MODEL,
  M31A_MODEL_ALIAS_AUTO,
  ModelSlashCommandEvent,
  logModelSlashCommand,
  getDisplayString,
  getAutoModelDescription,
  NIM_MODEL_CATALOG,
  NIM_THINKING_EFFORT_OPTIONS,
  NIM_CONTEXT_WINDOW_PRESETS,
  fetchNimAvailableModels,
  formatTokenCountShort,
  getNimCatalogEntry,
  resolveModel,
  tokenLimit,
  type NimThinkingEffort,
} from '@m31a/core';
import { useKeypress } from '../hooks/useKeypress.js';
import { theme } from '../semantic-colors.js';
import { DescriptiveRadioButtonSelect } from './shared/DescriptiveRadioButtonSelect.js';
import { ConfigContext } from '../contexts/ConfigContext.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useSettings } from '../contexts/SettingsContext.js';
import { SettingScope } from '../../config/settings.js';
import { TextInput } from './shared/TextInput.js';
import { useTextBuffer } from './shared/text-buffer.js';

interface ModelDialogProps {
  onClose: () => void;
}

const CUSTOM_MODEL_OPTION = '__custom_nim_model__';
const THINKING_MENU_OPTION = '__nim_thinking_effort__';
const CONTEXT_MENU_OPTION = '__nim_context_window__';

type DialogView = 'main' | 'manual' | 'custom' | 'thinking' | 'context';

export function ModelDialog({ onClose }: ModelDialogProps): React.JSX.Element {
  const config = useContext(ConfigContext);
  const settings = useSettings();
  const { mainAreaWidth, terminalWidth } = useUIState();
  const [view, setView] = useState<DialogView>('main');
  const [persistMode, setPersistMode] = useState(false);
  const [liveModelIds, setLiveModelIds] = useState<string[]>([]);

  const preferredModel = config?.getModel() || M31A_MODEL_ALIAS_AUTO;
  const resolvedConcreteModel = useMemo(() => {
    if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
      return (
        config.getModelConfigService?.()?.resolveModelId(preferredModel, {}) ??
        config.modelConfigService?.resolveModelId(preferredModel, {}) ??
        DEFAULT_M31A_MODEL
      );
    }
    return resolveModel(preferredModel, config ?? undefined);
  }, [preferredModel, config]);
  const activeThinkingEffort: NimThinkingEffort =
    config?.getThinkingEffort?.() ?? 'default';
  const contextOverride = config?.getContextWindowOverride?.();
  const effectiveContextTokens = tokenLimit(
    resolvedConcreteModel,
    contextOverride,
  );
  const dialogContentWidth = Math.max(20, (mainAreaWidth || terminalWidth) - 8);

  const customBuffer = useTextBuffer({
    initialText: preferredModel !== M31A_MODEL_ALIAS_AUTO ? preferredModel : '',
    viewport: {
      width: dialogContentWidth,
      height: 1,
    },
    singleLine: true,
  });

  const nimBaseUrl =
    process.env['NVIDIA_NIM_BASE_URL'] || 'https://integrate.api.nvidia.com/v1';

  useEffect(() => {
    // Skip unsolicited network calls inside unit tests unless running interactively.
    if (view !== 'manual' || process.env['VITEST']) {
      return;
    }
    let cancelled = false;
    const apiKey =
      process.env['NVIDIA_API_KEY'] ||
      config?.getContentGeneratorConfig?.()?.apiKey ||
      '';
    if (!apiKey) {
      return;
    }
    void fetchNimAvailableModels(nimBaseUrl, apiKey).then((ids) => {
      if (!cancelled && ids.length > 0) {
        setLiveModelIds(ids);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [view, nimBaseUrl, config]);

  const manualModelSelected = useMemo(() => {
    if (
      config?.getExperimentalDynamicModelConfiguration?.() === true &&
      config.getModelConfigService
    ) {
      const def = config
        .getModelConfigService()
        .getModelDefinition(preferredModel);
      return def && def.tier !== 'auto' && def.isVisible === true
        ? preferredModel
        : '';
    }

    if (preferredModel !== M31A_MODEL_ALIAS_AUTO) {
      return preferredModel;
    }
    return '';
  }, [preferredModel, config]);

  useKeypress(
    (key) => {
      if (key.name === 'escape') {
        if (view !== 'main') {
          setView('main');
        } else {
          onClose();
        }
        return true;
      }
      if (key.name === 'tab' && view !== 'custom') {
        setPersistMode((prev) => !prev);
        return true;
      }
      return false;
    },
    { isActive: true },
  );

  const mainOptions = useMemo(() => {
    const contextLabel = `${formatTokenCountShort(effectiveContextTokens)} (${
      contextOverride ? 'custom' : 'auto'
    })`;
    const thinkingLabel = `Thinking effort (${activeThinkingEffort})`;

    const extraSettingsOptions = [
      {
        value: THINKING_MENU_OPTION,
        title: thinkingLabel,
        description:
          'Configure reasoning effort / thinking budget for NIM reasoning models',
        key: THINKING_MENU_OPTION,
      },
      {
        value: CONTEXT_MENU_OPTION,
        title: `Context window (${contextLabel})`,
        description:
          'Set custom context window token cap or use model catalog default',
        key: CONTEXT_MENU_OPTION,
      },
    ];

    if (
      config?.getExperimentalDynamicModelConfiguration?.() === true &&
      config.getModelConfigService
    ) {
      const allOptions = config
        .getModelConfigService()
        .getAvailableModelOptions({});

      const list = allOptions
        .filter((o) => o.tier === 'auto')
        .map((o) => ({
          value: o.modelId,
          title: o.name,
          description: o.description,
          key: o.modelId,
        }));

      list.push({
        value: 'Manual',
        title: manualModelSelected
          ? `Manual (${getDisplayString(manualModelSelected, config ?? undefined)})`
          : 'Manual',
        description: 'Browse NVIDIA NIM models catalog',
        key: 'Manual',
      });
      list.push({
        value: CUSTOM_MODEL_OPTION,
        title: 'Custom NVIDIA NIM model ID',
        description:
          'Enter any NIM model slug (e.g. nvidia/nemotron-3-ultra-550b-a55b)',
        key: CUSTOM_MODEL_OPTION,
      });
      return [...list, ...extraSettingsOptions];
    }

    return [
      {
        value: M31A_MODEL_ALIAS_AUTO,
        title: getDisplayString(M31A_MODEL_ALIAS_AUTO),
        description: getAutoModelDescription(),
        key: M31A_MODEL_ALIAS_AUTO,
      },
      {
        value: 'Manual',
        title: manualModelSelected
          ? `Manual (${getDisplayString(manualModelSelected)})`
          : 'Manual',
        description: 'Browse NVIDIA NIM models catalog',
        key: 'Manual',
      },
      {
        value: CUSTOM_MODEL_OPTION,
        title: 'Custom NVIDIA NIM model ID',
        description:
          'Enter any NIM model slug (e.g. nvidia/nemotron-3-ultra-550b-a55b)',
        key: CUSTOM_MODEL_OPTION,
      },
      ...extraSettingsOptions,
    ];
  }, [
    config,
    manualModelSelected,
    effectiveContextTokens,
    contextOverride,
    activeThinkingEffort,
  ]);

  const manualOptions = useMemo(() => {
    const items: Array<{
      value: string;
      title: string;
      description?: string;
      key: string;
    }> = [];

    if (
      config?.getExperimentalDynamicModelConfiguration?.() === true &&
      config.getModelConfigService
    ) {
      const allOptions = config
        .getModelConfigService()
        .getAvailableModelOptions({});

      for (const o of allOptions.filter((opt) => opt.tier !== 'auto')) {
        items.push({
          value: o.modelId,
          title: o.name,
          key: o.modelId,
        });
      }
    } else {
      const defaultEntry = getNimCatalogEntry(DEFAULT_M31A_MODEL);
      items.push({
        value: DEFAULT_M31A_MODEL,
        title: getDisplayString(DEFAULT_M31A_MODEL),
        description: defaultEntry
          ? `${defaultEntry.name} · ${formatTokenCountShort(defaultEntry.contextWindow)} ctx · ${formatTokenCountShort(defaultEntry.maxOutputTokens)} out · ${defaultEntry.thinkingMode}`
          : undefined,
        key: DEFAULT_M31A_MODEL,
      });
    }

    for (const entry of NIM_MODEL_CATALOG) {
      if (items.some((item) => item.value === entry.id)) {
        continue;
      }
      const modeTag =
        entry.thinkingMode === 'none'
          ? ''
          : ` · ${entry.thinkingMode} thinking`;
      items.push({
        value: entry.id,
        title: `${entry.id} (${entry.name})`,
        description: `${formatTokenCountShort(entry.contextWindow)} ctx · ${formatTokenCountShort(entry.maxOutputTokens)} out${modeTag}`,
        key: entry.id,
      });
    }

    for (const liveId of liveModelIds) {
      if (items.some((item) => item.value === liveId)) {
        continue;
      }
      items.push({
        value: liveId,
        title: liveId,
        description: 'Live NVIDIA NIM /models endpoint',
        key: liveId,
      });
    }

    return items;
  }, [config, liveModelIds]);

  const thinkingOptions = useMemo(() => {
    const catalog = getNimCatalogEntry(resolvedConcreteModel);
    return NIM_THINKING_EFFORT_OPTIONS.map((effort) => {
      let description = '';
      if (effort === 'default') {
        description = catalog
          ? `Use default (${catalog.thinkingMode} mode for ${catalog.name})`
          : 'Use model configuration default thinking behavior';
      } else if (effort === 'off') {
        description = 'Disable reasoning/thinking via chat_template_kwargs';
      } else {
        description = `Request ${effort} reasoning effort / thinking budget`;
      }
      return {
        value: effort,
        title: effort === activeThinkingEffort ? `${effort} (active)` : effort,
        description,
        key: effort,
      };
    });
  }, [resolvedConcreteModel, activeThinkingEffort]);

  const contextOptions = useMemo(() => {
    const catalogLimit = tokenLimit(resolvedConcreteModel);
    const items = NIM_CONTEXT_WINDOW_PRESETS.map((preset) => {
      const value =
        preset.tokens === undefined ? 'auto' : String(preset.tokens);
      const isSelected =
        preset.tokens === undefined
          ? contextOverride === undefined
          : contextOverride === preset.tokens;
      return {
        value,
        title: isSelected ? `${preset.label} (active)` : preset.label,
        description:
          preset.tokens === undefined
            ? `Model catalog default: ${formatTokenCountShort(catalogLimit)} (${catalogLimit.toLocaleString()} tokens)`
            : undefined,
        key: value,
      };
    });
    if (
      contextOverride !== undefined &&
      !NIM_CONTEXT_WINDOW_PRESETS.some((p) => p.tokens === contextOverride)
    ) {
      items.push({
        value: String(contextOverride),
        title: `${formatTokenCountShort(contextOverride)} (${contextOverride.toLocaleString()} tokens) (active)`,
        description: 'Custom context window override',
        key: String(contextOverride),
      });
    }
    return items;
  }, [resolvedConcreteModel, contextOverride]);

  const options = useMemo(() => {
    let rawOptions: Array<{
      value: string;
      title: string;
      description?: string;
      key: string;
    }> = mainOptions;
    if (view === 'manual') rawOptions = manualOptions;
    else if (view === 'thinking') rawOptions = thinkingOptions;
    else if (view === 'context') rawOptions = contextOptions;

    const seen = new Set<string>();
    return rawOptions.filter((option) => {
      if (seen.has(option.value)) {
        return false;
      }
      seen.add(option.value);
      return true;
    });
  }, [view, mainOptions, manualOptions, thinkingOptions, contextOptions]);

  const initialIndex = useMemo(() => {
    if (view === 'thinking') {
      const idx = options.findIndex(
        (option) => option.value === activeThinkingEffort,
      );
      return idx !== -1 ? idx : 0;
    }
    if (view === 'context') {
      const target =
        contextOverride === undefined ? 'auto' : String(contextOverride);
      const idx = options.findIndex((option) => option.value === target);
      return idx !== -1 ? idx : 0;
    }
    const idx = options.findIndex((option) => option.value === preferredModel);
    if (idx !== -1) {
      return idx;
    }
    if (view === 'main') {
      const manualIdx = options.findIndex((o) => o.value === 'Manual');
      return manualIdx !== -1 ? manualIdx : 0;
    }
    return 0;
  }, [preferredModel, options, view, activeThinkingEffort, contextOverride]);

  const applyModelSelection = useCallback(
    (model: string) => {
      const trimmed = model.trim();
      if (!trimmed) {
        return;
      }
      if (config) {
        config.setModel(trimmed, persistMode ? false : true);
        const event = new ModelSlashCommandEvent(trimmed);
        logModelSlashCommand(config, event);
      }
      onClose();
    },
    [config, onClose, persistMode],
  );

  const applyThinkingSelection = useCallback(
    (effortValue: string) => {
      const effort = NIM_THINKING_EFFORT_OPTIONS.find(
        (opt) => opt === effortValue,
      );
      if (!effort) return;
      config?.setThinkingEffort?.(effort, !persistMode);
      if (persistMode && settings?.setValue) {
        settings.setValue(SettingScope.User, 'model.thinkingEffort', effort);
      }
      onClose();
    },
    [config, persistMode, settings, onClose],
  );

  const applyContextSelection = useCallback(
    (value: string) => {
      const parsed = value === 'auto' ? undefined : Number.parseInt(value, 10);
      const tokens =
        parsed !== undefined && Number.isFinite(parsed) && parsed > 0
          ? parsed
          : undefined;
      config?.setContextWindowOverride?.(tokens, !persistMode);
      if (persistMode && settings?.setValue) {
        settings.setValue(SettingScope.User, 'model.contextWindow', tokens);
      }
      onClose();
    },
    [config, persistMode, settings, onClose],
  );

  const handleSelect = useCallback(
    (selected: string) => {
      if (view === 'thinking') {
        applyThinkingSelection(selected);
        return;
      }
      if (view === 'context') {
        applyContextSelection(selected);
        return;
      }
      if (selected === 'Manual') {
        setView('manual');
        return;
      }
      if (selected === CUSTOM_MODEL_OPTION) {
        setView('custom');
        return;
      }
      if (selected === THINKING_MENU_OPTION) {
        setView('thinking');
        return;
      }
      if (selected === CONTEXT_MENU_OPTION) {
        setView('context');
        return;
      }
      applyModelSelection(selected);
    },
    [view, applyThinkingSelection, applyContextSelection, applyModelSelection],
  );

  const heading =
    view === 'thinking'
      ? '◆ Select Thinking Effort'
      : view === 'context'
        ? '◆ Select Context Window'
        : view === 'manual'
          ? '◆ NVIDIA NIM Models Catalog'
          : '◆ Select NVIDIA NIM Model';

  return (
    <Box
      borderStyle="round"
      borderColor={theme.border.default}
      flexDirection="column"
      padding={1}
      width="100%"
    >
      <Box justifyContent="space-between" width="100%">
        <Text bold color={theme.text.accent}>
          {heading}
        </Text>
        <Text color={theme.status.success}>NIM</Text>
      </Box>
      <Text color={theme.text.secondary} wrap="truncate-end">
        Endpoint: {nimBaseUrl} · Active: {getDisplayString(preferredModel)} ·{' '}
        {formatTokenCountShort(effectiveContextTokens)} ctx · thinking:{' '}
        {activeThinkingEffort}
      </Text>

      {view === 'custom' ? (
        <Box flexDirection="column" marginTop={1}>
          <Text color={theme.text.primary}>
            Enter NVIDIA NIM model identifier:
          </Text>
          <Box
            marginTop={1}
            borderStyle="round"
            borderColor={theme.ui.focus}
            paddingX={1}
          >
            <TextInput
              buffer={customBuffer}
              onSubmit={applyModelSelection}
              onCancel={() => setView('main')}
              placeholder={DEFAULT_M31A_MODEL}
            />
          </Box>
        </Box>
      ) : (
        <Box marginTop={1}>
          <DescriptiveRadioButtonSelect
            items={options}
            onSelect={handleSelect}
            initialIndex={initialIndex}
            showNumbers={true}
          />
        </Box>
      )}

      <Box marginTop={1} flexDirection="column">
        <Box>
          <Text bold color={theme.text.primary}>
            Remember model for future sessions:{' '}
          </Text>
          <Text color={theme.status.success}>
            {persistMode ? 'true' : 'false'}
          </Text>
          <Text color={theme.text.secondary}> (Press Tab to toggle)</Text>
        </Box>
      </Box>
      <Box flexDirection="column">
        <Text color={theme.text.secondary}>
          {'> To use a specific model on startup, use the --model flag.'}
        </Text>
      </Box>
      <Box marginTop={1} flexDirection="column">
        <Text color={theme.text.secondary}>
          {view === 'main'
            ? '(Press Esc to close)'
            : '(Press Enter to apply, Esc to go back)'}
        </Text>
      </Box>
    </Box>
  );
}
