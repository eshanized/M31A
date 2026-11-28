/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import fsPromises from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import * as Diff from 'diff';
import {
  APPLY_PATCH_TOOL_NAME,
  APPLY_PATCH_DISPLAY_NAME,
} from './tool-names.js';
import type { Config } from '../config/config.js';
import {
  BaseDeclarativeTool,
  BaseToolInvocation,
  Kind,
  type ExecuteOptions,
  type FileDiff,
  type ToolCallConfirmationDetails,
  type ToolEditConfirmationDetails,
  type ToolInvocation,
  type ToolLocation,
  type ToolResult,
  type ToolConfirmationOutcome,
} from './tools.js';
import { ToolErrorType } from './tool-error.js';
import {
  makeRelative,
  shortenPath,
  resolveDefensiveToolPath,
  resolveToRealPath,
} from '../utils/paths.js';
import { resolveActiveWorkspaceDir } from '../utils/pathCorrector.js';
import { getErrorMessage, isNodeError } from '../utils/errors.js';
import { detectLineEnding } from '../utils/textUtils.js';
import { DEFAULT_DIFF_OPTIONS, getDiffStat } from './diffOptions.js';
import type { MessageBus } from '../confirmation-bus/message-bus.js';
import { APPLY_PATCH_DEFINITION } from './definitions/coreTools.js';
import { isBuildFile } from '../utils/buildFileUtils.js';
import { recordModifiedBuildFile } from '../utils/untrustedContextTracker.js';
import { withPathLocks } from '../utils/pathMutex.js';
import { splitBom, joinBom, normalizeSmartUnicode } from '../utils/bom.js';
import { formatEditedFile } from '../services/formatterService.js';
import { discoverJitContext, appendJitContext } from './jit-context.js';
import { logFileOperation } from '../telemetry/loggers.js';
import { FileOperationEvent } from '../telemetry/types.js';
import { FileOperation } from '../telemetry/metrics.js';
import { getSpecificMimeType } from '../utils/fileUtils.js';
import { getLanguageFromFilePath } from '../utils/language-detection.js';

export interface ApplyPatchToolParams {
  patch_text: string;
}

export interface UpdateFileChunk {
  readonly oldLines: readonly string[];
  readonly newLines: readonly string[];
  readonly changeContext?: string;
  readonly endOfFile?: boolean;
}

export type PatchHunk =
  | { readonly type: 'add'; readonly path: string; readonly contents: string }
  | { readonly type: 'delete'; readonly path: string }
  | {
      readonly type: 'update';
      readonly path: string;
      readonly movePath?: string;
      readonly chunks: readonly UpdateFileChunk[];
    };

export class PatchParseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'PatchParseError';
  }
}

