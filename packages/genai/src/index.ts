/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * M31A `@m31a/genai` — OWN re-authored structural types for the M31A NVIDIA NIM
 * provider. NOT derived from Google `@google/genai`. No Google package, no
 * Google runtime, no Google model. Structural superset is intentionally minimal;
 * exact parity with `@google/genai` is NOT claimed.
 */

// ---------- enums ----------
export enum Type {
  TYPE_UNSPECIFIED = 'TYPE_UNSPECIFIED',
  STRING = 'STRING',
  NUMBER = 'NUMBER',
  INTEGER = 'INTEGER',
  BOOLEAN = 'BOOLEAN',
  ARRAY = 'ARRAY',
  OBJECT = 'OBJECT',
  NULL = 'NULL',
}

export enum FinishReason {
  FINISH_REASON_UNSPECIFIED = 'FINISH_REASON_UNSPECIFIED',
  STOP = 'STOP',
  MAX_TOKENS = 'MAX_TOKENS',
  SAFETY = 'SAFETY',
  RECITATION = 'RECITATION',
  LANGUAGE = 'LANGUAGE',
  OTHER = 'OTHER',
  BLOCKLIST = 'BLOCKLIST',
  PROHIBITED_CONTENT = 'PROHIBITED_CONTENT',
  SPII = 'SPII',
  MALFORMED_FUNCTION_CALL = 'MALFORMED_FUNCTION_CALL',
  IMAGE_SAFETY = 'IMAGE_SAFETY',
  UNEXPECTED_TOOL_CALL = 'UNEXPECTED_TOOL_CALL',
  IMAGE_PROHIBITED_CONTENT = 'IMAGE_PROHIBITED_CONTENT',
  NO_IMAGE = 'NO_IMAGE',
}

export enum HarmCategory {
  HARM_CATEGORY_UNSPECIFIED = 'HARM_CATEGORY_UNSPECIFIED',
  HARM_CATEGORY_HATE_SPEECH = 'HARM_CATEGORY_HATE_SPEECH',
  HARM_CATEGORY_DANGEROUS_CONTENT = 'HARM_CATEGORY_DANGEROUS_CONTENT',
  HARM_CATEGORY_HARASSMENT = 'HARM_CATEGORY_HARASSMENT',
  HARM_CATEGORY_SEXUALLY_EXPLICIT = 'HARM_CATEGORY_SEXUALLY_EXPLICIT',
  HARM_CATEGORY_CIVIC_INTEGRITY = 'HARM_CATEGORY_CIVIC_INTEGRITY',
}

export enum HarmBlockThreshold {
  HARM_BLOCK_THRESHOLD_UNSPECIFIED = 'HARM_BLOCK_THRESHOLD_UNSPECIFIED',
  BLOCK_LOW_AND_ABOVE = 'BLOCK_LOW_AND_ABOVE',
  BLOCK_MEDIUM_AND_ABOVE = 'BLOCK_MEDIUM_AND_ABOVE',
  BLOCK_ONLY_HIGH = 'BLOCK_ONLY_HIGH',
  BLOCK_NONE = 'BLOCK_NONE',
  OFF = 'OFF',
}

export enum BlockedReason {
  BLOCKED_REASON_UNSPECIFIED = 'BLOCKED_REASON_UNSPECIFIED',
  SAFETY = 'SAFETY',
  OTHER = 'OTHER',
  BLOCKLIST = 'BLOCKLIST',
  PROHIBITED_CONTENT = 'PROHIBITED_CONTENT',
  IMAGE_SAFETY = 'IMAGE_SAFETY',
}

export enum FunctionCallingConfigMode {
  MODE_UNSPECIFIED = 'MODE_UNSPECIFIED',
  AUTO = 'AUTO',
  ANY = 'ANY',
  NONE = 'NONE',
  VALIDATED = 'VALIDATED',
}

export enum ThinkingLevel {
  THINKING_LEVEL_UNSPECIFIED = 'THINKING_LEVEL_UNSPECIFIED',
  MINIMAL = 'MINIMAL',
  LOW = 'LOW',
  MEDIUM = 'MEDIUM',
  HIGH = 'HIGH',
}

export enum MediaResolution {
  MEDIA_RESOLUTION_UNSPECIFIED = 'MEDIA_RESOLUTION_UNSPECIFIED',
  MEDIA_RESOLUTION_LOW = 'MEDIA_RESOLUTION_LOW',
  MEDIA_RESOLUTION_MEDIUM = 'MEDIA_RESOLUTION_MEDIUM',
  MEDIA_RESOLUTION_HIGH = 'MEDIA_RESOLUTION_HIGH',
}

