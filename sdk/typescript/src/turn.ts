/**
 * Active Turn Management and Streaming for M31A TypeScript SDK.
 */

import { EventEmitter } from 'events';
import {
  ApprovalRequest,
  ApprovalDecision,
  QuestionRequest,
  ToolStartedNotification,
  ToolFinishedNotification,
  NoticeNotification,
  GuardianDeniedNotification,
  CompactionNotification,
  TurnAdmissionStatus,
} from './types.js';
import { TimeoutError } from './errors.js';

export type TurnEvent =
  | { type: 'started'; turnId: string; threadId: string; leaseId?: string }
  | { type: 'content'; chunk: string }
  | { type: 'reasoning'; chunk: string }
  | { type: 'tool_started'; callId: string; tool: string; arguments: string }
  | { type: 'tool_finished'; callId: string; success: boolean; output: string }
  | { type: 'approval_requested'; request: ApprovalRequest }
  | { type: 'question_requested'; request: QuestionRequest }
  | { type: 'notice'; message: string; warning: boolean }
  | { type: 'guardian_denied'; rationale: string }
  | { type: 'compaction'; windowId: string; originalTokens: number; compactedTokens: number }
  | { type: 'completed'; message: string; usage?: Record<string, unknown> }
  | { type: 'failed'; error: string }
  | { type: 'interrupted' };

export interface TurnCallbacks {
  onContent?: (chunk: string) => void;
  onReasoning?: (chunk: string) => void;
  onToolStarted?: (ev: ToolStartedNotification) => void;
  onToolFinished?: (ev: ToolFinishedNotification) => void;
  onApprovalRequested?: (req: ApprovalRequest) => Promise<ApprovalDecision | void> | ApprovalDecision | void;
  onQuestionRequested?: (req: QuestionRequest) => Promise<string[] | void> | string[] | void;
  onNotice?: (ev: NoticeNotification) => void;
  onGuardianDenied?: (ev: GuardianDeniedNotification) => void;
  onCompaction?: (ev: CompactionNotification) => void;
}

export interface TurnClientBridge {
  steerTurn(message: string, turnId?: string, threadId?: string): Promise<{ steered: boolean; turn_id: string }>;
  interruptTurn(turnId?: string, threadId?: string): Promise<{ interrupted: boolean; turn_id: string }>;
  approveTurn(approvalId: string, decision?: ApprovalDecision): Promise<unknown>;
  answerQuestion(questionId: string, answers: string[]): Promise<unknown>;
}

/**
 * Controller representing an active or completed execution turn.
 */
export class ActiveTurn extends EventEmitter {
  public readonly turnId: string;
  public readonly threadId: string;
  public readonly leaseId?: string;
  public readonly status: TurnAdmissionStatus;

  private _completed = false;
  private _error: Error | null = null;
  private _finalMessage = '';
  private _content = '';
  private _reasoning = '';
  private _eventQueue: TurnEvent[] = [];
  private _waitingResolvers: Array<() => void> = [];

  constructor(
    turnId: string,
    threadId: string,
    status: TurnAdmissionStatus,
    leaseId: string | undefined,
    private bridge: TurnClientBridge
  ) {
    super();
    this.turnId = turnId;
    this.threadId = threadId;
    this.status = status;
    this.leaseId = leaseId;
  }

  get isCompleted(): boolean {
    return this._completed;
  }

  get content(): string {
    return this._content;
  }

  get reasoning(): string {
    return this._reasoning;
  }

  get finalMessage(): string {
    return this._finalMessage;
  }

  async steer(message: string): Promise<void> {
    await this.bridge.steerTurn(message, this.turnId, this.threadId);
  }

  async interrupt(): Promise<void> {
    await this.bridge.interruptTurn(this.turnId, this.threadId);
  }

  onContent(cb: (chunk: string) => void): this {
    return this.on('content', cb);
  }

  onReasoning(cb: (chunk: string) => void): this {
    return this.on('reasoning', cb);
  }

  onToolStarted(cb: (ev: ToolStartedNotification) => void): this {
    return this.on('tool_started', cb);
  }

  onToolFinished(cb: (ev: ToolFinishedNotification) => void): this {
    return this.on('tool_finished', cb);
  }

  onNotice(cb: (ev: NoticeNotification) => void): this {
    return this.on('notice', cb);
  }

  onGuardianDenied(cb: (ev: GuardianDeniedNotification) => void): this {
    return this.on('guardian_denied', cb);
  }

