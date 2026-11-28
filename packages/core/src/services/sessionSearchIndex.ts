/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import * as process from 'node:process';
import { z } from 'zod';
import { Storage } from '../config/storage.js';

export interface SessionIndexDocument {
  sessionId: string;
  projectRoot: string;
  summary: string;
  content: string;
  updatedAt: string;
  filePath?: string;
  mtimeMs?: number;
}

export interface SessionSearchHit {
  sessionId: string;
  projectRoot: string;
  summary: string;
  snippet: string;
  updatedAt: string;
  score: number;
}

interface SqliteStatementLike {
  run: (...params: Array<string | number | null>) => unknown;
  get: (...params: Array<string | number | null>) => unknown;
  all: (...params: Array<string | number | null>) => unknown[];
}

interface SqliteDatabaseLike {
  exec: (sql: string) => void;
  prepare: (sql: string) => SqliteStatementLike;
  close: () => void;
}

type DatabaseSyncConstructor = new (location: string) => SqliteDatabaseLike;

const SqliteModuleSchema = z.object({
  DatabaseSync: z.custom<DatabaseSyncConstructor>(
    (v) => typeof v === 'function',
  ),
});

const MetaRowSchema = z.object({
  mtime_ms: z.number(),
});

const SearchRowSchema = z.object({
  session_id: z.string(),
  project_root: z.string(),
  summary: z.string(),
  snippet: z.string(),
  updated_at: z.string(),
  score: z.number(),
});

const StoredSessionRecordSchema = z.object({
  sessionId: z.string().optional(),
  summary: z.string().optional(),
  lastUpdated: z.string().optional(),
  startTime: z.string().optional(),
  messages: z
    .array(
      z.object({
        type: z.string().optional(),
        content: z.unknown().optional(),
      }),
    )
    .optional(),
});

const MessagePartTextSchema = z.object({
  text: z.string(),
});

function loadDatabaseSync(): DatabaseSyncConstructor {
  const rawModule: unknown = process.getBuiltinModule('node:sqlite');
  return SqliteModuleSchema.parse(rawModule).DatabaseSync;
}

