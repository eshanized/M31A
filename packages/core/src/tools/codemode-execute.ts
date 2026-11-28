/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import * as util from 'node:util';
import * as vm from 'node:vm';
import { glob } from 'glob';
import type { MessageBus } from '../confirmation-bus/message-bus.js';
import { type Config } from '../config/config.js';
import { getErrorMessage } from '../utils/errors.js';
import { resolveToRealPath } from '../utils/paths.js';
import { EXECUTE_DEFINITION } from './definitions/coreTools.js';
import { resolveToolDeclaration } from './definitions/resolver.js';
import { EXECUTE_DISPLAY_NAME, EXECUTE_TOOL_NAME } from './tool-names.js';
import { ToolErrorType } from './tool-error.js';
import {
  BaseDeclarativeTool,
  BaseToolInvocation,
  Kind,
  type ExecuteOptions,
  type ToolInvocation,
  type ToolResult,
} from './tools.js';

export interface ExecuteToolParams {
  code: string;
}

const DEFAULT_TIMEOUT_MS = 5000;
const MAX_OUTPUT_CHARS = 32_000;
const MAX_FILE_READ_BYTES = 256 * 1024;
const MAX_GLOB_RESULTS = 200;
const MAX_GREP_MATCHES = 100;

function formatOutputValue(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }
  return util.inspect(value, { depth: 4, maxArrayLength: 100 });
}

function capOutput(text: string): string {
  if (text.length <= MAX_OUTPUT_CHARS) {
    return text;
  }
  return (
    text.slice(0, MAX_OUTPUT_CHARS) +
    `\n... [output truncated at ${MAX_OUTPUT_CHARS} chars]`
  );
}

/**
 * Runs a read-only JavaScript inspection script inside an isolated V8 VM context
 * with workspace-scoped helper APIs and strict timeout / output bounds.
 */
export async function runSandboxedInspection(
  code: string,
  config: Config,
  timeoutMs: number = DEFAULT_TIMEOUT_MS,
): Promise<{ logs: string[]; result: unknown }> {
  const targetDir = config.getTargetDir();
  const logs: string[] = [];

  const assertWorkspaceReadPath = (relOrAbsPath: string): string => {
    const resolved = path.resolve(targetDir, relOrAbsPath);
    let realTarget = resolved;
    try {
      realTarget = resolveToRealPath(resolved);
    } catch {
      realTarget = resolved;
    }
    const err = config.validatePathAccess(realTarget, 'read');
    if (err) {
      throw new Error(err);
    }
    return realTarget;
  };

  const sandboxApi = {
    console: {
      log: (...args: unknown[]) => {
        logs.push(args.map((a) => formatOutputValue(a)).join(' '));
      },
      warn: (...args: unknown[]) => {
        logs.push('[warn] ' + args.map((a) => formatOutputValue(a)).join(' '));
      },
      error: (...args: unknown[]) => {
        logs.push('[error] ' + args.map((a) => formatOutputValue(a)).join(' '));
      },
    },
    readFile: async (
      filePath: string,
      opts?: { startLine?: number; endLine?: number },
    ): Promise<string> => {
      const safePath = assertWorkspaceReadPath(filePath);
      const stat = await fs.stat(safePath);
      if (
        stat.size > MAX_FILE_READ_BYTES &&
        !opts?.startLine &&
        !opts?.endLine
      ) {
        throw new Error(
          `File "${filePath}" exceeds ${MAX_FILE_READ_BYTES} bytes without startLine/endLine slice.`,
        );
      }
      const raw = await fs.readFile(safePath, 'utf-8');
      if (opts?.startLine !== undefined || opts?.endLine !== undefined) {
        const lines = raw.split('\n');
        const start = Math.max(1, opts.startLine ?? 1) - 1;
        const end = opts.endLine ?? lines.length;
        return lines.slice(start, end).join('\n');
      }
      return raw;
    },
    listDir: async (dirPath: string = '.'): Promise<string[]> => {
      const safePath = assertWorkspaceReadPath(dirPath);
      const entries = await fs.readdir(safePath, { withFileTypes: true });
      return entries
        .map((e) => (e.isDirectory() ? `${e.name}/` : e.name))
        .sort();
    },
    glob: async (pattern: string): Promise<string[]> => {
      const safeBase = assertWorkspaceReadPath('.');
      const matches = await glob(pattern, {
        cwd: safeBase,
        nodir: true,
        ignore: ['**/node_modules/**', '**/.git/**'],
      });
      return matches.slice(0, MAX_GLOB_RESULTS).sort();
    },
    grep: async (
      pattern: string,
      subPath: string = '.',
    ): Promise<Array<{ file: string; line: number; text: string }>> => {
      const safeTarget = assertWorkspaceReadPath(subPath);
      const regex = new RegExp(pattern, 'i');
      const results: Array<{ file: string; line: number; text: string }> = [];

      const searchSingleFile = async (absFile: string): Promise<void> => {
        if (results.length >= MAX_GREP_MATCHES) return;
        const stat = await fs.stat(absFile);
        if (stat.size > MAX_FILE_READ_BYTES) return;
        const raw = await fs.readFile(absFile, 'utf-8');
        const lines = raw.split('\n');
        for (let idx = 0; idx < lines.length; idx++) {
          const line = lines[idx] ?? '';
          if (regex.test(line)) {
            results.push({
              file: path.relative(targetDir, absFile) || absFile,
              line: idx + 1,
              text: line.slice(0, 240),
            });
            if (results.length >= MAX_GREP_MATCHES) {
              break;
            }
          }
        }
      };

      const targetStat = await fs.stat(safeTarget);
      if (targetStat.isFile()) {
        await searchSingleFile(safeTarget);
        return results;
      }

      const files = await glob('**/*', {
        cwd: safeTarget,
        nodir: true,
        ignore: ['**/node_modules/**', '**/.git/**'],
      });
      for (const rel of files) {
        if (results.length >= MAX_GREP_MATCHES) break;
        const candidate = path.join(safeTarget, rel);
        if (!config.validatePathAccess(candidate, 'read')) {
          await searchSingleFile(candidate);
        }
      }
      return results;
    },
  };

  const context = vm.createContext(sandboxApi, {
    codeGeneration: { strings: false, wasm: false },
  });

  const wrapped = `(async () => {\n${code}\n})()`;
  const script = new vm.Script(wrapped, { filename: 'm31a-codemode.js' });
  const executionPromise: unknown = script.runInContext(context, {
    timeout: timeoutMs,
  });

  const resolvedPromise = Promise.resolve(executionPromise);

  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const result = await Promise.race([
      resolvedPromise,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`Execution timed out after ${timeoutMs}ms.`)),
          timeoutMs,
        );
      }),
    ]);
    return { logs, result };
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

