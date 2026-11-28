/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * NVIDIA NIM content generator.
 *
 * M31A talks to NVIDIA NIM (https://docs.nvidia.com/nim/) through its
 * OpenAI-compatible HTTP API. No Google model, auth, or endpoint is used.
 *
 * Endpoint:  POST {baseUrl}/chat/completions   (OpenAI-compatible)
 * Auth:      Authorization: Bearer ${NVIDIA_API_KEY}
 * Default base URL: https://integrate.api.nvidia.com/v1
 * Default model:    nvidia/nemotron-3-ultra-550b-a55b
 *
 * Aligns with OpenCode's NVIDIA NIM provider quirks:
 * - Uses `max_tokens` (never `max_completion_tokens`) and omits `strict` / `store`.
 * - Maps ThinkingConfig to per-family `chat_template_kwargs`, `reasoning_budget`,
 *   and `reasoning_effort` parameters.
 * - Extracts `reasoning_content` / `reasoning` / `reasoning_text` into `thought: true`
 *   parts and replays required reasoning fields across multi-turn tool calls.
 * - Reconstructs sparse streaming `tool_calls` deltas without `index` and emits a
 *   terminal chunk carrying `finishReason` + trailing `stream_options` usage metadata.
 */

import {
  EmbedContentResponse,
  type FinishReason,
  GenerateContentResponse,
  type GenerateContentResponseUsageMetadata,
  Type,
  type CallableTool,
  type Content,
  type ContentUnion,
  type CountTokensParameters,
  type CountTokensResponse,
  type EmbedContentParameters,
  type FunctionCall,
  type GenerateContentParameters,
  type Part,
  type Schema,
  type Tool,
} from '@m31a/genai';
import type { ContentGenerator } from './contentGenerator.js';
import type { LlmRole } from '../telemetry/llmRole.js';
import {
  extractNimReasoningText,
  getNimModelCompatibility,
  mapNimFinishReason,
  mapNimUsageMetadata,
  normalizeNimToolCallId,
  resolveNimThinkingBodyPatch,
  type NimModelCompatibility,
} from './nvidiaNimProtocolHelpers.js';

export const NVIDIA_NIM_DEFAULT_BASE_URL =
  'https://integrate.api.nvidia.com/v1';
export const NVIDIA_NIM_DEFAULT_MODEL = 'nvidia/nemotron-3-ultra-550b-a55b';
export const NVIDIA_NIM_API_KEY_ENV = 'NVIDIA_API_KEY';
export const NVIDIA_NIM_BASE_URL_ENV = 'NVIDIA_NIM_BASE_URL';
export const NVIDIA_NIM_MODEL_ENV = 'NVIDIA_NIM_MODEL';
export const NVIDIA_NIM_EMBED_MODEL_ENV = 'NVIDIA_NIM_EMBED_MODEL';
export const NVIDIA_NIM_DEFAULT_EMBED_MODEL = 'nvidia/nv-embed-v1';

export interface NvidiaNimContentGeneratorConfig {
  apiKey?: string;
  baseUrl?: string;
  model?: string;
  embedModel?: string;
  customHeaders?: Record<string, string>;
  proxy?: string;
  /** Injected fetch for testing. */
  fetchImpl?: typeof fetch;
}

// ---------- Gemini <-> OpenAI-compatible (NIM) mapping ----------

type OpenAIContentPart =
  | { type: 'text'; text: string }
  | { type: 'image_url'; image_url: { url: string } };

interface OpenAIToolCall {
  id: string;
  type: 'function';
  function: { name: string; arguments: string };
}

type OpenAIMessage =
  | { role: 'system'; content: string }
  | { role: 'user'; content: string | OpenAIContentPart[] }
  | {
      role: 'assistant';
      content: string | null;
      tool_calls?: OpenAIToolCall[];
      reasoning_content?: string;
      reasoning?: string;
      reasoning_text?: string;
    }
  | { role: 'tool'; tool_call_id: string; content: string };

interface OpenAITool {
  type: 'function';
  function: {
    name: string;
    description?: string;
    parameters?: Record<string, unknown>;
  };
}

function isContent(item: unknown): item is Content {
  return typeof item === 'object' && item !== null && 'parts' in item;
}

function isPart(item: unknown): item is Part {
  return typeof item === 'object' && item !== null && !('parts' in item);
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' ? value : undefined;
}

function isOpenAIToolCall(value: unknown): value is OpenAIToolCall {
  if (!isPlainRecord(value)) return false;
  if (!asString(value['id']) || value['type'] !== 'function') {
    return false;
  }
  const fn = value['function'];
  return (
    isPlainRecord(fn) &&
    asString(fn['name']) !== undefined &&
    asString(fn['arguments']) !== undefined
  );
}

function contentUnionToContents(
  union: ContentUnion | Content[] | undefined,
): Content[] {
  if (!union) return [];
  if (typeof union === 'string') {
    return [{ role: 'user', parts: [{ text: union }] }];
  }
  if (Array.isArray(union)) {
    const contents: Content[] = [];
    const looseParts: Part[] = [];
    for (const item of union) {
      if (typeof item === 'string') {
        looseParts.push({ text: item });
      } else if (isContent(item)) {
        contents.push(item);
      } else if (isPart(item)) {
        looseParts.push(item);
      }
    }
    if (looseParts.length > 0) {
      contents.push({ role: 'user', parts: looseParts });
    }
    return contents;
  }
  if (isContent(union)) return [union];
  if (isPart(union)) return [{ role: 'user', parts: [union] }];
  return [];
}

function normalizeSchemaType(type: unknown): string | undefined {
  if (!type) return undefined;
  const t = String(type).toLowerCase();
  // Gemini uses Type.OBJECT etc.; JSON Schema uses lowercase.
  switch (t) {
    case 'object':
    case 'array':
    case 'string':
    case 'number':
    case 'integer':
    case 'boolean':
    case 'null':
      return t;
    default:
      return undefined;
  }
}

function geminiSchemaToJsonSchema(
  schema: Schema | undefined,
): Record<string, unknown> | undefined {
  if (!schema) return undefined;
  const out: Record<string, unknown> = {};
  const rawType = 'type' in schema ? schema.type : undefined;
  const jsonType = normalizeSchemaType(rawType ?? Type.OBJECT);
  const isNullable = Boolean('nullable' in schema && schema.nullable);

  if (jsonType) {
    out['type'] =
      isNullable && jsonType !== 'null' ? [jsonType, 'null'] : jsonType;
  }
  if (schema.description) out['description'] = schema.description;
  if (schema.enum) out['enum'] = schema.enum;
  if (schema.required) out['required'] = schema.required;
  if (schema.format) out['format'] = schema.format;
  if (schema.minimum !== undefined) out['minimum'] = schema.minimum;
  if (schema.maximum !== undefined) out['maximum'] = schema.maximum;
  if (schema.items) out['items'] = geminiSchemaToJsonSchema(schema.items) ?? {};
  if (schema.properties) {
    const props: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(schema.properties)) {
      props[k] = geminiSchemaToJsonSchema(v) ?? {};
    }
    out['properties'] = props;
  }
  if (schema.anyOf) {
    out['anyOf'] = schema.anyOf.map((s) => geminiSchemaToJsonSchema(s) ?? {});
  }
  return out;
}

