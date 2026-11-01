/**
 * M31A App Server TypeScript Client.
 *
 * Implements high-level JSON-RPC client bindings for the M31A daemon,
 * managing session lifecycles, admitting turns, handling streaming notifications,
 * managing interactive approvals/questions, checkpoints, and multi-agent boards.
 */

import { EventEmitter } from 'events';
import * as os from 'os';
import * as path from 'path';
import * as fs from 'fs';
import {
  Transport,
  UdsTransport,
  StdioTransport,
  HttpTransport,
} from './transport.js';
import {
  JsonRpcRequest,
  JsonRpcResponse,
  JsonRpcNotification,
  InitializeResult,
  DiagnosticsResult,
  AppConfig,
  SessionMetadata,
  ThreadDetails,
  ThreadSection,
  CreateThreadParams,
  StartTurnResult,
  FinishTurnResult,
  SteerTurnResult,
  InterruptTurnResult,
  ApprovalDecision,
  ApprovalResult,
  QuestionResponse,
  CheckpointRecord,
  LockRecord,
  AcquireLockParams,
  AcquireLockResult,
  ReleaseLockResult,
  BoardPost,
  ThreadGoal,
  InboxItem,
  ClaimRecord,
  ModelInfo,
  SkillDefinition,
  InstructionSource,
  ToolDefinition,
  NotificationPayloadMap,
  LockMode,
} from './types.js';
import {
  M31AError,
  JsonRpcError,
  TimeoutError,
  TurnRejectedError,
} from './errors.js';
import { ActiveTurn, TurnCallbacks, TurnClientBridge } from './turn.js';
import { ThreadHandle, ThreadClientBridge } from './thread.js';

export interface M31AClientOptions {
  transport?: Transport;
  socketPath?: string;
  url?: string;
  binaryPath?: string;
  args?: string[];
  timeoutMs?: number;
}

export class M31AClient extends EventEmitter implements TurnClientBridge, ThreadClientBridge {
  private transport: Transport;
  private nextId = 1;
  private pending = new Map<
    number | string,
    {
      resolve: (val: any) => void;
      reject: (err: any) => void;
      timer: NodeJS.Timeout;
    }
  >();
  private defaultTimeoutMs: number;
  private activeTurns = new Map<string, ActiveTurn>();

  constructor(options: M31AClientOptions = {}) {
    super();
    this.defaultTimeoutMs = options.timeoutMs ?? 30000;

    if (options.transport) {
      this.transport = options.transport;
    } else if (options.url || process.env.M31A_URL) {
      const baseUrl = options.url || process.env.M31A_URL!;
      this.transport = new HttpTransport({ baseUrl });
    } else if (options.socketPath || process.env.M31A_SOCKET) {
      const socket = options.socketPath || process.env.M31A_SOCKET!;
      this.transport = new UdsTransport(socket);
    } else {
      const home = os.homedir();
      const defaultSocket = path.join(home, '.m31a', 'app-server-control', 'app-server-control.sock');
      const tempSocket = path.join(os.tmpdir(), 'm31a-daemon.sock');

      if (fs.existsSync(tempSocket)) {
        this.transport = new UdsTransport(tempSocket);
      } else {
        this.transport = new UdsTransport(defaultSocket);
      }
    }

    this.transport.on('response', (res: JsonRpcResponse) => {
      if (res.id !== undefined && res.id !== null) {
        const handler = this.pending.get(res.id);
        if (handler) {
          clearTimeout(handler.timer);
          this.pending.delete(res.id);
          if (res.error) {
            handler.reject(new JsonRpcError(res.error.code, res.error.message, res.error.data));
          } else {
            handler.resolve(res.result);
          }
        }
      }
    });

    this.transport.on('notification', (notif: JsonRpcNotification) => {
      this.handleNotification(notif);
    });

    this.transport.on('error', (err: Error) => {
      this.emit('error', err);
    });

    this.transport.on('close', () => {
      this.emit('close');
    });
  }

  private handleNotification(notif: JsonRpcNotification): void {
    this.emit('notification', notif);
    this.emit(notif.method, notif.params);

    const params: any = notif.params || {};
    const turnId = params.turn_id;

    if (turnId && this.activeTurns.has(turnId)) {
      const turn = this.activeTurns.get(turnId)!;
      turn.dispatchServerEvent(notif.method, params);

      if (
        notif.method === 'notification/turn_completed' ||
        notif.method === 'notification/turn_failed' ||
        notif.method === 'notification/turn_interrupted'
      ) {
        this.activeTurns.delete(turnId);
      }
    } else if (params.thread_id) {
      // Find active turn matching thread_id if turn_id not in params
      for (const [tid, turn] of this.activeTurns.entries()) {
        if (turn.threadId === params.thread_id) {
          turn.dispatchServerEvent(notif.method, params);
          if (
            notif.method === 'notification/turn_completed' ||
            notif.method === 'notification/turn_failed' ||
            notif.method === 'notification/turn_interrupted'
          ) {
            this.activeTurns.delete(tid);
          }
          break;
        }
      }
    }
  }