function stripHeredoc(input: string): string {
  const match = input.match(
    /^(?:cat\s+)?<<(['"]?)(\w+)\1\s*\n([\s\S]*?)\n\2\s*$/,
  );
  return match ? match[3] : input;
}

function isBoundary(line: string): boolean {
  return (
    line === '*** End Patch' ||
    line.startsWith('*** Add File: ') ||
    line.startsWith('*** Delete File: ') ||
    line.startsWith('*** Update File: ')
  );
}

function parseAdd(
  lines: readonly string[],
  start: number,
  end: number,
  filePath: string,
): { content: string; next: number } {
  const content: string[] = [];
  let index = start;
  while (index < end && !isBoundary(lines[index].trim())) {
    if (!lines[index].startsWith('+')) {
      const line = lines[index].trim();
      throw new PatchParseError(
        `Invalid Add File line ${index + 1} for '${filePath}': expected a line starting with '+', got '${line}'`,
      );
    }
    content.push(`${lines[index].slice(1)}\n`);
    index++;
  }
  return { content: content.join(''), next: index };
}

function parseUpdate(
  lines: readonly string[],
  start: number,
  end: number,
  filePath: string,
  hunkIndex: number,
): { chunks: readonly UpdateFileChunk[]; next: number } {
  const chunks: Array<{
    oldLines: string[];
    newLines: string[];
    changeContext?: string;
    endOfFile?: boolean;
  }> = [];
  let index = start;
  let afterEndOfFile = false;

  while (index < end) {
    const line = lines[index];
    const updateLine = line.trimEnd();
    if (afterEndOfFile) {
      if (updateLine === '') {
        index++;
        continue;
      }
      if (updateLine === '@@' || updateLine.startsWith('@@ ')) {
        afterEndOfFile = false;
      } else if (isBoundary(updateLine)) {
        break;
      } else {
        throw new PatchParseError(
          `Expected update hunk to start with '@@' at line ${index + 1}, got '${line}'`,
        );
      }
    }

    if (updateLine === '*** End of File') {
      const chunk = chunks.at(-1);
      if (chunk && chunk.oldLines.length === 0 && chunk.newLines.length === 0) {
        throw new PatchParseError(
          `Update hunk at line ${index + 1} does not contain any lines`,
        );
      }
      if (chunk) {
        chunk.endOfFile = true;
        afterEndOfFile = true;
      }
      index++;
      continue;
    }

    if (isBoundary(updateLine)) break;

    if (updateLine === '@@' || updateLine.startsWith('@@ ')) {
      const previous = chunks.at(-1);
      if (
        previous &&
        previous.oldLines.length === 0 &&
        previous.newLines.length === 0
      ) {
        throw new PatchParseError(
          `Unexpected line in update hunk at line ${index + 1}: '${line}'`,
        );
      }
      chunks.push({
        oldLines: [],
        newLines: [],
        changeContext:
          updateLine === '@@' ? undefined : updateLine.slice('@@ '.length),
      });
      index++;
      continue;
    }

    if (chunks.length === 0) {
      chunks.push({ oldLines: [], newLines: [] });
    }
    const chunk = chunks.at(-1)!;
    if (line === '') {
      chunk.oldLines.push('');
      chunk.newLines.push('');
      index++;
      continue;
    }
    if (line.startsWith(' ')) {
      chunk.oldLines.push(line.slice(1));
      chunk.newLines.push(line.slice(1));
      index++;
      continue;
    }
    if (line.startsWith('-')) {
      chunk.oldLines.push(line.slice(1));
      index++;
      continue;
    }
    if (line.startsWith('+')) {
      chunk.newLines.push(line.slice(1));
      index++;
      continue;
    }

    throw new PatchParseError(
      `Invalid update line ${index + 1} for '${filePath}': '${line}'. Lines inside an update hunk must start with ' ', '-', or '+'`,
    );
  }

  if (chunks.length === 0) {
    throw new PatchParseError(
      `Update file hunk at line ${hunkIndex + 1} for '${filePath}' is empty`,
    );
  }
  const last = chunks.at(-1)!;
  if (last.oldLines.length === 0 && last.newLines.length === 0) {
    throw new PatchParseError(
      `Update hunk for '${filePath}' does not contain any lines`,
    );
  }

  return { chunks, next: index };
}

/**
 * Parses a multi-file `*** Begin Patch` ... `*** End Patch` document.
 */
export function parsePatchText(patchText: string): PatchHunk[] {
  const lines = stripHeredoc(patchText.trim())
    .split('\n')
    .map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line));

  const begin = lines[0]?.trim() === '*** Begin Patch' ? 0 : -1;
  const end = lines.at(-1)?.trim() === '*** End Patch' ? lines.length - 1 : -1;

  if (begin === -1) {
    throw new PatchParseError(
      "The first line of the patch must be '*** Begin Patch'",
    );
  }
  if (end === -1 || begin >= end) {
    throw new PatchParseError(
      "The last line of the patch must be '*** End Patch'",
    );
  }

  const hunks: PatchHunk[] = [];
  let index = begin + 1;
  while (index < end) {
    const line = lines[index];
    const header = line.trim();
    if (!header) {
      index++;
      continue;
    }

    if (header.startsWith('*** Add File: ')) {
      const filePath = header.slice('*** Add File: '.length).trim();
      if (!filePath) {
        throw new PatchParseError(`Empty file path at line ${index + 1}`);
      }
      const parsed = parseAdd(lines, index + 1, end, filePath);
      hunks.push({ type: 'add', path: filePath, contents: parsed.content });
      index = parsed.next;
      continue;
    }

    if (header.startsWith('*** Delete File: ')) {
      const filePath = header.slice('*** Delete File: '.length).trim();
      if (!filePath) {
        throw new PatchParseError(`Empty file path at line ${index + 1}`);
      }
      const nextLine = lines[index + 1]?.trim();
      if (index + 1 < end && nextLine && !isBoundary(nextLine)) {
        throw new PatchParseError(
          `Unexpected line after Delete File '${filePath}' at line ${index + 2}: '${nextLine}'`,
        );
      }
      hunks.push({ type: 'delete', path: filePath });
      index++;
      continue;
    }

    if (header.startsWith('*** Update File: ')) {
      const filePath = header.slice('*** Update File: '.length).trim();
      if (!filePath) {
        throw new PatchParseError(`Empty file path at line ${index + 1}`);
      }
      let next = index + 1;
      let movePath: string | undefined;
      while (lines[next]?.trimEnd() === '*** End of File') next++;
      const move = lines[next]?.trimEnd();
      if (move === '*** Move to:' || move?.startsWith('*** Move to: ')) {
        movePath = move.slice('*** Move to: '.length).trim();
        if (!movePath) {
          throw new PatchParseError(
            `Move destination for '${filePath}' at line ${next + 1} must not be empty`,
          );
        }
        next++;
      }
      const parsed = parseUpdate(lines, next, end, filePath, index);
      hunks.push({
        type: 'update',
        path: filePath,
        movePath,
        chunks: parsed.chunks,
      });
      index = parsed.next;
      continue;
    }

    throw new PatchParseError(
      `Invalid hunk header at line ${index + 1}: '${header}'. Expected '*** Add File: <path>', '*** Delete File: <path>', or '*** Update File: <path>'`,
    );
  }

  return hunks;
}

