/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  FinishReason,
  ThinkingLevel,
  type GenerateContentResponseUsageMetadata,
  type ThinkingConfig,
} from '@m31a/genai';

export type NimThinkingEffort =
  | 'default'
  | 'off'
  | 'minimal'
  | 'low'
  | 'medium'
  | 'high';

export const NIM_THINKING_EFFORT_OPTIONS: readonly NimThinkingEffort[] = [
  'default',
  'off',
  'minimal',
  'low',
  'medium',
  'high',
] as const;

export const NIM_CONTEXT_WINDOW_PRESETS: ReadonlyArray<{
  readonly label: string;
  readonly tokens: number | undefined;
}> = [
  { label: 'Auto (model catalog default)', tokens: undefined },
  { label: '32K (32,768 tokens)', tokens: 32_768 },
  { label: '64K (65,536 tokens)', tokens: 65_536 },
  { label: '128K (131,072 tokens)', tokens: 131_072 },
  { label: '256K (262,144 tokens)', tokens: 262_144 },
  { label: '512K (524,288 tokens)', tokens: 524_288 },
  { label: '1M (1,048,576 tokens)', tokens: 1_048_576 },
];

export interface NimCatalogModelEntry {
  readonly id: string;
  readonly name: string;
  readonly family: string;
  readonly contextWindow: number;
  readonly maxOutputTokens: number;
  readonly supportsTools: boolean;
  readonly thinkingMode: 'none' | 'toggle' | 'effort' | 'budget';
  readonly effortValues?: readonly NimThinkingEffort[];
  readonly modalities: readonly string[];
}

/**
 * Curated NVIDIA NIM chat model catalog aligned with OpenCode's models.dev snapshot.
 * Default model `nvidia/nemotron-3-ultra-550b-a55b` is kept first.
 */
export const NIM_MODEL_CATALOG: readonly NimCatalogModelEntry[] = [
  {
    id: 'nvidia/nemotron-3-ultra-550b-a55b',
    name: 'Nemotron 3 Ultra 550B A55B',
    family: 'nemotron-ultra',
    contextWindow: 1_048_576,
    maxOutputTokens: 65_536,
    supportsTools: true,
    thinkingMode: 'budget',
    effortValues: ['off', 'medium', 'high'],
    modalities: ['text'],
  },
  {
    id: 'nvidia/nemotron-3.5-super-vl-120b-a12b',
    name: 'Nemotron 3.5 Super VL 120B A12B',
    family: 'nemotron-super',
    contextWindow: 1_048_576,
    maxOutputTokens: 8_192,
    supportsTools: true,
    thinkingMode: 'toggle',
    modalities: ['text', 'image', 'video'],
  },
  {
    id: 'nvidia/nemotron-3.5-lightning-30b-a3b',
    name: 'Nemotron 3.5 Lightning 30B A3B',
    family: 'nemotron-lightning',
    contextWindow: 262_144,
    maxOutputTokens: 262_144,
    supportsTools: true,
    thinkingMode: 'toggle',
    modalities: ['text'],
  },
  {
    id: 'nvidia/nemotron-3-nano-omni-30b-a3b-reasoning',
    name: 'Nemotron 3 Nano Omni 30B',
    family: 'nemotron-nano',
    contextWindow: 256_000,
    maxOutputTokens: 65_536,
    supportsTools: true,
    thinkingMode: 'budget',
    modalities: ['text', 'image', 'video', 'audio'],
  },
  {
    id: 'deepseek-ai/deepseek-v4.1-flash',
    name: 'DeepSeek V4.1 Flash (NIM)',
    family: 'deepseek',
    contextWindow: 1_000_000,
    maxOutputTokens: 384_000,
    supportsTools: true,
    thinkingMode: 'effort',
    effortValues: ['off', 'low', 'high'],
    modalities: ['text', 'image'],
  },
  {
    id: 'z-ai/glm-5.3-flash',
    name: 'GLM-5.3-Flash (NIM)',
    family: 'glm',
    contextWindow: 1_000_000,
    maxOutputTokens: 131_072,
    supportsTools: true,
    thinkingMode: 'effort',
    effortValues: ['low', 'high'],
    modalities: ['text', 'image'],
  },
  {
    id: 'moonshotai/kimi-k2.6',
    name: 'Kimi K2.6 (NIM)',
    family: 'kimi',
    contextWindow: 262_144,
    maxOutputTokens: 262_144,
    supportsTools: true,
    thinkingMode: 'toggle',
    modalities: ['text', 'image', 'video'],
  },
  {
    id: 'minimaxai/minimax-m3.1-flash',
    name: 'MiniMax M3.1 Flash (NIM)',
    family: 'minimax',
    contextWindow: 262_144,
    maxOutputTokens: 262_144,
    supportsTools: true,
    thinkingMode: 'toggle',
    modalities: ['text', 'image', 'video'],
  },
  {
    id: 'stepfun-ai/step-5-preview',
    name: 'Step 5 Preview (NIM)',
    family: 'step',
    contextWindow: 1_048_576,
    maxOutputTokens: 1_048_576,
    supportsTools: true,
    thinkingMode: 'effort',
    effortValues: ['off', 'minimal', 'low', 'medium', 'high'],
    modalities: ['text'],
  },
  {
    id: 'google/gemma-4-31b-it',
    name: 'Gemma 4 31B IT (NIM)',
    family: 'gemma',
    contextWindow: 256_000,
    maxOutputTokens: 16_384,
    supportsTools: true,
    thinkingMode: 'toggle',
    modalities: ['text', 'image'],
  },
];