  async connect(): Promise<InitializeResult> {
    await this.transport.connect();
    return this.initialize();
  }

  async close(): Promise<void> {
    for (const [id, req] of this.pending.entries()) {
      clearTimeout(req.timer);
      req.reject(new M31AError('Client connection closed'));
    }
    this.pending.clear();
    await this.transport.close();
  }

  async request<T = unknown>(method: string, params?: unknown, timeoutMs?: number): Promise<T> {
    const id = this.nextId++;
    const req: JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method,
      params,
    };

    const effectiveTimeout = timeoutMs ?? this.defaultTimeoutMs;

    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new TimeoutError(`Request '${method}' timed out after ${effectiveTimeout}ms`, effectiveTimeout));
      }, effectiveTimeout);

      this.pending.set(id, { resolve, reject, timer });

      this.transport.send(req).catch((err) => {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(err);
      });
    });
  }

  // --- Core Lifecycle & Discovery RPCs ---

  async initialize(clientName = 'm31a-typescript-sdk', clientVersion = '0.1.13'): Promise<InitializeResult> {
    return this.request<InitializeResult>('initialize', {
      client_name: clientName,
      client_version: clientVersion,
    });
  }

  async getDiagnostics(): Promise<DiagnosticsResult> {
    return this.request<DiagnosticsResult>('server/diagnostics');
  }

  async readConfig(): Promise<AppConfig> {
    return this.request<AppConfig>('config/read');
  }

  async listModels(): Promise<ModelInfo[]> {
    return this.request<ModelInfo[]>('models/list');
  }

  async listSkills(): Promise<SkillDefinition[]> {
    return this.request<SkillDefinition[]>('skills/list');
  }

  async listInstructions(cwd?: string): Promise<InstructionSource[]> {
    return this.request<InstructionSource[]>('instructions/list', cwd ? { cwd } : {});
  }

  async listTools(): Promise<ToolDefinition[]> {
    return this.request<ToolDefinition[]>('tools/list');
  }

  async execCommand(command: string, cwd?: string): Promise<{ output: string }> {
    return this.request<{ output: string }>('command/exec', {
      command,
      cwd,
    });
  }

  // --- Session & Thread Management ---

  thread(threadId: string): ThreadHandle {
    return new ThreadHandle(threadId, this);
  }

  async createThread(options: CreateThreadParams = {}): Promise<ThreadHandle> {
    const meta = await this.request<SessionMetadata>('thread/create', options);
    return new ThreadHandle(meta.id, this);
  }

  async readThread(threadId: string): Promise<ThreadDetails> {
    return this.request<ThreadDetails>('thread/read', { thread_id: threadId });
  }

  async listThreads(limit = 50): Promise<SessionMetadata[]> {
    return this.request<SessionMetadata[]>('thread/list', { limit });
  }

  async archiveThread(threadId: string): Promise<{ archived: boolean; thread_id: string }> {
    return this.request<{ archived: boolean; thread_id: string }>('thread/archive', { thread_id: threadId });
  }

  async unarchiveThread(threadId: string): Promise<{ unarchived: boolean; thread_id: string }> {
    return this.request<{ unarchived: boolean; thread_id: string }>('thread/unarchive', { thread_id: threadId });
  }

  async deleteThread(threadId: string): Promise<{ deleted: boolean; thread_id: string }> {
    return this.request<{ deleted: boolean; thread_id: string }>('thread/delete', { thread_id: threadId });
  }

  async forkThread(threadId: string, title = 'Forked Session'): Promise<SessionMetadata> {
    return this.request<SessionMetadata>('thread/fork', {
      thread_id: threadId,
      title,
    });
  }

  async resumeThread(threadId: string): Promise<SessionMetadata> {
    return this.request<SessionMetadata>('thread/resume', { thread_id: threadId });
  }

  async compactSession(threadId: string, keepRecentTurns = 6): Promise<{ compacted: boolean; summary: string | null }> {
    return this.request<{ compacted: boolean; summary: string | null }>('session/compact', {
      thread_id: threadId,
      keep_recent_turns: keepRecentTurns,
    });
  }

  // --- Thread Sections ---

  async createThreadSection(name: string, color?: string, icon?: string): Promise<ThreadSection> {
    return this.request<ThreadSection>('thread/section/create', { name, color, icon });
  }

  async listThreadSections(): Promise<ThreadSection[]> {
    return this.request<ThreadSection[]>('thread/section/list');
  }

  async renameThreadSection(
    sectionId: string,
    name: string,
    color?: string,
    icon?: string
  ): Promise<{ renamed: boolean; section_id: string }> {
    return this.request<{ renamed: boolean; section_id: string }>('thread/section/rename', {
      section_id: sectionId,
      name,
      color,
      icon,
    });
  }

  async deleteThreadSection(sectionId: string): Promise<{ deleted: boolean; section_id: string }> {
    return this.request<{ deleted: boolean; section_id: string }>('thread/section/delete', {
      section_id: sectionId,
    });
  }

  async moveThreadToSection(threadId: string, sectionId?: string): Promise<boolean> {
    const res = await this.request<{ moved: boolean }>('thread/section/move', {
      thread_id: threadId,
      section_id: sectionId,
    });
    return res.moved;
  }

  async listSectionThreads(sectionId?: string, limit = 50): Promise<SessionMetadata[]> {
    return this.request<SessionMetadata[]>('thread/section/threads', {
      section_id: sectionId,
      limit,
    });
  }

  // --- Turn Execution & Admissions ---

  async startTurn(threadId: string, prompt: string): Promise<StartTurnResult> {
    const res = await this.request<StartTurnResult>('turn/start', {
      thread_id: threadId,
      prompt,
    });

    if (res.status === 'steered_into_active_turn' || (res as any).rejected) {
      if ((res as any).rejected) {
        throw new TurnRejectedError((res as any).reason || 'Admission rejected');
      }
    }

    return res;
  }

  async finishTurn(threadId: string, leaseId: string): Promise<FinishTurnResult> {
    return this.request<FinishTurnResult>('turn/finish', {
      thread_id: threadId,
      lease_id: leaseId,
    });
  }

  /**
   * High-level turn runner that admits a turn, attaches streaming callbacks,
   * registers the ActiveTurn instance, and returns the controller.
   */
  async runTurn(threadId: string, prompt: string, callbacks?: TurnCallbacks): Promise<ActiveTurn> {
    const res = await this.startTurn(threadId, prompt);
    const turnId = res.turn_id || `turn_${Date.now()}`;
    const turn = new ActiveTurn(turnId, threadId, res.status, res.lease_id, this);

    this.activeTurns.set(turnId, turn);

    if (callbacks) {
      if (callbacks.onContent) turn.onContent(callbacks.onContent);
      if (callbacks.onReasoning) turn.onReasoning(callbacks.onReasoning);
      if (callbacks.onToolStarted) turn.onToolStarted(callbacks.onToolStarted);
      if (callbacks.onToolFinished) turn.onToolFinished(callbacks.onToolFinished);
      if (callbacks.onNotice) turn.onNotice(callbacks.onNotice);
      if (callbacks.onGuardianDenied) turn.onGuardianDenied(callbacks.onGuardianDenied);
      if (callbacks.onApprovalRequested) {
        turn.on('approval_requested', (req: any) => {
          Promise.resolve(callbacks.onApprovalRequested!(req)).then((decision) => {
            if (decision) this.approveTurn(req.approval_id, decision);
          });
        });
      }
      if (callbacks.onQuestionRequested) {
        turn.on('question_requested', (req: any) => {
          Promise.resolve(callbacks.onQuestionRequested!(req)).then((answers) => {
            if (answers && Array.isArray(answers)) this.answerQuestion(req.question_id, answers);
          });
        });
      }
    }

    return turn;
  }

  async steerTurn(message: string, turnId?: string, threadId?: string): Promise<SteerTurnResult> {
    return this.request<SteerTurnResult>('turn/steer', {
      message,
      turn_id: turnId,
      thread_id: threadId,
    });
  }

  async interruptTurn(turnId?: string, threadId?: string): Promise<InterruptTurnResult> {
    return this.request<InterruptTurnResult>('turn/interrupt', {
      turn_id: turnId,
      thread_id: threadId,
    });
  }

  async approveTurn(approvalId: string, decision: ApprovalDecision = { decision: 'allow' }): Promise<ApprovalResult> {
    return this.request<ApprovalResult>('turn/approve', {
      approval_id: approvalId,
      decision: decision.decision || 'allow',
      reason: decision.reason,
      command: decision.command,
      pattern: decision.pattern,
    });
  }

  async denyTurn(approvalId: string, reason = 'Denied by client'): Promise<ApprovalResult> {
    return this.request<ApprovalResult>('turn/deny', {
      approval_id: approvalId,
      reason,
    });
  }

  async answerQuestion(questionId: string, answers: string[]): Promise<QuestionResponse> {
    return this.request<QuestionResponse>('turn/answer', {
      question_id: questionId,
      answers,
    });
  }

  // --- Checkpoints ---

  async createCheckpoint(
    threadId: string,
    label: string,
    modifiedFiles: string[] = [],
    turnIndex = 1,
    gitCommit?: string
  ): Promise<CheckpointRecord> {
    return this.request<CheckpointRecord>('checkpoint/create', {
      thread_id: threadId,
      label,
      turn_index: turnIndex,
      modified_files: modifiedFiles,
      git_commit: gitCommit,
    });
  }

  async listCheckpoints(threadId: string): Promise<CheckpointRecord[]> {
    return this.request<CheckpointRecord[]>('checkpoint/list', { thread_id: threadId });
  }

  async restoreCheckpoint(checkpointId: string, revertConversation = true): Promise<CheckpointRecord> {
    return this.request<CheckpointRecord>('checkpoint/restore', {
      checkpoint_id: checkpointId,
      revert_conversation: revertConversation,
    });
  }

  // --- Path Concurrency Locks ---

  async listLocks(): Promise<LockRecord[]> {
    return this.request<LockRecord[]>('locks/list');
  }

  async acquireLock(
    pathOrParams: string | AcquireLockParams,
    mode: LockMode = 'read',
    holder = 'typescript-sdk',
    timeoutMs = 5000
  ): Promise<AcquireLockResult> {
    let params: AcquireLockParams;
    if (typeof pathOrParams === 'string') {
      params = {
        path: pathOrParams,
        mode,
        holder,
        timeout_ms: timeoutMs,
      };
    } else {
      params = pathOrParams;
    }
    return this.request<AcquireLockResult>('locks/acquire', params);
  }

  async releaseLock(token: string): Promise<ReleaseLockResult> {
    return this.request<ReleaseLockResult>('locks/release', { token });
  }

  // --- Multi-Agent Message Board ---

  async postToBoard(
    body: string,
    channel = 'general',
    author = 'agent',
    payload: Record<string, unknown> = {}
  ): Promise<{ post_id: string }> {
    return this.request<{ post_id: string }>('board/post', {
      channel,
      author,
      body,
      payload,
    });
  }

  async listBoardPosts(channel = 'general', limit = 50): Promise<BoardPost[]> {
    return this.request<BoardPost[]>('board/list', { channel, limit });
  }

  // --- Goals & Budgeting ---

  async setGoal(threadId: string, objective: string, budget?: number): Promise<ThreadGoal> {
    return this.request<ThreadGoal>('goal/set', {
      thread_id: threadId,
      objective,
      budget,
    });
  }

  async getGoal(threadId: string): Promise<ThreadGoal | null> {
    return this.request<ThreadGoal | null>('goal/get', { thread_id: threadId });
  }

  async clearGoal(threadId: string): Promise<void> {
    await this.request<{ cleared: boolean }>('goal/clear', { thread_id: threadId });
  }

  async updateGoal(threadId: string, status: string): Promise<void> {
    await this.request<{ updated: boolean }>('goal/update', { thread_id: threadId, status });
  }

  // --- Session Inboxes ---

  async enqueueInbox(threadId: string, payload: string, kind = 'prompt'): Promise<InboxItem> {
    return this.request<InboxItem>('inbox/enqueue', {
      thread_id: threadId,
      kind,
      payload,
    });
  }

  async dequeueInbox(threadId: string): Promise<InboxItem | null> {
    return this.request<InboxItem | null>('inbox/dequeue', { thread_id: threadId });
  }

  async listInbox(threadId: string): Promise<InboxItem[]> {
    return this.request<InboxItem[]>('inbox/list', { thread_id: threadId });
  }

  // --- Active Claims & Recovery ---

  async getActiveClaim(threadId: string): Promise<ClaimRecord | null> {
    return this.request<ClaimRecord | null>('claims/active', { thread_id: threadId });
  }

  async recoverClaims(): Promise<ClaimRecord[]> {
    return this.request<ClaimRecord[]>('claims/recover');
  }

  // --- Strongly Typed Event Subscription ---

  onEvent<K extends keyof NotificationPayloadMap & string>(
    event: K,
    listener: (params: NotificationPayloadMap[K]) => void
  ): this {
    return this.on(event, listener as (...args: any[]) => void);
  }
}