function geminiToolsToOpenAI(
  tools: Array<Tool | CallableTool> | undefined,
): OpenAITool[] | undefined {
  if (!tools || tools.length === 0) return undefined;
  const out: OpenAITool[] = [];
  for (const tool of tools) {
    if (!('functionDeclarations' in tool)) continue;
    for (const fn of tool.functionDeclarations ?? []) {
      out.push({
        type: 'function',
        function: {
          name: fn.name ?? '',
          description: fn.description,
          // Omit OpenAI `strict` so NIM and OpenAI-compatible endpoints do not 400.
          parameters: geminiSchemaToJsonSchema(fn.parameters) ?? {
            type: 'object',
            properties: {},
          },
        },
      });
    }
  }
  return out.length ? out : undefined;
}

function partToOpenAIContent(part: Part): OpenAIContentPart | null {
  if (typeof part === 'string') return { type: 'text', text: part };
  if ('text' in part && part.text != null)
    return { type: 'text', text: part.text };
  if ('inlineData' in part && part.inlineData?.data) {
    const mime = part.inlineData.mimeType ?? 'image/png';
    return {
      type: 'image_url',
      image_url: { url: `data:${mime};base64,${part.inlineData.data}` },
    };
  }
  if ('fileData' in part && part.fileData?.fileUri) {
    return { type: 'image_url', image_url: { url: part.fileData.fileUri } };
  }
  return null;
}