export interface NimModelCompatibility {
  readonly reasoningField?:
    | 'reasoning_content'
    | 'reasoning'
    | 'reasoning_text';
  readonly requireReasoning: boolean;
  readonly toolCallIdFormat: 'default' | 'mistral9' | 'claude';
  readonly requireAssistantAfterTool: boolean;
  readonly supportsClearThinkingFalse: boolean;
  readonly maxReasoningBudget?: number;
}

export interface NimThinkingBodyPatch {
  readonly reasoningEffort?: string;
  readonly reasoningBudget?: number;
  readonly chatTemplateKwargs?: Record<string, unknown>;
}

export interface NimModelTokenLimits {
  readonly context: number;
  readonly output?: number;
}

export const NIM_REASONING_FIELDS = [
  'reasoning_content',
  'reasoning',
  'reasoning_text',
] as const;

const MISTRAL_FAMILIES = [
  'mistral',
  'devstral',
  'codestral',
  'pixtral',
  'mixtral',
] as const;

const EXTRA_NIM_TOKEN_LIMITS: ReadonlyArray<{
  readonly match: string;
  readonly limits: NimModelTokenLimits;
}> = [
  {
    match: 'nemotron-3.5-content-safety',
    limits: { context: 128_000, output: 8_192 },
  },
];

export function formatTokenCountShort(tokens: number): string {
  if (tokens >= 1_000_000) {
    const millions = Number((tokens / 1_000_000).toFixed(1));
    return `${millions === 1 ? 1 : millions}M`;
  }
  if (tokens >= 1_000) {
    return `${Math.round(tokens / 1_000)}K`;
  }
  return String(tokens);
}

export function getNimCatalogEntry(
  modelId: string,
): NimCatalogModelEntry | undefined {
  const lower = modelId.toLowerCase();
  return NIM_MODEL_CATALOG.find(
    (entry) =>
      entry.id.toLowerCase() === lower ||
      lower.includes(entry.id.toLowerCase()),
  );
}

export function getNimModelTokenLimits(
  modelId: string,
): NimModelTokenLimits | undefined {
  const lower = modelId.toLowerCase();
  const catalog = NIM_MODEL_CATALOG.find((entry) => {
    const shortName = entry.id.split('/').pop() ?? entry.id;
    return lower.includes(shortName.toLowerCase());
  });
  if (catalog) {
    return {
      context: catalog.contextWindow,
      output: catalog.maxOutputTokens,
    };
  }
  for (const entry of EXTRA_NIM_TOKEN_LIMITS) {
    if (lower.includes(entry.match)) {
      return entry.limits;
    }
  }
  return undefined;
}