export enum Language {
  LANGUAGE_UNSPECIFIED = 'LANGUAGE_UNSPECIFIED',
  PYTHON = 'PYTHON',
}

export enum Outcome {
  OUTCOME_UNSPECIFIED = 'OUTCOME_UNSPECIFIED',
  OUTCOME_OK = 'OUTCOME_OK',
  OUTCOME_FAILED = 'OUTCOME_FAILED',
  OUTCOME_DEADLINE_EXCEEDED = 'OUTCOME_DEADLINE_EXCEEDED',
}

export enum Environment {
  ENVIRONMENT_UNSPECIFIED = 'ENVIRONMENT_UNSPECIFIED',
  ENVIRONMENT_BROWSER = 'ENVIRONMENT_BROWSER',
}

// ---------- Schema ----------
export type SchemaUnion = Schema | Type | Record<string, unknown> | object;
export interface Schema {
  type?: Type;
  format?: string;
  title?: string;
  description?: string;
  nullable?: boolean;
  enum?: string[];
  maxItems?: string;
  minItems?: string;
  properties?: Record<string, Schema>;
  required?: string[];
  items?: Schema;
  anyOf?: Schema[];
  propertyOrdering?: string[];
  default?: unknown;
  example?: unknown;
  minimum?: number;
  maximum?: number;
  minLength?: string;
  maxLength?: string;
  pattern?: string;
  // allow Google extras without breaking
}

// ---------- Parts / Content ----------
export type Role = 'user' | 'model' | 'system' | (string & {});
export interface Blob {
  mimeType?: string;
  data?: string;
  displayName?: string;
}
export interface FileData {
  mimeType?: string;
  fileUri?: string;
  displayName?: string;
}
export interface VideoMetadata {
  fps?: number;
  startOffset?: string;
  endOffset?: string;
}
export interface CodeExecutionResult {
  outcome?: Outcome;
  output?: string;
}
export interface ExecutableCode {
  language?: Language;
  code?: string;
}
export interface FunctionCall {
  id?: string;
  name?: string;
  args?: Record<string, unknown>;
}
export interface FunctionResponse {
  id?: string;
  name?: string;
  response?: Record<string, unknown>;
  parts?: Part[];
  willContinue?: boolean;
  scheduling?: string;
}

export interface Part {
  text?: string;
  inlineData?: Blob;
  fileData?: FileData;
  functionCall?: FunctionCall;
  functionResponse?: FunctionResponse;
  executableCode?: ExecutableCode;
  codeExecutionResult?: CodeExecutionResult;
  videoMetadata?: VideoMetadata;
  thought?: boolean;
  thoughtSignature?: string;
}

export type PartUnion = Part | string;
export type PartListUnion = PartUnion | PartUnion[];
export interface Content {
  role?: string;
  parts?: Part[];
  automaticFunctionCallingHistory?: Content[];
}
export type ContentUnion = Content | PartUnion | PartUnion[] | string;
export type ContentListUnion = ContentUnion | Content[];

// ---------- Tools ----------
export interface FunctionDeclaration {
  name?: string;
  description?: string;
  parameters?: Schema;
  parametersJsonSchema?: unknown;
  response?: Schema;
  responseJsonSchema?: unknown;
  behavior?: string;
}
export interface ComputerUseConfig {
  environment?: Environment;
  excludedPredefinedFunctions?: string[];
}
export interface Tool {
  functionDeclarations?: FunctionDeclaration[];
  googleSearch?: Record<string, unknown>;
  googleSearchRetrieval?: Record<string, unknown>;
  retrieval?: Record<string, unknown>;
  codeExecution?: Record<string, unknown>;
  urlContext?: Record<string, unknown>;
  computerUse?: ComputerUseConfig;
}
export interface CallableTool {
  tool(): Promise<Tool>;
  callTool(functionCalls: FunctionCall[]): Promise<Part[]>;
}
export type ToolListUnion = Array<Tool | CallableTool>;
export interface FunctionCallingConfig {
  mode?: FunctionCallingConfigMode;
  allowedFunctionNames?: string[];
  streamFunctionCallArguments?: boolean;
}
export interface ToolConfig {
  functionCallingConfig?: FunctionCallingConfig;
}

// ---------- Safety ----------
export interface SafetySetting {
  category?: HarmCategory;
  threshold?: HarmBlockThreshold;
  method?: string;
}
export interface SafetyRating {
  category?: HarmCategory;
  probability?: string;
  blocked?: boolean;
  severity?: string;
}