function hasToolHistory(messages: readonly OpenAIMessage[]): boolean {
  return messages.some(
    (msg) =>
      msg.role === 'tool' ||
      (msg.role === 'assistant' &&
        Array.isArray(msg.tool_calls) &&
        msg.tool_calls.length > 0),
  );
}

let toolCallCounter = 0;

function contentsToOpenAIMessages(
  contents: Content[],
  compat: NimModelCompatibility,
): OpenAIMessage[] {
  const messages: OpenAIMessage[] = [];
  const bridgeMistralTools = () => {
    if (
      compat.requireAssistantAfterTool &&
      messages[messages.length - 1]?.role === 'tool'
    ) {
      messages.push({ role: 'assistant', content: 'Done.' });
    }
  };

  for (const content of contents) {
    if (content.role === 'system') {
      const sysText = (content.parts ?? [])
        .map((p) =>
          typeof p === 'string' ? p : 'text' in p ? (p.text ?? '') : '',
        )
        .filter(Boolean)
        .join('\n');
      if (sysText) {
        bridgeMistralTools();
        const wrappedText = `<system-reminder>\n${sysText}\n</system-reminder>`;
        const prev = messages[messages.length - 1];
        if (prev?.role === 'user' && typeof prev.content === 'string') {
          messages[messages.length - 1] = {
            role: 'user',
            content: `${prev.content}\n${wrappedText}`,
          };
        } else if (prev?.role === 'user' && Array.isArray(prev.content)) {
          messages[messages.length - 1] = {
            role: 'user',
            content: [...prev.content, { type: 'text', text: wrappedText }],
          };
        } else {
          messages.push({ role: 'user', content: wrappedText });
        }
      }
      continue;
    }

    const role = content.role === 'model' ? 'assistant' : 'user';
    const parts = content.parts ?? [];
    const textParts: OpenAIContentPart[] = [];
    const reasoningParts: string[] = [];
    const toolCalls: OpenAIToolCall[] = [];
    const toolResults: OpenAIMessage[] = [];

    for (const part of parts) {
      if (typeof part === 'string') {
        textParts.push({ type: 'text', text: part });
        continue;
      }
      if ('thought' in part && part.thought) {
        if ('text' in part && typeof part.text === 'string' && part.text) {
          reasoningParts.push(part.text);
        }
        continue;
      }
      if ('functionCall' in part && part.functionCall) {
        const fc: FunctionCall = part.functionCall;
        const rawId = fc.id ?? `call_${++toolCallCounter}`;
        toolCalls.push({
          id: normalizeNimToolCallId(rawId, compat.toolCallIdFormat),
          type: 'function',
          function: {
            name: fc.name ?? '',
            arguments: JSON.stringify(fc.args ?? {}),
          },
        });
        continue;
      }
      if ('functionResponse' in part && part.functionResponse) {
        const fr = part.functionResponse;
        const rawId = fr.id ?? fr.name ?? `call_${toolCallCounter}`;
        toolResults.push({
          role: 'tool',
          tool_call_id: normalizeNimToolCallId(rawId, compat.toolCallIdFormat),
          content: JSON.stringify(fr.response ?? {}),
        });
        continue;
      }
      const c = partToOpenAIContent(part);
      if (c) textParts.push(c);
    }

    if (role === 'assistant') {
      const text = textParts
        .filter((p): p is { type: 'text'; text: string } => p.type === 'text')
        .map((p) => p.text)
        .join('');
      const reasoningText = reasoningParts.join('');
      const hasVisibleText = text.trim().length > 0;
      const hasTools = toolCalls.length > 0;
      const hasReasoning = reasoningText.length > 0;

      if (hasVisibleText || hasTools || hasReasoning) {
        const assistantMsg: OpenAIMessage = {
          role: 'assistant',
          content: text.length > 0 ? text : hasTools ? null : '',
          ...(hasTools ? { tool_calls: toolCalls } : {}),
        };
        const replayField = compat.reasoningField ?? 'reasoning_content';
        if (hasReasoning) {
          assistantMsg[replayField] = reasoningText;
        } else if (compat.requireReasoning && hasTools) {
          assistantMsg[replayField] = '';
        }
        messages.push(assistantMsg);
      }
    } else if (textParts.length) {
      bridgeMistralTools();
      const firstPart = textParts[0];
      messages.push({
        role: 'user',
        content:
          textParts.length === 1 && firstPart?.type === 'text'
            ? firstPart.text
            : textParts,
      });
    }
    // Tool results must follow the assistant tool_calls message.
    messages.push(...toolResults);
  }
  return messages;
}