export function thinkingEffortToThinkingConfig(
  effort: NimThinkingEffort,
  modelId: string,
): ThinkingConfig | undefined {
  if (effort === 'default') {
    return undefined;
  }
  if (effort === 'off') {
    return {
      includeThoughts: false,
      thinkingBudget: 0,
    };
  }
  const compat = getNimModelCompatibility(modelId);
  const levelMap: Record<
    Exclude<NimThinkingEffort, 'default' | 'off'>,
    ThinkingLevel
  > = {
    minimal: ThinkingLevel.MINIMAL,
    low: ThinkingLevel.LOW,
    medium: ThinkingLevel.MEDIUM,
    high: ThinkingLevel.HIGH,
  };
  return {
    includeThoughts: true,
    thinkingLevel: levelMap[effort],
    ...(compat.maxReasoningBudget
      ? {
          thinkingBudget:
            effort === 'high'
              ? compat.maxReasoningBudget
              : effort === 'medium'
                ? Math.min(16_384, compat.maxReasoningBudget)
                : 4_096,
        }
      : {}),
  };
}

/**
 * Derives NVIDIA NIM compatibility behavior per hosted model family, matching
 * OpenCode's `nvidiaChat` and `openai-chat` rules.
 */
export function getNimModelCompatibility(
  modelId: string,
): NimModelCompatibility {
  const lower = modelId.toLowerCase();
  const isMistral = MISTRAL_FAMILIES.some((family) => lower.includes(family));
  const isClaude = lower.includes('claude');
  const isDeepSeek = lower.includes('deepseek');
  const isKimi = lower.includes('kimi');
  const isGlm = lower.includes('glm');
  const isMinimax = lower.includes('minimax');
  const isNemotronSuperOrSafety =
    lower.includes('nemotron-3.5-super') ||
    lower.includes('nemotron-3.5-content-safety');
  const isStep = lower.includes('step-5');
  const isMuse = lower.includes('muse-glimmer');

  const requiresReasoningField =
    isDeepSeek ||
    isKimi ||
    isGlm ||
    isMinimax ||
    isNemotronSuperOrSafety ||
    isStep ||
    isMuse;

  const isNemotronWithBudget =
    lower.includes('nemotron-3-ultra') ||
    lower.includes('nemotron-3-nano-omni');

  return {
    reasoningField: requiresReasoningField ? 'reasoning_content' : undefined,
    requireReasoning: requiresReasoningField,
    toolCallIdFormat: isMistral ? 'mistral9' : isClaude ? 'claude' : 'default',
    requireAssistantAfterTool: isMistral,
    supportsClearThinkingFalse: isGlm,
    maxReasoningBudget: isNemotronWithBudget ? 32_768 : undefined,
  };
}

/**
 * Maps tool call IDs safely for NIM-hosted models with strict ID format rules
 * (e.g., Mistral 9-char alphanumeric, Claude [a-zA-Z0-9_-]).
 */
export function normalizeNimToolCallId(
  id: string,
  format: NimModelCompatibility['toolCallIdFormat'],
): string {
  if (format === 'mistral9') {
    return id
      .replace(/[^a-zA-Z0-9]/g, '')
      .slice(0, 9)
      .padEnd(9, '0');
  }
  if (format === 'claude') {
    return id.replace(/[^a-zA-Z0-9_-]/g, '_');
  }
  return id;
}

function thinkingLevelToEffort(
  level: ThinkingLevel | undefined,
): string | undefined {
  switch (level) {
    case ThinkingLevel.MINIMAL:
      return 'minimal';
    case ThinkingLevel.LOW:
      return 'low';
    case ThinkingLevel.MEDIUM:
      return 'medium';
    case ThinkingLevel.HIGH:
      return 'high';
    default:
      return undefined;
  }
}