const exactLine = (left: string, right: string) => left === right;
const rstripLine = (left: string, right: string) =>
  left.trimEnd() === right.trimEnd();
const trimLine = (left: string, right: string) => left.trim() === right.trim();
const normalizedLine = (left: string, right: string) =>
  normalizeSmartUnicode(left.trim()) === normalizeSmartUnicode(right.trim());

function matchesAt(
  lines: readonly string[],
  pattern: readonly string[],
  offset: number,
  compare: (left: string, right: string) => boolean,
): boolean {
  return pattern.every((line, index) => compare(lines[offset + index], line));
}

function seekLines(
  lines: readonly string[],
  pattern: readonly string[],
  start: number,
  eof = false,
): number {
  if (pattern.length === 0) return -1;
  const comparators = [exactLine, rstripLine, trimLine, normalizedLine];
  if (eof) {
    const offset = lines.length - pattern.length;
    if (offset < start) return -1;
    for (const compare of comparators) {
      if (matchesAt(lines, pattern, offset, compare)) return offset;
    }
    return -1;
  }
  for (const compare of comparators) {
    for (
      let offset = start;
      offset <= lines.length - pattern.length;
      offset++
    ) {
      if (matchesAt(lines, pattern, offset, compare)) return offset;
    }
  }
  return -1;
}

/**
 * Computes updated file content by applying parsed update chunks against `original`.
 */
export function deriveUpdatedContent(
  filePath: string,
  chunks: readonly UpdateFileChunk[],
  original: string,
): { content: string; bom: boolean; lineEnding: '\r\n' | '\n' } {
  const source = splitBom(original);
  const lineEnding = detectLineEnding(source.text);
  const normalized = source.text.replace(/\r\n/g, '\n');
  const lines = normalized.split('\n');
  if (lines.at(-1) === '') lines.pop();

  const replacements: Array<[number, number, readonly string[]]> = [];
  let lineIndex = 0;

  for (const chunk of chunks) {
    if (chunk.changeContext) {
      const contextIdx = seekLines(lines, [chunk.changeContext], lineIndex);
      if (contextIdx === -1) {
        throw new Error(
          `Failed to find context '${chunk.changeContext}' in ${filePath}`,
        );
      }
      lineIndex = contextIdx + 1;
    }
    if (chunk.oldLines.length === 0) {
      replacements.push([lines.length, 0, chunk.newLines]);
      continue;
    }

    let oldLines = chunk.oldLines;
    let newLines = chunk.newLines;
    let found = seekLines(lines, oldLines, lineIndex, chunk.endOfFile);
    if (found === -1 && oldLines.at(-1) === '') {
      oldLines = oldLines.slice(0, -1);
      if (newLines.at(-1) === '') newLines = newLines.slice(0, -1);
      found = seekLines(lines, oldLines, lineIndex, chunk.endOfFile);
    }
    if (found === -1) {
      throw new Error(
        `Failed to find expected lines in ${filePath}:\n${chunk.oldLines.join('\n')}`,
      );
    }
    replacements.push([found, oldLines.length, newLines]);
    lineIndex = found + oldLines.length;
  }

  replacements.sort((a, b) => a[0] - b[0]);
  for (const [start, removeCount, insertLines] of replacements.reverse()) {
    lines.splice(start, removeCount, ...insertLines);
  }
  if (lines.at(-1) !== '') lines.push('');

  const rebuilt = lines.join('\n');
  const nextBom = splitBom(rebuilt);
  return {
    content: nextBom.text,
    bom: source.bom || nextBom.bom,
    lineEnding,
  };
}