function parseToolArguments(raw: string): Record<string, unknown> {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (isPlainRecord(parsed)) {
      return Object.fromEntries(Object.entries(parsed));
    }
    return { _raw: raw };
  } catch {
    return { _raw: raw };
  }
}

function nimMessageToGeminiResponse(
  message: {
    content?: string | null;
    reasoningText?: string;
    tool_calls?: OpenAIToolCall[];
  },
  finishReason: string | null | undefined,
  usageMetadata?: GenerateContentResponseUsageMetadata,
): GenerateContentResponse {
  const parts: Part[] = [];
  if (message.reasoningText) {
    parts.push({ text: message.reasoningText, thought: true });
  }
  if (message.content) {
    parts.push({ text: message.content });
  }
  const functionCalls: FunctionCall[] = [];
  for (const tc of message.tool_calls ?? []) {
    const args = parseToolArguments(tc.function.arguments);
    const fc: FunctionCall = { id: tc.id, name: tc.function.name, args };
    functionCalls.push(fc);
    parts.push({ functionCall: fc });
  }
  if (parts.length === 0) {
    parts.push({ text: '' });
  }

  const mappedFinishReason: FinishReason | undefined = finishReason
    ? mapNimFinishReason(finishReason)
    : undefined;

  const response = new GenerateContentResponse();
  response.candidates = [
    {
      content: { role: 'model', parts },
      ...(mappedFinishReason ? { finishReason: mappedFinishReason } : {}),
      index: 0,
    },
  ];
  response.functionCalls = functionCalls.length ? functionCalls : undefined;
  response.text = message.content ?? '';
  response.usageMetadata = usageMetadata;
  return response;
}

// ---------- Generator ----------

export class NvidiaNimContentGenerator implements ContentGenerator {
  private readonly apiKey: string;
  private readonly baseUrl: string;
  private readonly model: string;
  private readonly embedModel: string;
  private readonly customHeaders: Record<string, string>;
  private readonly fetchImpl: typeof fetch;

  constructor(config: NvidiaNimContentGeneratorConfig = {}) {
    this.apiKey = config.apiKey ?? '';
    this.baseUrl = (config.baseUrl ?? NVIDIA_NIM_DEFAULT_BASE_URL).replace(
      /\/+$/,
      '',
    );
    this.model = config.model ?? NVIDIA_NIM_DEFAULT_MODEL;
    this.embedModel = config.embedModel ?? NVIDIA_NIM_DEFAULT_EMBED_MODEL;
    this.customHeaders = config.customHeaders ?? {};
    this.fetchImpl = config.fetchImpl ?? fetch;
  }