/**
 * Translates `@m31a/genai` `ThinkingConfig` into NVIDIA NIM's per-family
 * `chat_template_kwargs` / `reasoning_budget` / `reasoning_effort` request fields.
 */
export function resolveNimThinkingBodyPatch(
  modelId: string,
  thinkingConfig: ThinkingConfig | undefined,
): NimThinkingBodyPatch | undefined {
  if (!thinkingConfig) {
    return undefined;
  }

  const lower = modelId.toLowerCase();
  const compat = getNimModelCompatibility(modelId);
  const { includeThoughts, thinkingBudget, thinkingLevel } = thinkingConfig;
  const effort = thinkingLevelToEffort(thinkingLevel);

  const isExplicitOff =
    thinkingBudget === 0 ||
    (includeThoughts === false &&
      thinkingBudget === undefined &&
      effort === undefined);

  if (lower.includes('deepseek-v4')) {
    if (isExplicitOff) {
      return { chatTemplateKwargs: { thinking: false } };
    }
    if (effort) {
      return {
        chatTemplateKwargs: {
          thinking: true,
          reasoning_effort: effort,
        },
      };
    }
    if (includeThoughts === true || (thinkingBudget ?? 0) !== 0) {
      return { chatTemplateKwargs: { thinking: true } };
    }
    return undefined;
  }

  if (lower.includes('kimi-k2.6') || lower.includes('kimi-k2-6')) {
    if (isExplicitOff) {
      return { chatTemplateKwargs: { thinking: false } };
    }
    if (includeThoughts === true || effort || (thinkingBudget ?? 0) !== 0) {
      return { chatTemplateKwargs: { thinking: true } };
    }
    return undefined;
  }

  if (lower.includes('kimi-k3')) {
    if (isExplicitOff) {
      return { chatTemplateKwargs: { thinking: false } };
    }
    if (effort) {
      return { reasoningEffort: effort };
    }
    if (includeThoughts === true || (thinkingBudget ?? 0) !== 0) {
      return { chatTemplateKwargs: { thinking: true } };
    }
    return undefined;
  }

  if (lower.includes('minimax-m3')) {
    if (isExplicitOff) {
      return { chatTemplateKwargs: { thinking_mode: 'disabled' } };
    }
    if (includeThoughts === true || effort || (thinkingBudget ?? 0) !== 0) {
      return { chatTemplateKwargs: { thinking_mode: 'enabled' } };
    }
    return undefined;
  }

  if (lower.includes('glm')) {
    if (isExplicitOff) {
      return { chatTemplateKwargs: { enable_thinking: false } };
    }
    if (includeThoughts === true || effort || (thinkingBudget ?? 0) !== 0) {
      return {
        chatTemplateKwargs: {
          enable_thinking: true,
          clear_thinking: false,
        },
      };
    }
    return undefined;
  }

  // Nemotron / Qwen / Gemma / DeepSeek-R1 / default NIM chat templates
  const isNemotronOrQwenOrGemma =
    lower.includes('nemotron') ||
    lower.includes('qwen') ||
    lower.includes('gemma');

  if (isExplicitOff) {
    if (lower.includes('nemotron-3-ultra')) {
      return {
        reasoningEffort: 'none',
        chatTemplateKwargs: { enable_thinking: false },
      };
    }
    return { chatTemplateKwargs: { enable_thinking: false } };
  }

  if (thinkingBudget !== undefined && thinkingBudget > 0) {
    const cappedBudget = compat.maxReasoningBudget
      ? Math.min(thinkingBudget, compat.maxReasoningBudget)
      : thinkingBudget;
    return {
      chatTemplateKwargs: { enable_thinking: true },
      ...(compat.maxReasoningBudget ? { reasoningBudget: cappedBudget } : {}),
      ...(effort ? { reasoningEffort: effort } : {}),
    };
  }

  if (effort) {
    if (lower.includes('nemotron-3-ultra')) {
      const normalizedEffort = effort === 'minimal' ? 'low' : effort;
      return {
        reasoningEffort: normalizedEffort,
        chatTemplateKwargs: { enable_thinking: true },
      };
    }
    return isNemotronOrQwenOrGemma
      ? {
          chatTemplateKwargs: { enable_thinking: true },
          reasoningEffort: effort,
        }
      : { reasoningEffort: effort };
  }

  if (includeThoughts === true || thinkingBudget === -1) {
    return { chatTemplateKwargs: { enable_thinking: true } };
  }

  return undefined;
}