export function isApplyPatchToolParams(
  args: unknown,
): args is ApplyPatchToolParams {
  return (
    typeof args === 'object' &&
    args !== null &&
    'patch_text' in args &&
    typeof args.patch_text === 'string'
  );
}

interface PreparedFileOp {
  readonly type: 'add' | 'delete' | 'update';
  readonly sourcePath: string;
  readonly targetPath: string;
  readonly before: string;
  readonly after: string;
  readonly hadBom: boolean;
  readonly lineEnding: '\r\n' | '\n';
}

class ApplyPatchToolInvocation extends BaseToolInvocation<
  ApplyPatchToolParams,
  ToolResult
> {
  private readonly resolvedTargets: Array<{
    hunk: PatchHunk;
    sourcePath: string;
    targetPath: string;
  }>;

  constructor(
    private readonly config: Config,
    params: ApplyPatchToolParams,
    messageBus: MessageBus,
    toolName?: string,
    displayName?: string,
  ) {
    super(
      params,
      messageBus,
      toolName,
      displayName,
      undefined,
      undefined,
      true,
      () => this.config.getApprovalMode(),
    );

    const hunks = parsePatchText(this.params.patch_text);
    this.resolvedTargets = hunks.map((hunk) => {
      const sourcePath = this.resolveWorkspacePath(hunk.path);
      const targetPath =
        hunk.type === 'update' && hunk.movePath
          ? this.resolveWorkspacePath(hunk.movePath)
          : sourcePath;
      return { hunk, sourcePath, targetPath };
    });
  }

  private resolveWorkspacePath(rawPath: string): string {
    const targetDir = resolveActiveWorkspaceDir(this.config, rawPath);
    const sanitizedPath = resolveDefensiveToolPath(rawPath, targetDir);
    const full = path.isAbsolute(rawPath)
      ? rawPath.replace(/\0/g, '')
      : path.resolve(targetDir, sanitizedPath);
    try {
      return resolveToRealPath(full);
    } catch {
      try {
        return path.join(
          resolveToRealPath(path.dirname(full)),
          path.basename(full),
        );
      } catch {
        return full;
      }
    }
  }

  override toolLocations(): ToolLocation[] {
    const unique = new Set<string>();
    for (const item of this.resolvedTargets) {
      unique.add(item.sourcePath);
      unique.add(item.targetPath);
    }
    return Array.from(unique).map((filePath) => ({ path: filePath }));
  }

  override getDescription(): string {
    const count = this.resolvedTargets.length;
    const sample = this.resolvedTargets
      .slice(0, 3)
      .map((item) => {
        const dir = resolveActiveWorkspaceDir(this.config, item.hunk.path);
        return shortenPath(makeRelative(item.targetPath, dir));
      })
      .join(', ');
    return `Apply patch (${count} file${count === 1 ? '' : 's'}: ${sample}${count > 3 ? ', ...' : ''})`;
  }

  private async prepareOperations(): Promise<PreparedFileOp[]> {
    const ops: PreparedFileOp[] = [];
    for (const { hunk, sourcePath, targetPath } of this.resolvedTargets) {
      for (const candidate of [sourcePath, targetPath]) {
        const err = this.config.validatePathAccess(candidate);
        if (err) {
          throw new Error(err);
        }
      }

      if (hunk.type === 'add') {
        try {
          await fsPromises.access(sourcePath);
          throw new Error(`Cannot add file that already exists: ${sourcePath}`);
        } catch (err: unknown) {
          if (!isNodeError(err) || err.code !== 'ENOENT') {
            throw err;
          }
        }
        ops.push({
          type: 'add',
          sourcePath,
          targetPath,
          before: '',
          after: hunk.contents,
          hadBom: splitBom(hunk.contents).bom,
          lineEnding: os.EOL === '\r\n' ? '\r\n' : '\n',
        });
      } else if (hunk.type === 'delete') {
        const raw = await this.config
          .getFileSystemService()
          .readTextFile(sourcePath);
        ops.push({
          type: 'delete',
          sourcePath,
          targetPath,
          before: splitBom(raw).text,
          after: '',
          hadBom: splitBom(raw).bom,
          lineEnding: '\n',
        });
      } else {
        const raw = await this.config
          .getFileSystemService()
          .readTextFile(sourcePath);
        const derived = deriveUpdatedContent(hunk.path, hunk.chunks, raw);
        ops.push({
          type: 'update',
          sourcePath,
          targetPath,
          before: splitBom(raw).text,
          after: derived.content,
          hadBom: derived.bom,
          lineEnding: derived.lineEnding,
        });
      }
    }
    return ops;
  }

  protected override async getConfirmationDetails(
    _abortSignal: AbortSignal,
  ): Promise<ToolCallConfirmationDetails | false> {
    let ops: PreparedFileOp[];
    try {
      ops = await this.prepareOperations();
    } catch {
      return false;
    }
    if (ops.length === 0) return false;

    const combinedDiff = ops
      .map((op) => {
        const name = path.basename(op.targetPath);
        return Diff.createPatch(
          name,
          op.before,
          op.after,
          op.type === 'add' ? 'New' : 'Current',
          op.type === 'delete' ? 'Deleted' : 'Proposed',
          DEFAULT_DIFF_OPTIONS,
        );
      })
      .join('\n');

    const primary = ops[0];
    const confirmation: ToolEditConfirmationDetails = {
      type: 'edit',
      title: `Confirm Patch (${ops.length} file${ops.length === 1 ? '' : 's'})`,
      fileName:
        ops.length === 1
          ? path.basename(primary.targetPath)
          : `${ops.length} files`,
      filePath: primary.targetPath,
      fileDiff: combinedDiff,
      originalContent: ops.length === 1 ? primary.before : null,
      newContent:
        ops.length === 1 ? primary.after : ops.map((o) => o.after).join('\n'),
      isBuildFile: ops.some(
        (op) => isBuildFile(op.sourcePath) || isBuildFile(op.targetPath),
      ),
      onConfirm: async (_outcome: ToolConfirmationOutcome) => {},
    };
    return confirmation;
  }

  async execute({ abortSignal }: ExecuteOptions): Promise<ToolResult> {
    if (abortSignal.aborted) {
      throw new Error('Patch aborted');
    }

    const lockKeys: string[] = [];
    for (const item of this.resolvedTargets) {
      lockKeys.push(item.sourcePath, item.targetPath);
    }

    return withPathLocks(
      lockKeys,
      async () => {
        if (abortSignal.aborted) {
          throw new Error('Patch aborted');
        }

        let ops: PreparedFileOp[];
        try {
          ops = await this.prepareOperations();
        } catch (error) {
          const msg = getErrorMessage(error);
          return {
            llmContent: `Patch verification failed: ${msg}`,
            returnDisplay: `Error: ${msg}`,
            error: {
              message: msg,
              type: ToolErrorType.EDIT_PREPARATION_FAILURE,
            },
          };
        }

        if (ops.length === 0) {
          return {
            llmContent: 'No changes specified in patch.',
            returnDisplay: 'No file changes to apply.',
          };
        }

        const summaryLines: string[] = [];
        const changedFiles: string[] = [];

        try {
          for (const op of ops) {
            if (op.type === 'delete') {
              await fsPromises.unlink(op.sourcePath);
              if (isBuildFile(op.sourcePath)) {
                recordModifiedBuildFile(op.sourcePath, this.config);
              }
              changedFiles.push(`D ${op.sourcePath}`);
              summaryLines.push(`Deleted ${op.sourcePath}`);
              logFileOperation(
                this.config,
                new FileOperationEvent(
                  APPLY_PATCH_TOOL_NAME,
                  FileOperation.UPDATE,
                  op.before.split('\n').length,
                  getSpecificMimeType(op.sourcePath),
                  path.extname(op.sourcePath),
                  getLanguageFromFilePath(op.sourcePath),
                ),
              );
              continue;
            }

            await fsPromises.mkdir(path.dirname(op.targetPath), {
              recursive: true,
            });
            let written =
              op.lineEnding === '\r\n'
                ? op.after.replace(/\r?\n/g, '\r\n')
                : op.after;
            written = joinBom(written, op.hadBom);

            await this.config
              .getFileSystemService()
              .writeTextFile(op.targetPath, written);

            if (op.type === 'update' && op.sourcePath !== op.targetPath) {
              try {
                await fsPromises.unlink(op.sourcePath);
              } catch {
                // Source already removed
              }
            }

            if (isBuildFile(op.targetPath)) {
              recordModifiedBuildFile(op.targetPath, this.config);
            }

            await formatEditedFile(op.targetPath, this.config.getTargetDir());

            const codeLabel =
              op.type === 'add'
                ? 'A'
                : op.sourcePath !== op.targetPath
                  ? 'R'
                  : 'M';
            changedFiles.push(`${codeLabel} ${op.targetPath}`);
            summaryLines.push(`${codeLabel} ${op.targetPath}`);

            logFileOperation(
              this.config,
              new FileOperationEvent(
                APPLY_PATCH_TOOL_NAME,
                op.type === 'add' ? FileOperation.CREATE : FileOperation.UPDATE,
                op.after.split('\n').length,
                getSpecificMimeType(op.targetPath),
                path.extname(op.targetPath),
                getLanguageFromFilePath(op.targetPath),
              ),
            );
          }
        } catch (error) {
          const msg = getErrorMessage(error);
          return {
            llmContent: `Failed while applying patch: ${msg}${summaryLines.length ? `. Completed: ${summaryLines.join(', ')}` : ''}`,
            returnDisplay: `Error applying patch: ${msg}`,
            error: {
              message: msg,
              type: ToolErrorType.FILE_WRITE_FAILURE,
            },
          };
        }

        const primaryOp = ops[0];
        const fileName = path.basename(primaryOp.targetPath);
        const combinedDiff = ops
          .map((op) =>
            Diff.createPatch(
              path.basename(op.targetPath),
              op.before,
              op.after,
              op.type === 'add' ? 'New' : 'Current',
              op.type === 'delete' ? 'Deleted' : 'Proposed',
              DEFAULT_DIFF_OPTIONS,
            ),
          )
          .join('\n');

        const diffStat = getDiffStat(
          fileName,
          primaryOp.before,
          primaryOp.after,
          primaryOp.after,
        );
        const displayResult: FileDiff = {
          fileDiff: combinedDiff,
          fileName: ops.length === 1 ? fileName : `${ops.length} files patched`,
          filePath: primaryOp.targetPath,
          originalContent: ops.length === 1 ? primaryOp.before : null,
          newContent: primaryOp.after,
          diffStat,
          isNewFile: primaryOp.type === 'add',
          isBuildFile: ops.some((o) => isBuildFile(o.targetPath)),
        };

        let llmContent = `Success. Updated the following files:\n${changedFiles.join('\n')}`;
        const jitContext = await discoverJitContext(
          this.config,
          primaryOp.targetPath,
        );
        if (jitContext) {
          llmContent = appendJitContext(llmContent, jitContext);
        }

        return {
          llmContent,
          returnDisplay: displayResult,
        };
      },
      abortSignal,
    );
  }
}

