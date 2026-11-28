/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, expect, it, vi } from 'vitest';
import {
  FinishReason,
  ThinkingLevel,
  Type,
  type GenerateContentParameters,
} from '@m31a/genai';
import { LlmRole } from '../telemetry/llmRole.js';
import {
  NvidiaNimContentGenerator,
  fetchNimAvailableModels,
} from './nvidiaNimContentGenerator.js';

function createSseStream(events: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  return new ReadableStream({
    start(controller) {
      for (const chunk of events) {
        controller.enqueue(encoder.encode(chunk));
      }
      controller.close();
    },
  });
}

describe('NvidiaNimContentGenerator', () => {
  it('sends max_tokens, omits strict/store, strips propertyOrdering, and maps Nemotron thinking budget', async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          choices: [
            {
              message: {
                role: 'assistant',
                content: 'Answer text',
                reasoning_content: 'Step-by-step check',
              },
              finish_reason: 'stop',
            },
          ],
          usage: {
            prompt_tokens: 100,
            completion_tokens: 40,
            total_tokens: 140,
            prompt_tokens_details: { cached_tokens: 20 },
            completion_tokens_details: { reasoning_tokens: 15 },
          },
        }),
        { status: 200 },
      ),
    );

    const generator = new NvidiaNimContentGenerator({
      apiKey: 'nvapi-test',
      model: 'nvidia/nemotron-3-ultra-550b-a55b',
      fetchImpl: fetchMock as unknown as typeof fetch,
    });

    const req: GenerateContentParameters = {
      model: 'auto',
      contents: [{ role: 'user', parts: [{ text: 'hello' }] }],
      config: {
        maxOutputTokens: 2048,
        topK: 40,
        thinkingConfig: {
          includeThoughts: true,
          thinkingBudget: 50_000,
        },
        tools: [
          {
            functionDeclarations: [
              {
                name: 'read_file',
                description: 'Read a file',
                parameters: {
                  type: Type.OBJECT,
                  propertyOrdering: ['file_path'],
                  properties: {
                    file_path: { type: Type.STRING, nullable: true },
                  },
                  required: ['file_path'],
                },
              },
            ],
          },
        ],
      },
    };

    const resp = await generator.generateContent(req, 'p1', LlmRole.MAIN);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0];
    const sentBody = JSON.parse(String(init.body)) as Record<string, unknown>;

    expect(sentBody['model']).toBe('nvidia/nemotron-3-ultra-550b-a55b');
    expect(sentBody['max_tokens']).toBe(2048);
    expect(sentBody['top_k']).toBe(40);
    expect(sentBody).not.toHaveProperty('max_completion_tokens');
    expect(sentBody).not.toHaveProperty('store');
    expect(sentBody['chat_template_kwargs']).toEqual({ enable_thinking: true });
    // Capped at Nemotron 3 Ultra's catalog max (32,768).
    expect(sentBody['reasoning_budget']).toBe(32_768);

    const tools = sentBody['tools'] as Array<{
      type: string;
      function: {
        name: string;
        strict?: boolean;
        parameters: Record<string, unknown>;
      };
    }>;
    expect(tools[0]?.function.strict).toBeUndefined();
    expect(tools[0]?.function.parameters).not.toHaveProperty(
      'propertyOrdering',
    );
    expect(
      (tools[0]?.function.parameters['properties'] as Record<string, unknown>)[
        'file_path'
      ],
    ).toEqual({ type: ['string', 'null'] });

    expect(resp.candidates?.[0]?.content?.parts).toEqual([
      { text: 'Step-by-step check', thought: true },
      { text: 'Answer text' },
    ]);
    expect(resp.usageMetadata).toEqual({
      promptTokenCount: 100,
      candidatesTokenCount: 40,
      totalTokenCount: 140,
      cachedContentTokenCount: 20,
      thoughtsTokenCount: 15,
    });
  });

  it('maps family-specific thinking toggles/efforts for GLM, DeepSeek, MiniMax, Kimi, and Nemotron off', async () => {
    const captureBody = async (
      model: string,
      thinkingConfig: NonNullable<
        GenerateContentParameters['config']
      >['thinkingConfig'],
    ) => {
      const fetchMock = vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            choices: [{ message: { content: 'ok' }, finish_reason: 'stop' }],
          }),
          { status: 200 },
        ),
      );
      const gen = new NvidiaNimContentGenerator({
        model,
        fetchImpl: fetchMock as unknown as typeof fetch,
      });
      await gen.generateContent(
        {
          model,
          contents: [{ role: 'user', parts: [{ text: 'hi' }] }],
          config: { thinkingConfig },
        },
        'p2',
        LlmRole.MAIN,
      );
      const [, init] = fetchMock.mock.calls[0];
      return JSON.parse(String(init.body)) as Record<string, unknown>;
    };

    const ultraOff = await captureBody('nvidia/nemotron-3-ultra-550b-a55b', {
      thinkingBudget: 0,
    });
    expect(ultraOff['reasoning_effort']).toBe('none');
    expect(ultraOff['chat_template_kwargs']).toEqual({
      enable_thinking: false,
    });

    const glmOn = await captureBody('z-ai/glm-5.3-flash', {
      includeThoughts: true,
    });
    expect(glmOn['chat_template_kwargs']).toEqual({
      enable_thinking: true,
      clear_thinking: false,
    });

    const deepseekHigh = await captureBody('deepseek-ai/deepseek-v4.1-flash', {
      thinkingLevel: ThinkingLevel.HIGH,
    });
    expect(deepseekHigh['chat_template_kwargs']).toEqual({
      thinking: true,
      reasoning_effort: 'high',
    });

    const minimaxOff = await captureBody('minimaxai/minimax-m3.1-flash', {
      thinkingBudget: 0,
    });
    expect(minimaxOff['chat_template_kwargs']).toEqual({
      thinking_mode: 'disabled',
    });

    const kimiOn = await captureBody('moonshotai/kimi-k2.6', {
      includeThoughts: true,
    });
    expect(kimiOn['chat_template_kwargs']).toEqual({ thinking: true });
  });

  it('replays reasoning_content safely on tool turns, sends tools:[] for tool history, and applies Mistral tool ID rules', async () => {
    const fetchMock = vi.fn().mockImplementation(
      async () =>
        new Response(
          JSON.stringify({
            choices: [{ message: { content: 'done' }, finish_reason: 'stop' }],
          }),
          { status: 200 },
        ),
    );

    const dsGen = new NvidiaNimContentGenerator({
      model: 'deepseek-ai/deepseek-v4.1-flash',
      fetchImpl: fetchMock as unknown as typeof fetch,
    });
    await dsGen.generateContent(
      {
        model: 'deepseek-ai/deepseek-v4.1-flash',
        contents: [
          { role: 'user', parts: [{ text: 'check' }] },
          {
            role: 'model',
            parts: [
              { text: 'internal check', thought: true },
              {
                functionCall: { id: 'call_1', name: 'ls', args: { path: '.' } },
              },
            ],
          },
          {
            role: 'user',
            parts: [
              {
                functionResponse: {
                  id: 'call_1',
                  name: 'ls',
                  response: { output: 'a.ts' },
                },
              },
            ],
          },
        ],
      },
      'p3',
      LlmRole.MAIN,
    );

    const dsBody = JSON.parse(String(fetchMock.mock.calls[0][1].body)) as {
      messages: Array<Record<string, unknown>>;
      tools?: unknown[];
    };
    expect(dsBody.tools).toEqual([]);
    expect(dsBody.messages[1]).toEqual({
      role: 'assistant',
      content: null,
      reasoning_content: 'internal check',
      tool_calls: [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'ls', arguments: '{"path":"."}' },
        },
      ],
    });

    // Mistral tool_call_id normalization + assistant bridge after tool role
    const mistralGen = new NvidiaNimContentGenerator({
      model: 'mistralai/devstral-2',
      fetchImpl: fetchMock as unknown as typeof fetch,
    });
    await mistralGen.generateContent(
      {
        model: 'mistralai/devstral-2',
        contents: [
          {
            role: 'model',
            parts: [
              {
                functionCall: { id: 'ls__call_abc-123', name: 'ls', args: {} },
              },
            ],
          },
          {
            role: 'user',
            parts: [
              {
                functionResponse: {
                  id: 'ls__call_abc-123',
                  name: 'ls',
                  response: { ok: true },
                },
              },
            ],
          },
          { role: 'user', parts: [{ text: 'next' }] },
        ],
      },
      'p4',
      LlmRole.MAIN,
    );

    const mBody = JSON.parse(String(fetchMock.mock.calls[1][1].body)) as {
      messages: Array<Record<string, unknown>>;
    };
    const tcId = (mBody.messages[0]?.['tool_calls'] as Array<{ id: string }>)[0]
      ?.id;
    expect(tcId).toMatch(/^[a-zA-Z0-9]{9}$/);
    expect(mBody.messages[1]).toMatchObject({
      role: 'tool',
      tool_call_id: tcId,
    });
    expect(mBody.messages[2]).toEqual({ role: 'assistant', content: 'Done.' });
    expect(mBody.messages[3]).toEqual({ role: 'user', content: 'next' });
  });

  it('streams reasoning deltas, emits terminal finishReason + trailing usage for text-only streams, and separates parallel tool calls without index', async () => {
    const textStream = createSseStream([
      'data: {"choices":[{"delta":{"reasoning_content":"Think..."}}]}\n\n',
      'data: {"choices":[{"delta":{"content":"Hello "}}]}\n\n',
      'data: {"choices":[{"delta":{"content":"world"},"finish_reason":"end"}]}\n\n',
      'data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":8,"total_tokens":20,"completion_tokens_details":{"reasoning_tokens":3}}}\n\n',
      'data: [DONE]\n\n',
    ]);

    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(textStream, { status: 200 }));

    const generator = new NvidiaNimContentGenerator({
      model: 'nvidia/nemotron-3-ultra-550b-a55b',
      fetchImpl: fetchMock as unknown as typeof fetch,
    });

    const stream = await generator.generateContentStream(
      {
        model: 'nvidia/nemotron-3-ultra-550b-a55b',
        contents: [{ role: 'user', parts: [{ text: 'say hi' }] }],
      },
      'p5',
      LlmRole.MAIN,
    );

    const chunks = [];
    for await (const chunk of stream) {
      chunks.push(chunk);
    }

    expect(chunks).toHaveLength(4);
    expect(chunks[0]?.candidates?.[0]?.content?.parts).toEqual([
      { text: 'Think...', thought: true },
    ]);
    expect(chunks[1]?.text).toBe('Hello ');
    expect(chunks[2]?.text).toBe('world');
    expect(chunks[3]?.candidates?.[0]?.finishReason).toBe(FinishReason.STOP);
    expect(chunks[3]?.usageMetadata).toEqual({
      promptTokenCount: 12,
      candidatesTokenCount: 8,
      totalTokenCount: 20,
      thoughtsTokenCount: 3,
    });

    // Parallel tool calls omitting `index` must remain distinct by ID
    const parallelToolStream = createSseStream([
      'data: {"choices":[{"delta":{"tool_calls":[{"id":"call_a","type":"function","function":{"name":"read_file","arguments":"{\\"path\\":"}}]}}]}\n\n',
      'data: {"choices":[{"delta":{"tool_calls":[{"id":"call_a","type":"function","function":{"arguments":"\\"a.ts\\"}"}}]}}]}\n\n',
      'data: {"choices":[{"delta":{"tool_calls":[{"id":"call_b","type":"function","function":{"name":"read_file","arguments":"{\\"path\\":\\"b.ts\\"}"}}]},"finish_reason":"tool_calls"}]}\n\n',
      'data: [DONE]\n\n',
    ]);

    fetchMock.mockResolvedValueOnce(
      new Response(parallelToolStream, { status: 200 }),
    );
    const toolStream = await generator.generateContentStream(
      {
        model: 'nvidia/nemotron-3-ultra-550b-a55b',
        contents: [{ role: 'user', parts: [{ text: 'read both' }] }],
      },
      'p6',
      LlmRole.MAIN,
    );
    const toolChunks = [];
    for await (const chunk of toolStream) {
      toolChunks.push(chunk);
    }
    expect(toolChunks).toHaveLength(1);
    expect(toolChunks[0]?.functionCalls).toEqual([
      { id: 'call_a', name: 'read_file', args: { path: 'a.ts' } },
      { id: 'call_b', name: 'read_file', args: { path: 'b.ts' } },
    ]);
    expect(toolChunks[0]?.candidates?.[0]?.finishReason).toBe(
      FinishReason.STOP,
    );
  });

  it('throws on inline SSE error payloads and forwards embedding input_type options', async () => {
    const errorStream = createSseStream([
      'data: {"error":{"message":"Model overloaded","code":503}}\n\n',
    ]);
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(errorStream, { status: 200 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ data: [{ embedding: [0.1, 0.2] }] }), {
          status: 200,
        }),
      );

    const generator = new NvidiaNimContentGenerator({
      embedModel: 'nvidia/nemotron-3.5-embed-1b',
      fetchImpl: fetchMock as unknown as typeof fetch,
    });

    const stream = await generator.generateContentStream(
      {
        model: 'nvidia/nemotron-3-ultra-550b-a55b',
        contents: [{ role: 'user', parts: [{ text: 'fail' }] }],
      },
      'p7',
      LlmRole.MAIN,
    );
    await expect(async () => {
      for await (const chunk of stream) {
        void chunk;
      }
    }).rejects.toThrow('NVIDIA NIM stream error: Model overloaded');

    const embResp = await generator.embedContent({
      model: 'nvidia/nemotron-3.5-embed-1b',
      contents: [{ role: 'user', parts: [{ text: 'embed me' }] }],
      config: {
        outputDimensionality: 512,
        inputType: 'passage',
        truncate: 'END',
      } as unknown as NonNullable<
        Parameters<typeof generator.embedContent>[0]['config']
      >,
    });

    const sentEmbBody = JSON.parse(
      String(fetchMock.mock.calls[1][1].body),
    ) as Record<string, unknown>;
    expect(sentEmbBody).toEqual({
      model: 'nvidia/nemotron-3.5-embed-1b',
      input: 'embed me',
      dimensions: 512,
      input_type: 'passage',
      truncate: 'END',
    });
    expect(embResp.embeddings).toEqual([{ values: [0.1, 0.2] }]);
  });

  it('fetches and sorts live models from /models and handles offline errors gracefully', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: [
              { id: 'z-ai/glm-5.3-flash' },
              { id: 'nvidia/nemotron-3-ultra-550b-a55b' },
              { id: 'z-ai/glm-5.3-flash' },
            ],
          }),
          { status: 200 },
        ),
      )
      .mockRejectedValueOnce(new Error('network offline'));

    const models = await fetchNimAvailableModels(
      'https://integrate.api.nvidia.com/v1',
      'nvapi-test',
      fetchMock as unknown as typeof fetch,
    );
    expect(models).toEqual([
      'nvidia/nemotron-3-ultra-550b-a55b',
      'z-ai/glm-5.3-flash',
    ]);

    const offlineModels = await fetchNimAvailableModels(
      'https://integrate.api.nvidia.com/v1',
      'nvapi-test',
      fetchMock as unknown as typeof fetch,
    );
    expect(offlineModels).toEqual([]);
  });
});