/**
 * Extracts scalar reasoning text across OpenAI-compatible delta/message fields
 * (`reasoning_content`, `reasoning`, `reasoning_text`), matching OpenCode's priority order.
 */
export function extractNimReasoningText(
  record: Record<string, unknown>,
  preferredField?: string,
): { field: string; text: string } | undefined {
  const candidates = preferredField
    ? [preferredField, ...NIM_REASONING_FIELDS]
    : [...NIM_REASONING_FIELDS];
  const seen = new Set<string>();
  for (const field of candidates) {
    if (seen.has(field)) continue;
    seen.add(field);
    const val = record[field];
    if (typeof val === 'string' && val.length > 0) {
      return { field, text: val };
    }
  }
  return undefined;
}

export function mapNimFinishReason(
  reason: string | null | undefined,
): FinishReason {
  switch (reason) {
    case 'stop':
    case 'end':
    case 'tool_calls':
    case 'function_call':
      return FinishReason.STOP;
    case 'length':
    case 'model_context_window_exceeded':
      return FinishReason.MAX_TOKENS;
    case 'content_filter':
    case 'sensitive':
      return FinishReason.SAFETY;
    default:
      return FinishReason.STOP;
  }
}

function asFiniteNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value)
    ? value
    : undefined;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Maps OpenAI/NIM `usage` objects (including `prompt_tokens_details.cached_tokens`,
 * `prompt_cache_hit_tokens`, and `completion_tokens_details.reasoning_tokens`)
 * into M31A `GenerateContentResponseUsageMetadata`.
 */
export function mapNimUsageMetadata(
  rawUsage: Record<string, unknown> | undefined,
): GenerateContentResponseUsageMetadata | undefined {
  if (!rawUsage) {
    return undefined;
  }
  const promptTokens = asFiniteNumber(rawUsage['prompt_tokens']);
  const completionTokens = asFiniteNumber(rawUsage['completion_tokens']);
  const totalTokens = asFiniteNumber(rawUsage['total_tokens']);

  const promptDetails = isPlainObject(rawUsage['prompt_tokens_details'])
    ? rawUsage['prompt_tokens_details']
    : undefined;
  const completionDetails = isPlainObject(rawUsage['completion_tokens_details'])
    ? rawUsage['completion_tokens_details']
    : undefined;

  const cachedTokens =
    asFiniteNumber(promptDetails?.['cached_tokens']) ??
    asFiniteNumber(rawUsage['prompt_cache_hit_tokens']) ??
    asFiniteNumber(rawUsage['cached_tokens']) ??
    asFiniteNumber(rawUsage['cache_read_input_tokens']);

  const reasoningTokens =
    asFiniteNumber(completionDetails?.['reasoning_tokens']) ??
    asFiniteNumber(rawUsage['reasoning_tokens']);

  const outputTokens =
    completionTokens !== undefined &&
    reasoningTokens !== undefined &&
    promptTokens !== undefined &&
    totalTokens === promptTokens + completionTokens + reasoningTokens
      ? completionTokens + reasoningTokens
      : completionTokens;

  return {
    promptTokenCount: promptTokens ?? 0,
    candidatesTokenCount: outputTokens ?? 0,
    totalTokenCount: totalTokens ?? (promptTokens ?? 0) + (outputTokens ?? 0),
    ...(cachedTokens !== undefined
      ? { cachedContentTokenCount: cachedTokens }
      : {}),
    ...(reasoningTokens !== undefined
      ? { thoughtsTokenCount: reasoningTokens }
      : {}),
  };
}