/**
 * Multi-file patch application tool (`*** Begin Patch` ... `*** End Patch`).
 */
export class ApplyPatchTool extends BaseDeclarativeTool<
  ApplyPatchToolParams,
  ToolResult
> {
  static readonly Name = APPLY_PATCH_TOOL_NAME;

  constructor(
    private readonly config: Config,
    messageBus: MessageBus,
  ) {
    super(
      ApplyPatchTool.Name,
      APPLY_PATCH_DISPLAY_NAME,
      APPLY_PATCH_DEFINITION.base.description!,
      Kind.Edit,
      APPLY_PATCH_DEFINITION.base.parametersJsonSchema,
      messageBus,
      true,
      false,
    );
  }

  protected override validateToolParamValues(
    params: ApplyPatchToolParams,
  ): string | null {
    if (!params.patch_text || !params.patch_text.trim()) {
      return "The 'patch_text' parameter must be non-empty.";
    }
    try {
      const hunks = parsePatchText(params.patch_text);
      if (hunks.length === 0) {
        return 'Patch contains no file operations.';
      }
    } catch (err) {
      return err instanceof Error ? err.message : String(err);
    }
    return null;
  }

  protected createInvocation(
    params: ApplyPatchToolParams,
    messageBus: MessageBus,
    toolName?: string,
    displayName?: string,
  ): ToolInvocation<ApplyPatchToolParams, ToolResult> {
    return new ApplyPatchToolInvocation(
      this.config,
      params,
      messageBus,
      toolName,
      displayName,
    );
  }
}