  private headers(): Record<string, string> {
    return {
      'Content-Type': 'application/json',
      ...(this.apiKey ? { Authorization: `Bearer ${this.apiKey}` } : {}),
      ...this.customHeaders,
    };
  }

  private resolveEffectiveModel(requestModel: string | undefined): string {
    if (requestModel && requestModel.includes('/')) {
      return requestModel;
    }
    return this.model;
  }

  private buildBody(request: GenerateContentParameters, stream: boolean) {
    const effectiveModel = this.resolveEffectiveModel(request.model);
    const compat = getNimModelCompatibility(effectiveModel);
    const contents = contentUnionToContents(request.contents);
    const messages: OpenAIMessage[] = [];
    const sys = request.config?.systemInstruction;
    if (sys) {
      const sysContents = contentUnionToContents(sys);
      const sysText = sysContents
        .flatMap((c) => c.parts ?? [])
        .map((p) =>
          typeof p === 'string' ? p : 'text' in p ? (p.text ?? '') : '',
        )
        .join('\n');
      if (sysText) messages.push({ role: 'system', content: sysText });
    }
    messages.push(...contentsToOpenAIMessages(contents, compat));

    const cfg = request.config ?? {};
    const tools = geminiToolsToOpenAI(cfg.tools);
    const includeEmptyTools = !tools && hasToolHistory(messages);
    const thinkingPatch = resolveNimThinkingBodyPatch(
      effectiveModel,
      cfg.thinkingConfig,
    );

    const body: Record<string, unknown> = {
      model: effectiveModel,
      messages,
      stream,
      ...(stream ? { stream_options: { include_usage: true } } : {}),
      ...(cfg.temperature != null ? { temperature: cfg.temperature } : {}),
      ...(cfg.topP != null ? { top_p: cfg.topP } : {}),
      ...(cfg.topK != null ? { top_k: cfg.topK } : {}),
      ...(cfg.maxOutputTokens != null
        ? { max_tokens: cfg.maxOutputTokens }
        : {}),
      ...(cfg.stopSequences?.length ? { stop: cfg.stopSequences } : {}),
      ...(cfg.seed != null ? { seed: cfg.seed } : {}),
      ...(tools
        ? { tools, tool_choice: 'auto' }
        : includeEmptyTools
          ? { tools: [] }
          : {}),
      ...(thinkingPatch?.reasoningEffort !== undefined
        ? { reasoning_effort: thinkingPatch.reasoningEffort }
        : {}),
      ...(thinkingPatch?.reasoningBudget !== undefined
        ? { reasoning_budget: thinkingPatch.reasoningBudget }
        : {}),
      ...(thinkingPatch?.chatTemplateKwargs
        ? { chat_template_kwargs: thinkingPatch.chatTemplateKwargs }
        : {}),
    };
    if (cfg.responseMimeType === 'application/json') {
      body['response_format'] = { type: 'json_object' };
    }
    return { body, compat };
  }