  /**
   * Waits for the turn to complete and returns the final assistant message.
   */
  async result(timeoutMs = 120000): Promise<string> {
    if (this._completed) {
      if (this._error) throw this._error;
      return this._finalMessage || this._content;
    }

    return new Promise<string>((resolve, reject) => {
      let settled = false;
      const timer = setTimeout(() => {
        if (!settled) {
          settled = true;
          reject(new TimeoutError(`Turn '${this.turnId}' did not finish within ${timeoutMs}ms`, timeoutMs));
        }
      }, timeoutMs);

      this.once('completed', (msg: string) => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          resolve(msg);
        }
      });

      this.once('failed', (err: Error) => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(err);
        }
      });

      this.once('interrupted', () => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          resolve(this._content);
        }
      });
    });
  }

  /**
   * Internal dispatcher for server notification events.
   */
  dispatchServerEvent(method: string, params: any, callbacks?: TurnCallbacks): void {
    if (this._completed && method !== 'notification/turn_finished') {
      return;
    }

    switch (method) {
      case 'notification/turn_content_chunk': {
        const chunk = params.chunk || '';
        this._content += chunk;
        this.emit('content', chunk);
        callbacks?.onContent?.(chunk);
        this.pushEvent({ type: 'content', chunk });
        break;
      }

      case 'notification/turn_reasoning_chunk': {
        const chunk = params.chunk || '';
        this._reasoning += chunk;
        this.emit('reasoning', chunk);
        callbacks?.onReasoning?.(chunk);
        this.pushEvent({ type: 'reasoning', chunk });
        break;
      }

      case 'notification/tool_started': {
        const ev: ToolStartedNotification = {
          turn_id: params.turn_id,
          call_id: params.call_id,
          tool: params.tool,
          arguments: params.arguments,
        };
        this.emit('tool_started', ev);
        callbacks?.onToolStarted?.(ev);
        this.pushEvent({
          type: 'tool_started',
          callId: ev.call_id,
          tool: ev.tool,
          arguments: ev.arguments,
        });
        break;
      }

      case 'notification/tool_finished': {
        const ev: ToolFinishedNotification = {
          turn_id: params.turn_id,
          call_id: params.call_id,
          success: params.success,
          output: params.output,
        };
        this.emit('tool_finished', ev);
        callbacks?.onToolFinished?.(ev);
        this.pushEvent({
          type: 'tool_finished',
          callId: ev.call_id,
          success: ev.success,
          output: ev.output,
        });
        break;
      }

      case 'notification/approval_requested': {
        const req: ApprovalRequest = params;
        this.emit('approval_requested', req);
        this.pushEvent({ type: 'approval_requested', request: req });
        if (callbacks?.onApprovalRequested) {
          Promise.resolve(callbacks.onApprovalRequested(req))
            .then((decision) => {
              if (decision) {
                return this.bridge.approveTurn(req.approval_id, decision);
              }
            })
            .catch((err) => {
              this.emit('error', err);
            });
        }
        break;
      }

      case 'notification/question_requested': {
        const req: QuestionRequest = params;
        this.emit('question_requested', req);
        this.pushEvent({ type: 'question_requested', request: req });
        if (callbacks?.onQuestionRequested) {
          Promise.resolve(callbacks.onQuestionRequested(req))
            .then((answers) => {
              if (answers && Array.isArray(answers)) {
                return this.bridge.answerQuestion(req.question_id, answers);
              }
            })
            .catch((err) => {
              this.emit('error', err);
            });
        }
        break;
      }

      case 'notification/notice': {
        const ev: NoticeNotification = params;
        this.emit('notice', ev);
        callbacks?.onNotice?.(ev);
        this.pushEvent({ type: 'notice', message: ev.message, warning: ev.warning });
        break;
      }

      case 'notification/guardian_denied': {
        const ev: GuardianDeniedNotification = params;
        this.emit('guardian_denied', ev);
        callbacks?.onGuardianDenied?.(ev);
        this.pushEvent({ type: 'guardian_denied', rationale: ev.rationale });
        break;
      }

      case 'notification/compaction': {
        const ev: CompactionNotification = params;
        this.emit('compaction', ev);
        callbacks?.onCompaction?.(ev);
        this.pushEvent({
          type: 'compaction',
          windowId: ev.window_id,
          originalTokens: ev.original_tokens,
          compactedTokens: ev.compacted_tokens,
        });
        break;
      }

      case 'notification/turn_message': {
        this._finalMessage = params.message || this._content;
        break;
      }

      case 'notification/turn_completed': {
        this._completed = true;
        const msg = this._finalMessage || this._content;
        this.emit('completed', msg);
        this.pushEvent({ type: 'completed', message: msg, usage: params.usage });
        break;
      }

      case 'notification/turn_failed': {
        this._completed = true;
        const errorMsg = params.error || 'Turn execution failed';
        this._error = new Error(errorMsg);
        this.emit('failed', this._error);
        this.pushEvent({ type: 'failed', error: errorMsg });
        break;
      }

      case 'notification/turn_interrupted': {
        this._completed = true;
        this.emit('interrupted');
        this.pushEvent({ type: 'interrupted' });
        break;
      }
    }
  }

  private pushEvent(event: TurnEvent): void {
    this._eventQueue.push(event);
    if (this._waitingResolvers.length > 0) {
      const resolver = this._waitingResolvers.shift();
      resolver?.();
    }
  }

  /**
   * Async iterator support for `for await (const event of turn)`.
   */
  async *[Symbol.asyncIterator](): AsyncIterableIterator<TurnEvent> {
    while (true) {
      while (this._eventQueue.length > 0) {
        const item = this._eventQueue.shift()!;
        yield item;
        if (item.type === 'completed' || item.type === 'failed' || item.type === 'interrupted') {
          return;
        }
      }

      if (this._completed) {
        return;
      }

      await new Promise<void>((resolve) => {
        this._waitingResolvers.push(resolve);
      });
    }
  }
}