// ---------- GenerateContent ----------
export interface ThinkingConfig {
  includeThoughts?: boolean;
  thinkingBudget?: number;
  thinkingLevel?: ThinkingLevel;
}
export interface SpeechConfig {
  voiceConfig?: unknown;
}
export type SpeechConfigUnion = SpeechConfig | string;
export interface GenerationConfigRoutingConfig {
  autoMode?: unknown;
  manualMode?: unknown;
}
export interface ModelSelectionConfig {
  featureSelectionPreference?: string;
}

export interface GenerateContentConfig {
  systemInstruction?: ContentUnion;
  temperature?: number;
  topP?: number;
  topK?: number;
  candidateCount?: number;
  maxOutputTokens?: number;
  stopSequences?: string[];
  responseMimeType?: string;
  responseSchema?: SchemaUnion;
  responseJsonSchema?: unknown;
  responseModalities?: string[];
  responseLogprobs?: boolean;
  logprobs?: number;
  presencePenalty?: number;
  frequencyPenalty?: number;
  seed?: number;
  tools?: ToolListUnion;
  toolConfig?: ToolConfig;
  safetySettings?: SafetySetting[];
  cachedContent?: string;
  thinkingConfig?: ThinkingConfig;
  speechConfig?: SpeechConfigUnion;
  audioTimestamp?: boolean;
  mediaResolution?: MediaResolution;
  routingConfig?: GenerationConfigRoutingConfig;
  modelSelectionConfig?: ModelSelectionConfig;
  httpOptions?: Record<string, unknown>;
  abortSignal?: AbortSignal;
  labels?: Record<string, string>;
}

export interface GenerateContentParameters {
  model: string;
  contents: ContentListUnion;
  config?: GenerateContentConfig;
}

export interface GroundingChunkWeb {
  uri?: string;
  title?: string;
}
export interface GroundingChunk {
  web?: GroundingChunkWeb;
  retrievedContext?: unknown;
  maps?: unknown;
}
export interface GroundingSupportSegment {
  startIndex?: number;
  endIndex?: number;
  partIndex?: number;
  text?: string;
}
export interface GroundingSupport {
  segment?: GroundingSupportSegment;
  groundingChunkIndices?: number[];
  confidenceScores?: number[];
}
export interface GroundingMetadata {
  groundingChunks?: GroundingChunk[];
  groundingSupports?: GroundingSupport[];
  webSearchQueries?: string[];
  searchEntryPoint?: unknown;
  retrievalMetadata?: unknown;
}
export interface Citation {
  startIndex?: number;
  endIndex?: number;
  uri?: string;
  title?: string;
  license?: string;
  publicationDate?: unknown;
}
export interface CitationMetadata {
  citations?: Citation[];
}
export interface GenerateContentResponsePromptFeedback {
  blockReason?: BlockedReason;
  safetyRatings?: SafetyRating[];
  blockReasonMessage?: string;
}
export interface GenerateContentResponseUsageMetadata {
  promptTokenCount?: number;
  candidatesTokenCount?: number;
  totalTokenCount?: number;
  cachedContentTokenCount?: number;
  thoughtsTokenCount?: number;
  toolUsePromptTokenCount?: number;
  promptTokensDetails?: unknown[];
  candidatesTokensDetails?: unknown[];
}
export type GenerateContentResponseUsageMetadataAlias =
  GenerateContentResponseUsageMetadata;
export interface Candidate {
  content?: Content;
  finishReason?: FinishReason;
  finishMessage?: string;
  safetyRatings?: SafetyRating[];
  citationMetadata?: CitationMetadata;
  groundingMetadata?: GroundingMetadata;
  index?: number;
}

export interface HttpResponse {
  responseInternal?: Response;
  headers?: Record<string, string>;
  body?: string;
  json?: () => Promise<unknown>;
}

const responseOverrides = new WeakMap<
  GenerateContentResponse,
  {
    text?: string;
    data?: string;
    functionCalls?: FunctionCall[];
    executableCode?: string;
    codeExecutionResult?: string;
  }
>();

function setResponseOverride<
  K extends
    | 'text'
    | 'data'
    | 'functionCalls'
    | 'executableCode'
    | 'codeExecutionResult',
>(
  instance: GenerateContentResponse,
  key: K,
  value: GenerateContentResponse[K],
): void {
  const entry = responseOverrides.get(instance) ?? {};
  entry[key] = value;
  responseOverrides.set(instance, entry);
}

export class GenerateContentResponse {
  candidates?: Candidate[];
  promptFeedback?: GenerateContentResponsePromptFeedback;
  usageMetadata?: GenerateContentResponseUsageMetadata;
  modelVersion?: string;
  responseId?: string;