class ExecuteToolInvocation extends BaseToolInvocation<
  ExecuteToolParams,
  ToolResult
> {
  constructor(
    private readonly config: Config,
    params: ExecuteToolParams,
    messageBus: MessageBus,
    toolName?: string,
    toolDisplayName?: string,
  ) {
    super(params, messageBus, toolName, toolDisplayName);
  }

  getDescription(): string {
    const preview = this.params.code.trim().split('\n')[0] ?? '';
    return `Execute read-only JS inspection (${preview.slice(0, 60)})`;
  }

  async execute({
    abortSignal: _abortSignal,
  }: ExecuteOptions): Promise<ToolResult> {
    try {
      const { logs, result } = await runSandboxedInspection(
        this.params.code,
        this.config,
      );
      const parts: string[] = [];
      if (logs.length > 0) {
        parts.push(`Console:\n${logs.join('\n')}`);
      }
      if (result !== undefined) {
        const serialized =
          typeof result === 'string'
            ? result
            : (JSON.stringify(result, null, 2) ?? formatOutputValue(result));
        parts.push(`Result:\n${serialized}`);
      }
      const output = capOutput(
        parts.length > 0
          ? parts.join('\n\n')
          : '(completed with undefined result)',
      );
      return {
        llmContent: output,
        returnDisplay: output,
      };
    } catch (error) {
      const msg = getErrorMessage(error);
      return {
        llmContent: `Sandboxed execute error: ${msg}`,
        returnDisplay: `Error: ${msg}`,
        error: {
          message: msg,
          type: ToolErrorType.EXECUTION_FAILED,
        },
      };
    }
  }
}

/**
 * Sandboxed V8 VM batch inspection tool (`execute`) inspired by OpenCode codemode.
 */
export class ExecuteTool extends BaseDeclarativeTool<
  ExecuteToolParams,
  ToolResult
> {
  static readonly Name = EXECUTE_TOOL_NAME;

  constructor(
    private readonly config: Config,
    messageBus: MessageBus,
  ) {
    super(
      ExecuteTool.Name,
      EXECUTE_DISPLAY_NAME,
      EXECUTE_DEFINITION.base.description ??
        'Execute bounded read-only JavaScript batch inspections in an isolated VM.',
      Kind.Read,
      EXECUTE_DEFINITION.base.parametersJsonSchema,
      messageBus,
      true,
      false,
    );
  }

  protected override validateToolParamValues(
    params: ExecuteToolParams,
  ): string | null {
    if (!params.code || params.code.trim().length === 0) {
      return "The 'code' parameter must be non-empty.";
    }
    return null;
  }

  protected createInvocation(
    params: ExecuteToolParams,
    messageBus: MessageBus,
    toolName?: string,
    toolDisplayName?: string,
  ): ToolInvocation<ExecuteToolParams, ToolResult> {
    return new ExecuteToolInvocation(
      this.config,
      params,
      messageBus,
      toolName,
      toolDisplayName,
    );
  }

  override getSchema(modelId?: string) {
    return resolveToolDeclaration(EXECUTE_DEFINITION, modelId);
  }
}
