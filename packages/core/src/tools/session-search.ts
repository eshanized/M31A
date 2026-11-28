/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as path from 'node:path';
import type { MessageBus } from '../confirmation-bus/message-bus.js';
import { type Config } from '../config/config.js';
import { Storage } from '../config/storage.js';
import { SessionSearchIndex } from '../services/sessionSearchIndex.js';
import { getErrorMessage } from '../utils/errors.js';
import { SESSION_SEARCH_DEFINITION } from './definitions/coreTools.js';
import { resolveToolDeclaration } from './definitions/resolver.js';
import {
  SESSION_SEARCH_DISPLAY_NAME,
  SESSION_SEARCH_TOOL_NAME,
} from './tool-names.js';
import { ToolErrorType } from './tool-error.js';
import {
  BaseDeclarativeTool,
  BaseToolInvocation,
  Kind,
  type ExecuteOptions,
  type ToolInvocation,
  type ToolResult,
} from './tools.js';

export interface SessionSearchToolParams {
  query: string;
  limit?: number;
}

class SessionSearchInvocation extends BaseToolInvocation<
  SessionSearchToolParams,
  ToolResult
> {
  constructor(
    private readonly config: Config,
    params: SessionSearchToolParams,
    messageBus: MessageBus,
    toolName?: string,
    toolDisplayName?: string,
  ) {
    super(params, messageBus, toolName, toolDisplayName);
  }

  getDescription(): string {
    return `Search past sessions for "${this.params.query}"`;
  }

  async execute({
    abortSignal: _abortSignal,
  }: ExecuteOptions): Promise<ToolResult> {
    let index: SessionSearchIndex | null = null;
    try {
      const dbPath = path.join(
        Storage.getGlobalM31aDir(),
        'sessions-fts.sqlite',
      );
      index = await SessionSearchIndex.create(dbPath);
      const chatsDir = path.join(
        this.config.storage.getProjectTempDir(),
        'chats',
      );
      await index.indexDirectory(chatsDir, this.config.getTargetDir());
      const hits = index.search(this.params.query, {
        limit: this.params.limit ?? 10,
      });

      if (hits.length === 0) {
        return {
          llmContent: `No past sessions matched query "${this.params.query}".`,
          returnDisplay: `No session matches for "${this.params.query}".`,
        };
      }

      const formatted = hits
        .map(
          (hit, idx) =>
            `${idx + 1}. [${hit.sessionId}] (${hit.updatedAt}) ${hit.summary}\n   ${hit.snippet || '(matched metadata)'}`,
        )
        .join('\n\n');

      return {
        llmContent: formatted,
        returnDisplay: `Found ${hits.length} matching session${hits.length === 1 ? '' : 's'} for "${this.params.query}".`,
      };
    } catch (error) {
      const msg = getErrorMessage(error);
      return {
        llmContent: `Session search failed: ${msg}`,
        returnDisplay: `Session search error: ${msg}`,
        error: {
          message: msg,
          type: ToolErrorType.EXECUTION_FAILED,
        },
      };
    } finally {
      index?.close();
    }
  }
}

/**
 * Full-text session transcript search tool powered by SQLite FTS5.
 */
export class SessionSearchTool extends BaseDeclarativeTool<
  SessionSearchToolParams,
  ToolResult
> {
  static readonly Name = SESSION_SEARCH_TOOL_NAME;

  constructor(
    private readonly config: Config,
    messageBus: MessageBus,
  ) {
    super(
      SessionSearchTool.Name,
      SESSION_SEARCH_DISPLAY_NAME,
      SESSION_SEARCH_DEFINITION.base.description ??
        'Search past session transcripts using SQLite FTS5.',
      Kind.Search,
      SESSION_SEARCH_DEFINITION.base.parametersJsonSchema,
      messageBus,
      true,
      false,
    );
  }

  protected override validateToolParamValues(
    params: SessionSearchToolParams,
  ): string | null {
    if (!params.query || params.query.trim().length === 0) {
      return "The 'query' parameter must be non-empty.";
    }
    if (
      params.limit !== undefined &&
      (!Number.isInteger(params.limit) || params.limit < 1 || params.limit > 50)
    ) {
      return "The 'limit' parameter must be an integer between 1 and 50.";
    }
    return null;
  }

  protected createInvocation(
    params: SessionSearchToolParams,
    messageBus: MessageBus,
    toolName?: string,
    toolDisplayName?: string,
  ): ToolInvocation<SessionSearchToolParams, ToolResult> {
    return new SessionSearchInvocation(
      this.config,
      params,
      messageBus,
      toolName,
      toolDisplayName,
    );
  }

  override getSchema(modelId?: string) {
    return resolveToolDeclaration(SESSION_SEARCH_DEFINITION, modelId);
  }
}
