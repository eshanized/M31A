/**
 * Fluent Thread Handle abstraction for M31A TypeScript SDK.
 */

import {
  SessionMetadata,
  ThreadDetails,
  ThreadItem,
  ThreadGoal,
  CheckpointRecord,
  InboxItem,
  ClaimRecord,
} from './types.js';
import { ActiveTurn, TurnCallbacks, TurnEvent } from './turn.js';

export interface ThreadClientBridge {
  readThread(threadId: string): Promise<ThreadDetails>;
  archiveThread(threadId: string): Promise<unknown>;
  unarchiveThread(threadId: string): Promise<unknown>;
  deleteThread(threadId: string): Promise<unknown>;
  forkThread(threadId: string, title?: string): Promise<SessionMetadata>;
  resumeThread(threadId: string): Promise<SessionMetadata>;
  compactSession(threadId: string, keepRecentTurns?: number): Promise<{ compacted: boolean; summary: string | null }>;
  createCheckpoint(
    threadId: string,
    label: string,
    modifiedFiles?: string[],
    turnIndex?: number,
    gitCommit?: string
  ): Promise<CheckpointRecord>;
  listCheckpoints(threadId: string): Promise<CheckpointRecord[]>;
  restoreCheckpoint(checkpointId: string, revertConversation?: boolean): Promise<CheckpointRecord>;
  setGoal(threadId: string, objective: string, budget?: number): Promise<ThreadGoal>;
  getGoal(threadId: string): Promise<ThreadGoal | null>;
  clearGoal(threadId: string): Promise<void>;
  updateGoal(threadId: string, status: string): Promise<void>;
  moveThreadToSection(threadId: string, sectionId?: string): Promise<boolean>;
  enqueueInbox(threadId: string, payload: string, kind?: string): Promise<InboxItem>;
  dequeueInbox(threadId: string): Promise<InboxItem | null>;
  listInbox(threadId: string): Promise<InboxItem[]>;
  getActiveClaim(threadId: string): Promise<ClaimRecord | null>;
  runTurn(threadId: string, prompt: string, callbacks?: TurnCallbacks): Promise<ActiveTurn>;
}

/**
 * Handle wrapping a specific conversation session thread.
 */
export class ThreadHandle {
  constructor(public readonly id: string, private bridge: ThreadClientBridge) {}

  /**
   * Executes a turn in this thread and returns an ActiveTurn controller.
   */
  async run(prompt: string, callbacks?: TurnCallbacks): Promise<ActiveTurn> {
    return this.bridge.runTurn(this.id, prompt, callbacks);
  }

  /**
   * Runs a turn and streams its events via an AsyncIterable.
   */
  async *stream(prompt: string, callbacks?: TurnCallbacks): AsyncIterableIterator<TurnEvent> {
    const turn = await this.run(prompt, callbacks);
    for await (const event of turn) {
      yield event;
    }
  }

  /**
   * Reads metadata and items for this thread.
   */
  async read(): Promise<ThreadDetails> {
    return this.bridge.readThread(this.id);
  }

  /**
   * Returns conversation history items for this thread.
   */
  async history(): Promise<ThreadItem[]> {
    const details = await this.read();
    return details.items;
  }

  /**
   * Archives this thread.
   */
  async archive(): Promise<void> {
    await this.bridge.archiveThread(this.id);
  }

  /**
   * Unarchives this thread.
   */
  async unarchive(): Promise<void> {
    await this.bridge.unarchiveThread(this.id);
  }

  /**
   * Permanently deletes this thread.
   */
  async delete(): Promise<void> {
    await this.bridge.deleteThread(this.id);
  }

  /**
   * Forks this thread into a new branching session.
   */
  async fork(title?: string): Promise<ThreadHandle> {
    const forked = await this.bridge.forkThread(this.id, title);
    return new ThreadHandle(forked.id, this.bridge);
  }

  /**
   * Resumes or loads the thread.
   */
  async resume(): Promise<SessionMetadata> {
    return this.bridge.resumeThread(this.id);
  }

  /**
   * Compacts thread history, retaining the most recent turns.
   */
  async compact(keepRecentTurns = 6): Promise<string | null> {
    const res = await this.bridge.compactSession(this.id, keepRecentTurns);
    return res.summary;
  }

  /**
   * Creates a checkpoint snapshot of the workspace and conversation state.
   */
  async createCheckpoint(
    label: string,
    modifiedFiles: string[] = [],
    turnIndex = 1,
    gitCommit?: string
  ): Promise<CheckpointRecord> {
    return this.bridge.createCheckpoint(this.id, label, modifiedFiles, turnIndex, gitCommit);
  }

  /**
   * Lists all checkpoints associated with this thread.
   */
  async listCheckpoints(): Promise<CheckpointRecord[]> {
    return this.bridge.listCheckpoints(this.id);
  }

  /**
   * Restores session state to a previously saved checkpoint.
   */
  async restoreCheckpoint(checkpointId: string, revertConversation = true): Promise<CheckpointRecord> {
    return this.bridge.restoreCheckpoint(checkpointId, revertConversation);
  }

  /**
   * Sets or updates the active goal and optional token budget.
   */
  async setGoal(objective: string, budget?: number): Promise<ThreadGoal> {
    return this.bridge.setGoal(this.id, objective, budget);
  }

  /**
   * Retrieves the current goal.
   */
  async getGoal(): Promise<ThreadGoal | null> {
    return this.bridge.getGoal(this.id);
  }

  /**
   * Clears the current goal.
   */
  async clearGoal(): Promise<void> {
    await this.bridge.clearGoal(this.id);
  }

  /**
   * Updates goal status.
   */
  async updateGoalStatus(status: string): Promise<void> {
    await this.bridge.updateGoal(this.id, status);
  }

  /**
   * Moves this thread into an organizational section.
   */
  async moveToSection(sectionId?: string): Promise<boolean> {
    return this.bridge.moveThreadToSection(this.id, sectionId);
  }

  /**
   * Enqueues a message or instruction into this thread's inbox.
   */
  async enqueueInbox(payload: string, kind = 'prompt'): Promise<InboxItem> {
    return this.bridge.enqueueInbox(this.id, payload, kind);
  }

  /**
   * Dequeues the next item from this thread's inbox.
   */
  async dequeueInbox(): Promise<InboxItem | null> {
    return this.bridge.dequeueInbox(this.id);
  }

  /**
   * Lists all pending inbox items for this thread.
   */
  async listInbox(): Promise<InboxItem[]> {
    return this.bridge.listInbox(this.id);
  }

  /**
   * Retrieves any active worker or agent claim on this thread.
   */
  async getActiveClaim(): Promise<ClaimRecord | null> {
    return this.bridge.getActiveClaim(this.id);
  }
}