  async generateContent(
    request: GenerateContentParameters,
    _userPromptId: string,
    _role: LlmRole,
  ): Promise<GenerateContentResponse> {
    const { body, compat } = this.buildBody(request, false);
    const res = await this.fetchImpl(`${this.baseUrl}/chat/completions`, {
      method: 'POST',
      headers: this.headers(),
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const detail = await res.text().catch(() => '');
      throw new Error(`NVIDIA NIM error ${res.status}: ${detail}`);
    }
    const raw: unknown = await res.json();
    const json = isPlainRecord(raw) ? raw : {};
    if (isPlainRecord(json['error'])) {
      const msg =
        asString(json['error']['message']) ?? JSON.stringify(json['error']);
      throw new Error(`NVIDIA NIM error: ${msg}`);
    }
    const choices = Array.isArray(json['choices']) ? json['choices'] : [];
    const first = isPlainRecord(choices[0]) ? choices[0] : {};
    const rawMsg = isPlainRecord(first['message']) ? first['message'] : {};
    const rawToolCalls = Array.isArray(rawMsg['tool_calls'])
      ? rawMsg['tool_calls'].filter(isOpenAIToolCall)
      : undefined;
    const reasoning = extractNimReasoningText(rawMsg, compat.reasoningField);
    const message = {
      content: asString(rawMsg['content']) ?? null,
      reasoningText: reasoning?.text,
      tool_calls: rawToolCalls,
    };
    const finishReason =
      asString(first['finish_reason']) ??
      (rawToolCalls?.length ? 'tool_calls' : 'stop');
    const rawUsage = isPlainRecord(json['usage'])
      ? json['usage']
      : isPlainRecord(first['usage'])
        ? first['usage']
        : undefined;
    const usage = mapNimUsageMetadata(rawUsage);
    return nimMessageToGeminiResponse(message, finishReason, usage);
  }

  async generateContentStream(
    request: GenerateContentParameters,
    _userPromptId: string,
    _role: LlmRole,
  ): Promise<AsyncGenerator<GenerateContentResponse>> {
    const fetchImpl = this.fetchImpl;
    const url = `${this.baseUrl}/chat/completions`;
    const headers = this.headers();
    const { body, compat } = this.buildBody(request, true);
    async function* gen(): AsyncGenerator<GenerateContentResponse> {
      const res = await fetchImpl(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
      });
      if (!res.ok || !res.body) {
        const detail = await res.text().catch(() => '');
        throw new Error(`NVIDIA NIM stream error ${res.status}: ${detail}`);
      }
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';
      // Accumulate streaming tool-call deltas safely even if NIM omits `index`.
      const toolAcc = new Map<
        number,
        { id: string; name: string; args: string }
      >();
      let latestToolIndex: number | undefined;
      let nextToolIndex = 0;
      let finishReason: string | null = null;
      let latestUsage: GenerateContentResponseUsageMetadata | undefined;
      let observedReasoningField: string | undefined = compat.reasoningField;

      const findToolIndexById = (
        id: string | undefined,
      ): number | undefined => {
        if (!id) return undefined;
        for (const [slot, entry] of toolAcc.entries()) {
          if (entry.id === id) return slot;
        }
        return undefined;
      };

      const processLine = function* (
        rawLine: string,
      ): Generator<GenerateContentResponse> {
        const line = rawLine.trim();
        if (!line || !line.startsWith('data:')) return;
        const data = line.slice(5).trim();
        if (data === '[DONE]') return;
        let parsed: unknown;
        try {
          parsed = JSON.parse(data);
        } catch {
          return;
        }
        if (!isPlainRecord(parsed)) return;
        if (isPlainRecord(parsed['error'])) {
          const errMsg =
            asString(parsed['error']['message']) ??
            JSON.stringify(parsed['error']);
          throw new Error(`NVIDIA NIM stream error: ${errMsg}`);
        }
        const choices = Array.isArray(parsed['choices'])
          ? parsed['choices']
          : [];
        const choice = isPlainRecord(choices[0]) ? choices[0] : undefined;
        const rawUsage = isPlainRecord(parsed['usage'])
          ? parsed['usage']
          : choice && isPlainRecord(choice['usage'])
            ? choice['usage']
            : undefined;
        if (rawUsage) {
          latestUsage = mapNimUsageMetadata(rawUsage) ?? latestUsage;
        }
        if (!choice) return;

        const choiceFinishReason = asString(choice['finish_reason']);
        if (choiceFinishReason) {
          finishReason = choiceFinishReason;
        }

        const delta = isPlainRecord(choice['delta']) ? choice['delta'] : {};
        const reasoning = extractNimReasoningText(
          delta,
          observedReasoningField,
        );
        if (reasoning) {
          observedReasoningField = reasoning.field;
          yield nimMessageToGeminiResponse(
            { reasoningText: reasoning.text },
            null,
          );
        }

        const deltaContent = asString(delta['content']);
        if (deltaContent) {
          yield nimMessageToGeminiResponse({ content: deltaContent }, null);
        }

        const deltaToolCalls = Array.isArray(delta['tool_calls'])
          ? delta['tool_calls']
          : [];
        for (const [position, rawTc] of deltaToolCalls.entries()) {
          if (!isPlainRecord(rawTc)) continue;
          const tcId = asString(rawTc['id']);
          const matchedById = findToolIndexById(tcId);
          const explicitIndex = asNumber(rawTc['index']);
          const fallback =
            deltaToolCalls.length > 1
              ? position
              : (latestToolIndex ?? position);
          const existingFallback = toolAcc.get(fallback);
          const slot =
            explicitIndex ??
            matchedById ??
            (tcId && existingFallback?.id && existingFallback.id !== tcId
              ? nextToolIndex
              : fallback);

          const cur = toolAcc.get(slot) ?? { id: '', name: '', args: '' };
          if (tcId) {
            cur.id = tcId;
          }
          const fn = isPlainRecord(rawTc['function'])
            ? rawTc['function']
            : undefined;
          const fnName = fn ? asString(fn['name']) : undefined;
          if (fnName) {
            cur.name = fnName;
          }
          const fnArgs = fn ? asString(fn['arguments']) : undefined;
          if (fnArgs) {
            cur.args += fnArgs;
          }
          toolAcc.set(slot, cur);
          latestToolIndex = slot;
          nextToolIndex = Math.max(nextToolIndex, slot + 1);
        }
      };

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        let idx: number;
        while ((idx = buffer.indexOf('\n')) >= 0) {
          const line = buffer.slice(0, idx);
          buffer = buffer.slice(idx + 1);
          yield* processLine(line);
        }
      }
      if (buffer.trim()) {
        yield* processLine(buffer);
      }

      if (toolAcc.size > 0) {
        const toolCalls: OpenAIToolCall[] = [...toolAcc.entries()]
          .sort((a, b) => a[0] - b[0])
          .map(([, v]) => ({
            id: v.id || `call_${++toolCallCounter}`,
            type: 'function',
            function: { name: v.name, arguments: v.args || '{}' },
          }));
        yield nimMessageToGeminiResponse(
          { content: null, tool_calls: toolCalls },
          finishReason ?? 'tool_calls',
          latestUsage,
        );
      } else {
        yield nimMessageToGeminiResponse(
          { content: null },
          finishReason ?? 'stop',
          latestUsage,
        );
      }
    }
    return gen();
  }

  async countTokens(
    request: CountTokensParameters,
  ): Promise<CountTokensResponse> {
    // NIM does not expose a uniform token-count endpoint; estimate locally.
    const contents = contentUnionToContents(request.contents);
    const text = contents
      .flatMap((c) => c.parts ?? [])
      .map((p) =>
        typeof p === 'string'
          ? p
          : 'text' in p
            ? (p.text ?? '')
            : JSON.stringify(p),
      )
      .join(' ');
    const totalTokens = Math.ceil(text.length / 4);
    return { totalTokens };
  }

  async embedContent(
    request: EmbedContentParameters,
  ): Promise<EmbedContentResponse> {
    const contents = contentUnionToContents(request.contents);
    const input = contents
      .flatMap((c) => c.parts ?? [])
      .map((p) =>
        typeof p === 'string' ? p : 'text' in p ? (p.text ?? '') : '',
      )
      .join('\n');
    const cfg = request.config;
    const outputDimensionality = cfg
      ? asNumber(cfg['outputDimensionality'])
      : undefined;
    const inputType = cfg
      ? asString(cfg['inputType'] ?? cfg['input_type'])
      : undefined;
    const truncate = cfg ? asString(cfg['truncate']) : undefined;
    const encodingFormat = cfg
      ? asString(cfg['encodingFormat'] ?? cfg['encoding_format'])
      : undefined;
    const res = await this.fetchImpl(`${this.baseUrl}/embeddings`, {
      method: 'POST',
      headers: this.headers(),
      body: JSON.stringify({
        model:
          this.resolveEffectiveModel(request.model) === this.model &&
          !request.model
            ? this.embedModel
            : (request.model ?? this.embedModel),
        input,
        ...(outputDimensionality ? { dimensions: outputDimensionality } : {}),
        ...(inputType ? { input_type: inputType } : {}),
        ...(truncate ? { truncate } : {}),
        ...(encodingFormat ? { encoding_format: encodingFormat } : {}),
      }),
    });
    if (!res.ok) {
      const detail = await res.text().catch(() => '');
      throw new Error(`NVIDIA NIM embeddings error ${res.status}: ${detail}`);
    }
    const raw: unknown = await res.json();
    const json = isPlainRecord(raw) ? raw : {};
    const data = Array.isArray(json['data']) ? json['data'] : [];
    const first = isPlainRecord(data[0]) ? data[0] : {};
    const values = Array.isArray(first['embedding'])
      ? first['embedding'].filter((v): v is number => typeof v === 'number')
      : [];
    const resp = new EmbedContentResponse();
    resp.embeddings = [{ values }];
    return resp;
  }

  /** Queries `GET {baseUrl}/models` and returns sorted model IDs (empty on error). */
  async listModels(): Promise<string[]> {
    return fetchNimAvailableModels(this.baseUrl, this.apiKey, this.fetchImpl);
  }
}