  /** convenience: concatenated text of first candidate */
  get text(): string | undefined {
    const override = responseOverrides.get(this);
    if (override && 'text' in override) {
      return override.text;
    }
    const parts = this.candidates?.[0]?.content?.parts;
    if (!parts?.length) {
      return undefined;
    }
    let out = '';
    let hasText = false;
    for (const part of parts) {
      if (typeof part.text === 'string' && !part.thought) {
        out += part.text;
        hasText = true;
      }
    }
    return hasText ? out : undefined;
  }
  set text(value: string | undefined) {
    setResponseOverride(this, 'text', value);
  }

  /** convenience: inline data of first candidate */
  get data(): string | undefined {
    const override = responseOverrides.get(this);
    if (override && 'data' in override) {
      return override.data;
    }
    const parts = this.candidates?.[0]?.content?.parts;
    if (!parts?.length) {
      return undefined;
    }
    let out = '';
    let hasData = false;
    for (const part of parts) {
      if (typeof part.inlineData?.data === 'string') {
        out += part.inlineData.data;
        hasData = true;
      }
    }
    return hasData ? out : undefined;
  }
  set data(value: string | undefined) {
    setResponseOverride(this, 'data', value);
  }

  /** convenience: function calls in first candidate */
  get functionCalls(): FunctionCall[] | undefined {
    const override = responseOverrides.get(this);
    if (override && 'functionCalls' in override) {
      return override.functionCalls;
    }
    const parts = this.candidates?.[0]?.content?.parts;
    if (!parts?.length) {
      return undefined;
    }
    const calls: FunctionCall[] = [];
    for (const part of parts) {
      if (part.functionCall) {
        calls.push(part.functionCall);
      }
    }
    return calls.length > 0 ? calls : undefined;
  }
  set functionCalls(value: FunctionCall[] | undefined) {
    setResponseOverride(this, 'functionCalls', value);
  }

  /** convenience: executable code in first candidate */
  get executableCode(): string | undefined {
    const override = responseOverrides.get(this);
    if (override && 'executableCode' in override) {
      return override.executableCode;
    }
    const parts = this.candidates?.[0]?.content?.parts;
    if (!parts?.length) {
      return undefined;
    }
    for (const part of parts) {
      if (part.executableCode?.code !== undefined) {
        return part.executableCode.code;
      }
    }
    return undefined;
  }
  set executableCode(value: string | undefined) {
    setResponseOverride(this, 'executableCode', value);
  }

  /** convenience: code execution result in first candidate */
  get codeExecutionResult(): string | undefined {
    const override = responseOverrides.get(this);
    if (override && 'codeExecutionResult' in override) {
      return override.codeExecutionResult;
    }
    const parts = this.candidates?.[0]?.content?.parts;
    if (!parts?.length) {
      return undefined;
    }
    for (const part of parts) {
      if (part.codeExecutionResult?.output !== undefined) {
        return part.codeExecutionResult.output;
      }
    }
    return undefined;
  }
  set codeExecutionResult(value: string | undefined) {
    setResponseOverride(this, 'codeExecutionResult', value);
  }

  automaticFunctionCallingHistory?: Content[];
  sdkHttpResponse?: HttpResponse;
}

// ---------- Tokens ----------
export interface CountTokensParameters {
  model: string;
  contents: ContentListUnion;
  config?: Record<string, unknown>;
}
export interface CountTokensResponse {
  totalTokens?: number;
  cachedContentTokenCount?: number;
}

// ---------- Embeddings ----------
export interface ContentEmbedding {
  values?: number[];
}
export interface EmbedContentParameters {
  model: string;
  contents: ContentListUnion;
  config?: Record<string, unknown>;
}
export class EmbedContentResponse {
  embeddings?: ContentEmbedding[];
  metadata?: Record<string, unknown>;
}

// ---------- Errors ----------
export class ApiError extends Error {
  status: number;
  constructor(
    options: { message: string; status: number } | string,
    errorOptions?: ErrorOptions,
  ) {
    if (typeof options === 'string') {
      super(options, errorOptions);
      this.status = 0;
    } else {
      super(options.message, errorOptions);
      this.status = options.status;
    }
    this.name = 'ApiError';
  }
}

// ---------- helpers (own impls; NOT Google) ----------
export function createPartFromText(text: string): Part {
  return { text };
}
export function createUserContent(content: PartListUnion): Content {
  const parts: Part[] = Array.isArray(content)
    ? content.map((c) => (typeof c === 'string' ? { text: c } : c))
    : typeof content === 'string'
      ? [{ text: content }]
      : [content];
  return { role: 'user', parts };
}