function buildFtsQuery(rawQuery: string): string {
  const tokens = rawQuery
    .trim()
    .split(/\s+/)
    .map((t) => t.replace(/["'()*^:]/g, '').trim())
    .filter((t) => t.length > 0);
  if (tokens.length === 0) {
    return '';
  }
  return tokens.map((t) => `"${t}"`).join(' ');
}

function extractTextFromMessageContent(content: unknown): string {
  if (typeof content === 'string') {
    return content;
  }
  if (Array.isArray(content)) {
    const parts: string[] = [];
    for (const part of content) {
      if (typeof part === 'string') {
        parts.push(part);
      } else {
        const parsedPart = MessagePartTextSchema.safeParse(part);
        if (parsedPart.success) {
          parts.push(parsedPart.data.text);
        }
      }
    }
    return parts.join(' ');
  }
  return '';
}

/**
 * SQLite + FTS5 persistent cross-session full-text search index backed by
 * Node's built-in `node:sqlite` (zero native dependencies).
 */
export class SessionSearchIndex {
  private readonly db: SqliteDatabaseLike;

  constructor(
    dbPath: string = path.join(
      Storage.getGlobalM31aDir(),
      'sessions-fts.sqlite',
    ),
  ) {
    const DatabaseSync = loadDatabaseSync();
    this.db = new DatabaseSync(dbPath);
    if (dbPath !== ':memory:') {
      try {
        this.db.exec('PRAGMA journal_mode = WAL;');
      } catch {
        // Ignore WAL errors on constrained filesystems.
      }
    }
    this.db.exec(`
      PRAGMA synchronous = NORMAL;
      CREATE TABLE IF NOT EXISTS sessions_meta (
        session_id TEXT PRIMARY KEY,
        project_root TEXT NOT NULL,
        summary TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        file_path TEXT NOT NULL DEFAULT '',
        mtime_ms REAL NOT NULL DEFAULT 0
      );
      CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
        session_id UNINDEXED,
        project_root,
        summary,
        content,
        tokenize='porter unicode61'
      );
    `);
  }

  static async create(
    dbPath: string = path.join(
      Storage.getGlobalM31aDir(),
      'sessions-fts.sqlite',
    ),
  ): Promise<SessionSearchIndex> {
    if (dbPath !== ':memory:') {
      await fs.mkdir(path.dirname(dbPath), { recursive: true });
    }
    return new SessionSearchIndex(dbPath);
  }

  getIndexedMtime(sessionId: string): number | undefined {
    const row = this.db
      .prepare('SELECT mtime_ms FROM sessions_meta WHERE session_id = ?')
      .get(sessionId);
    const parsed = MetaRowSchema.safeParse(row);
    return parsed.success ? parsed.data.mtime_ms : undefined;
  }

  upsertSession(doc: SessionIndexDocument): void {
    this.db.exec('BEGIN');
    try {
      this.db
        .prepare('DELETE FROM sessions_fts WHERE session_id = ?')
        .run(doc.sessionId);
      this.db
        .prepare(
          `INSERT INTO sessions_meta (session_id, project_root, summary, updated_at, file_path, mtime_ms)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT(session_id) DO UPDATE SET
             project_root = excluded.project_root,
             summary = excluded.summary,
             updated_at = excluded.updated_at,
             file_path = excluded.file_path,
             mtime_ms = excluded.mtime_ms`,
        )
        .run(
          doc.sessionId,
          doc.projectRoot,
          doc.summary,
          doc.updatedAt,
          doc.filePath ?? '',
          doc.mtimeMs ?? 0,
        );
      this.db
        .prepare(
          `INSERT INTO sessions_fts (session_id, project_root, summary, content)
           VALUES (?, ?, ?, ?)`,
        )
        .run(doc.sessionId, doc.projectRoot, doc.summary, doc.content);
      this.db.exec('COMMIT');
    } catch (error) {
      this.db.exec('ROLLBACK');
      throw error;
    }
  }

  removeSession(sessionId: string): void {
    this.db.exec('BEGIN');
    try {
      this.db
        .prepare('DELETE FROM sessions_fts WHERE session_id = ?')
        .run(sessionId);
      this.db
        .prepare('DELETE FROM sessions_meta WHERE session_id = ?')
        .run(sessionId);
      this.db.exec('COMMIT');
    } catch (error) {
      this.db.exec('ROLLBACK');
      throw error;
    }
  }

  async indexDirectory(chatsDir: string, projectRoot = ''): Promise<number> {
    let entries: string[];
    try {
      entries = await fs.readdir(chatsDir);
    } catch {
      return 0;
    }

    let indexedCount = 0;
    for (const fileName of entries) {
      if (!fileName.startsWith('session-') || !fileName.endsWith('.json')) {
        continue;
      }
      const fullPath = path.join(chatsDir, fileName);
      try {
        const stat = await fs.stat(fullPath);
        const fallbackId = fileName.slice(0, -'.json'.length);
        const existingMtime = this.getIndexedMtime(fallbackId);
        if (
          existingMtime !== undefined &&
          Math.abs(existingMtime - stat.mtimeMs) < 1
        ) {
          continue;
        }
        const raw = await fs.readFile(fullPath, 'utf-8');
        const parsedJson: unknown = JSON.parse(raw);
        const parsed = StoredSessionRecordSchema.safeParse(parsedJson);
        if (!parsed.success) {
          continue;
        }
        const record = parsed.data;
        const sessionId = record.sessionId ?? fallbackId;
        const messageTexts = (record.messages ?? [])
          .map((m) => extractTextFromMessageContent(m.content))
          .filter((text) => text.trim().length > 0);
        const summary =
          record.summary?.trim() || messageTexts[0]?.slice(0, 120) || sessionId;
        const content = messageTexts.join('\n');

        this.upsertSession({
          sessionId,
          projectRoot,
          summary,
          content,
          updatedAt:
            record.lastUpdated ?? record.startTime ?? stat.mtime.toISOString(),
          filePath: fullPath,
          mtimeMs: stat.mtimeMs,
        });
        indexedCount++;
      } catch {
        // Skip corrupt session files without failing directory indexing.
      }
    }
    return indexedCount;
  }

  search(
    rawQuery: string,
    options?: { limit?: number; projectRoot?: string },
  ): SessionSearchHit[] {
    const ftsQuery = buildFtsQuery(rawQuery);
    if (!ftsQuery) {
      return [];
    }
    const limit = Math.max(1, Math.min(50, options?.limit ?? 15));
    const hasProjectFilter = Boolean(options?.projectRoot?.trim());
    const sql = hasProjectFilter
      ? `SELECT
           f.session_id AS session_id,
           m.project_root AS project_root,
           m.summary AS summary,
           snippet(sessions_fts, 3, '[', ']', '...', 18) AS snippet,
           m.updated_at AS updated_at,
           bm25(sessions_fts) AS score
         FROM sessions_fts f
         JOIN sessions_meta m ON m.session_id = f.session_id
         WHERE sessions_fts MATCH ? AND m.project_root = ?
         ORDER BY bm25(sessions_fts) ASC
         LIMIT ?`
      : `SELECT
           f.session_id AS session_id,
           m.project_root AS project_root,
           m.summary AS summary,
           snippet(sessions_fts, 3, '[', ']', '...', 18) AS snippet,
           m.updated_at AS updated_at,
           bm25(sessions_fts) AS score
         FROM sessions_fts f
         JOIN sessions_meta m ON m.session_id = f.session_id
         WHERE sessions_fts MATCH ?
         ORDER BY bm25(sessions_fts) ASC
         LIMIT ?`;

    const stmt = this.db.prepare(sql);
    const rows =
      hasProjectFilter && options?.projectRoot
        ? stmt.all(ftsQuery, options.projectRoot.trim(), limit)
        : stmt.all(ftsQuery, limit);

    const hits: SessionSearchHit[] = [];
    for (const row of rows) {
      const parsed = SearchRowSchema.safeParse(row);
      if (parsed.success) {
        hits.push({
          sessionId: parsed.data.session_id,
          projectRoot: parsed.data.project_root,
          summary: parsed.data.summary,
          snippet: parsed.data.snippet,
          updatedAt: parsed.data.updated_at,
          score: parsed.data.score,
        });
      }
    }
    return hits;
  }

  close(): void {
    this.db.close();
  }
}