/**
 * Fetches live available model IDs from NVIDIA NIM's OpenAI-compatible
 * `/models` endpoint, returning an empty list gracefully if offline/unauthenticated.
 */
export async function fetchNimAvailableModels(
  baseUrl: string = NVIDIA_NIM_DEFAULT_BASE_URL,
  apiKey: string = '',
  fetchImpl: typeof fetch = fetch,
): Promise<string[]> {
  const cleanUrl = baseUrl.replace(/\/+$/, '');
  try {
    const res = await fetchImpl(`${cleanUrl}/models`, {
      method: 'GET',
      headers: {
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      },
    });
    if (!res.ok) {
      return [];
    }
    const raw: unknown = await res.json();
    if (!isPlainRecord(raw) || !Array.isArray(raw['data'])) {
      return [];
    }
    const ids: string[] = [];
    for (const item of raw['data']) {
      if (isPlainRecord(item)) {
        const id = asString(item['id']);
        if (id) ids.push(id);
      }
    }
    return Array.from(new Set(ids)).sort();
  } catch {
    return [];
  }
}

/** Convenience factory reading config from the environment. */
export function createNvidiaNimContentGenerator(
  config: NvidiaNimContentGeneratorConfig = {},
): NvidiaNimContentGenerator {
  const env = process.env;
  return new NvidiaNimContentGenerator({
    apiKey: config.apiKey ?? env[NVIDIA_NIM_API_KEY_ENV] ?? '',
    baseUrl:
      config.baseUrl ??
      env[NVIDIA_NIM_BASE_URL_ENV] ??
      NVIDIA_NIM_DEFAULT_BASE_URL,
    model:
      config.model ?? env[NVIDIA_NIM_MODEL_ENV] ?? NVIDIA_NIM_DEFAULT_MODEL,
    embedModel:
      config.embedModel ??
      env[NVIDIA_NIM_EMBED_MODEL_ENV] ??
      NVIDIA_NIM_DEFAULT_EMBED_MODEL,
    customHeaders: config.customHeaders,
    proxy: config.proxy,
    fetchImpl: config.fetchImpl,
  });
}
