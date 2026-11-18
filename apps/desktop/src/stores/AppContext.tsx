/**
 * Unified Application Context and State Management for M31A Desktop.
 *
 * Connects the presentation layer to the authoritative Rust runtime via
 * typed IPC commands and the `m31a://event` runtime event bridge.
 */

import React, {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  useRef,
} from 'react';
import { ipc } from '../lib/ipc';
import {
  subscribeToRuntimeEventGaps,
  subscribeToRuntimeEvents,
} from '../lib/events';
import type {
  EnvironmentInfoDto,
  ModelInfoDto,
  PendingApprovalDto,
  PendingQuestionDto,
  RuntimeStatusDto,
  SessionDetailsDto,
  SessionSummaryDto,
  WorkspaceInfoDto,
} from '../types/ipc';
import type { RuntimeEvent } from '../types/events';

export interface ToolCallItem {
  id: string;
  name: string;
  arguments: string;
  output?: string;
  status: 'running' | 'success' | 'failed';
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  reasoning?: string;
  toolCalls?: ToolCallItem[];
  status?: 'streaming' | 'completed' | 'cancelled' | 'failed';
  timestamp: number;
}

export interface AppContextValue {
  // Runtime State
  runtimeStatus: RuntimeStatusDto | null;
  environmentInfo: EnvironmentInfoDto | null;
  workspaceInfo: WorkspaceInfoDto | null;
  availableModels: ModelInfoDto[];
  activeModel: string;
  isRuntimeReady: boolean;
  runtimeError: string | null;

  // Session State
  sessions: SessionSummaryDto[];
  activeSessionId: string | null;
  activeSessionDetails: SessionDetailsDto | null;
  isLoadingSessions: boolean;

  // Chat & Execution State
  messages: ChatMessage[];
  runningTurnId: string | null;
  isSubmittingTurn: boolean;
  collaborationMode: 'default' | 'plan' | 'review';

  // Approvals & Questions
  pendingApprovals: PendingApprovalDto[];
  pendingQuestions: PendingQuestionDto[];

  // Actions
  refreshRuntime: () => Promise<void>;
  switchModel: (modelId: string) => Promise<void>;
  selectWorkspace: (path: string) => Promise<void>;
  authorizeCurrentWorkspace: () => Promise<void>;
  selectSession: (sessionId: string) => Promise<void>;
  createNewSession: (title?: string) => Promise<string | null>;
  submitTurn: (prompt: string) => Promise<void>;
  cancelActiveTurn: () => Promise<void>;
  steerActiveTurn: (message: string) => Promise<void>;
  setCollaborationMode: (mode: 'default' | 'plan' | 'review') => void;
  resolveApproval: (
    requestId: string,
    approved: boolean,
    reason?: string
  ) => Promise<void>;
  resolveQuestion: (questionId: string, answers: string[]) => Promise<void>;
}

const AppContext = createContext<AppContextValue | null>(null);

export const AppProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  // Runtime State
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatusDto | null>(
    null
  );
  const [environmentInfo, setEnvironmentInfo] =
    useState<EnvironmentInfoDto | null>(null);
  const [workspaceInfo, setWorkspaceInfo] = useState<WorkspaceInfoDto | null>(
    null
  );
  const [availableModels, setAvailableModels] = useState<ModelInfoDto[]>([]);
  const [activeModel, setActiveModel] = useState<string>('');
  const [runtimeError, setRuntimeError] = useState<string | null>(null);

  // Session State
  const [sessions, setSessions] = useState<SessionSummaryDto[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeSessionDetails, setActiveSessionDetails] =
    useState<SessionDetailsDto | null>(null);
  const [isLoadingSessions, setIsLoadingSessions] = useState<boolean>(false);

  // Chat State
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [runningTurnId, setRunningTurnId] = useState<string | null>(null);
  const [isSubmittingTurn, setIsSubmittingTurn] = useState<boolean>(false);
  const [collaborationMode, setCollaborationMode] = useState<
    'default' | 'plan' | 'review'
  >('default');

  // Approvals & Questions State
  const [pendingApprovals, setPendingApprovals] = useState<
    PendingApprovalDto[]
  >([]);
  const [pendingQuestions, setPendingQuestions] = useState<
    PendingQuestionDto[]
  >([]);

  // Ref to track latest state inside event listener callback
  const activeSessionIdRef = useRef<string | null>(null);
  activeSessionIdRef.current = activeSessionId;

  // Initial Data Fetch
  const refreshRuntime = useCallback(async () => {
    try {
      const [status, env, ws, models] = await Promise.all([
        ipc.getRuntimeStatus(),
        ipc.getEnvironmentInfo(),
        ipc.getActiveWorkspace(),
        ipc.listAvailableModels(),
      ]);

      setRuntimeStatus(status);
      setEnvironmentInfo(env);
      setWorkspaceInfo(ws);
      setAvailableModels(models);
      setActiveModel(status.active_model);
      if (status.active_session_id) {
        setActiveSessionId(status.active_session_id);
      }
      setRuntimeError(null);
    } catch (err) {
      console.error('Failed to refresh runtime:', err);
      setRuntimeError(String(err));
    }
  }, []);

  const refreshSessions = useCallback(async () => {
    try {
      setIsLoadingSessions(true);
      const list = await ipc.listSessions();
      setSessions(list);
    } catch (err) {
      console.error('Failed to list sessions:', err);
    } finally {
      setIsLoadingSessions(false);
    }
  }, []);

  const refreshPendingInteractions = useCallback(async () => {
    try {
      const pending = await ipc.getPendingInteractions();
      setPendingApprovals(pending.approvals);
      setPendingQuestions(pending.questions);
    } catch (err) {
      console.error('Failed to reconcile pending runtime interactions:', err);
    }
  }, []);

  const selectSession = useCallback(async (sessionId: string) => {
    try {
      const details = await ipc.loadSession(sessionId);
      setActiveSessionId(details.id);
      setActiveSessionDetails(details);
      setWorkspaceInfo({
        path: details.cwd,
        is_trusted: true,
        exists: true,
      });
      setMessages(
        details.messages.map((message, index) => ({
          id: message.id,
          role: message.role,
          content: message.content,
          timestamp: details.updated_at * 1000 + index,
          toolCalls: [],
        }))
      );
      setRunningTurnId(null);
      setPendingApprovals((prev) =>
        prev.filter((approval) => approval.thread_id === details.id)
      );
      setPendingQuestions((prev) =>
        prev.filter((question) => question.thread_id === details.id)
      );
    } catch (err) {
      console.error(`Failed to load session ${sessionId}:`, err);
      throw err;
    }
  }, []);

  const createNewSession = useCallback(
    async (title?: string): Promise<string | null> => {
      try {
        const session = await ipc.createSession(title);
        await refreshSessions();
        await selectSession(session.id);
        return session.id;
      } catch (err) {
        console.error('Failed to create session:', err);
        return null;
      }
    },
    [refreshSessions, selectSession]
  );

  const switchModel = useCallback(async (modelId: string) => {
    try {
      const updatedModel = await ipc.activateModel(modelId);
      setActiveModel(updatedModel);
    } catch (err) {
      console.error('Failed to switch model:', err);
      throw err;
    }
  }, []);

  const selectWorkspace = useCallback(async (path: string) => {
    try {
      const ws = await ipc.setWorkspace(path);
      setWorkspaceInfo(ws);
    } catch (err) {
      console.error('Failed to set workspace:', err);
      throw err;
    }
  }, []);

  const authorizeCurrentWorkspace = useCallback(async () => {
    if (!workspaceInfo?.path) return;
    try {
      const ws = await ipc.authorizeWorkspace(workspaceInfo.path);
      setWorkspaceInfo(ws);
    } catch (err) {
      console.error('Failed to authorize workspace:', err);
      throw err;
    }
  }, [workspaceInfo?.path]);

  // Handle Runtime Event Bridge Stream
  const handleRuntimeEvent = useCallback((event: RuntimeEvent) => {
    // Every conversation-scoped event must match the currently selected session.
    const eventThreadId =
      'data' in event && 'thread_id' in event.data
        ? event.data.thread_id
        : undefined;
    if (
      eventThreadId &&
      activeSessionIdRef.current &&
      eventThreadId !== activeSessionIdRef.current
    ) {
      return;
    }

    switch (event.type) {
      case 'initializing':
        setRuntimeStatus((prev) => (prev ? { ...prev, state: 'Initializing' } : null));
        break;

      case 'ready':
        setRuntimeStatus((prev) => (prev ? { ...prev, state: 'Ready', is_operational: true } : null));
        break;

      case 'runtime_error':
        setRuntimeError(event.data.error);
        break;

      case 'turn_submitted':
      case 'turn_admitted': {
        const { thread_id, turn_id } = event.data;
        if (!activeSessionIdRef.current) {
          setActiveSessionId(thread_id);
        }
        setRunningTurnId(turn_id);
        setMessages((prev) => {
          const assistantId = `assistant-${turn_id}`;
          if (prev.some((message) => message.id === assistantId)) {
            return prev;
          }
          return [
            ...prev,
            {
              id: assistantId,
              role: 'assistant',
              content: '',
              reasoning: '',
              toolCalls: [],
              status: 'streaming',
              timestamp: Date.now(),
            },
          ];
        });
        break;
      }

      case 'content_delta': {
        const { turn_id, delta } = event.data;
        setMessages((prev) => {
          return prev.map((msg) => {
            if (msg.role === 'assistant' && msg.id === `assistant-${turn_id}`) {
              return {
                ...msg,
                content: msg.content + delta,
              };
            }
            return msg;
          });
        });
        break;
      }

      case 'reasoning_delta': {
        const { turn_id, delta } = event.data;
        setMessages((prev) => {
          return prev.map((msg) => {
            if (msg.role === 'assistant' && (msg.id === `assistant-${turn_id}`)) {
              return {
                ...msg,
                reasoning: (msg.reasoning || '') + delta,
              };
            }
            return msg;
          });
        });
        break;
      }

      case 'tool_started': {
        const { turn_id, call_id, tool_name, arguments: args } = event.data;
        setMessages((prev) => {
          return prev.map((msg) => {
            if (msg.role === 'assistant' && (msg.id === `assistant-${turn_id}`)) {
              const currentTools = msg.toolCalls || [];
              const updatedTools: ToolCallItem[] = [
                ...currentTools,
                {
                  id: call_id,
                  name: tool_name,
                  arguments: args,
                  status: 'running',
                },
              ];
              return { ...msg, toolCalls: updatedTools };
            }
            return msg;
          });
        });
        break;
      }

      case 'tool_finished': {
        const { turn_id, call_id, output, success } = event.data;
        setMessages((prev) => {
          return prev.map((msg) => {
            if (msg.role === 'assistant' && (msg.id === `assistant-${turn_id}`)) {
              const updatedTools = (msg.toolCalls || []).map((tc) => {
                if (tc.id === call_id) {
                  return {
                    ...tc,
                    output,
                    status: success ? ('success' as const) : ('failed' as const),
                  };
                }
                return tc;
              });
              return { ...msg, toolCalls: updatedTools };
            }
            return msg;
          });
        });
        break;
      }

      case 'turn_completed': {
        const { turn_id } = event.data;
        setRunningTurnId((current) => (current === turn_id ? null : current));
        setPendingApprovals((prev) =>
          prev.filter((approval) => approval.turn_id !== turn_id)
        );
        setPendingQuestions((prev) =>
          prev.filter((question) => question.turn_id !== turn_id)
        );
        setMessages((prev) =>
          prev.map((msg) => {
            if (msg.role === 'assistant' && (msg.id === `assistant-${turn_id}`)) {
              return { ...msg, status: 'completed' };
            }
            return msg;
          })
        );
        break;
      }

      case 'turn_cancelled': {
        const { turn_id } = event.data;
        setRunningTurnId((current) => (current === turn_id ? null : current));
        setPendingApprovals((prev) =>
          prev.filter((approval) => approval.turn_id !== turn_id)
        );
        setPendingQuestions((prev) =>
          prev.filter((question) => question.turn_id !== turn_id)
        );
        setMessages((prev) =>
          prev.map((msg) => {
            if (msg.role === 'assistant' && msg.id === `assistant-${turn_id}`) {
              return { ...msg, status: 'cancelled' };
            }
            return msg;
          })
        );
        break;
      }

      case 'turn_failed': {
        const { turn_id, error } = event.data;
        setRunningTurnId((current) => (current === turn_id ? null : current));
        setPendingApprovals((prev) =>
          prev.filter((approval) => approval.turn_id !== turn_id)
        );
        setPendingQuestions((prev) =>
          prev.filter((question) => question.turn_id !== turn_id)
        );
        setMessages((prev) =>
          prev.map((msg) => {
            if (msg.role === 'assistant' && (msg.id === `assistant-${turn_id}`)) {
              return {
                ...msg,
                status: 'failed',
                content: msg.content ? `${msg.content}\n\n[Error: ${error}]` : `[Error: ${error}]`,
              };
            }
            return msg;
          })
        );
        break;
      }

      case 'approval_requested': {
        const { request_id, thread_id, turn_id, command, reason } = event.data;
        setPendingApprovals((prev) => [
          ...prev.filter((a) => a.request_id !== request_id),
          { request_id, thread_id, turn_id, command, reason },
        ]);
        break;
      }

      case 'question_requested': {
        const { question_id, thread_id, turn_id, question, options, multi_select } = event.data;
        setPendingQuestions((prev) => [
          ...prev.filter((q) => q.question_id !== question_id),
          { question_id, thread_id, turn_id, question, options, multi_select },
        ]);
        break;
      }

      case 'model_changed': {
        setActiveModel(event.data.new_model);
        break;
      }

      case 'workspace_changed': {
        ipc.getActiveWorkspace().then(setWorkspaceInfo).catch(console.error);
        break;
      }

      default:
        break;
    }
  }, []);

  // Submit Turn to Runtime
  const submitTurn = useCallback(
    async (prompt: string) => {
      const trimmed = prompt.trim();
      if (!trimmed || isSubmittingTurn || runningTurnId) return;

      setIsSubmittingTurn(true);
      const userMessageId = `user-${Date.now()}`;

      // Append user message immediately to the timeline
      setMessages((prev) => [
        ...prev,
        {
          id: userMessageId,
          role: 'user',
          content: trimmed,
          timestamp: Date.now(),
        },
      ]);

      try {
        const handle = await ipc.submitTurn({
          thread_id: activeSessionId,
          prompt: trimmed,
          collaboration_mode: collaborationMode,
        });

        if (!activeSessionId) {
          setActiveSessionId(handle.thread_id);
          refreshSessions();
        }
        setRunningTurnId(handle.turn_id);
      } catch (err) {
        console.error('Failed to submit turn:', err);
        setMessages((prev) => [
          ...prev,
          {
            id: `err-${Date.now()}`,
            role: 'system',
            content: `Failed to execute turn: ${String(err)}`,
            timestamp: Date.now(),
          },
        ]);
      } finally {
        setIsSubmittingTurn(false);
      }
    },
    [
      isSubmittingTurn,
      runningTurnId,
      activeSessionId,
      collaborationMode,
      refreshSessions,
    ]
  );

  const cancelActiveTurn = useCallback(async () => {
    if (!activeSessionId || !runningTurnId) return;
    try {
      await ipc.cancelTurn(activeSessionId, runningTurnId);
    } catch (err) {
      console.error('Failed to cancel turn:', err);
    }
  }, [activeSessionId, runningTurnId]);

  const steerActiveTurn = useCallback(
    async (message: string) => {
      if (!activeSessionId || !runningTurnId) return;
      try {
        await ipc.steerTurn(activeSessionId, runningTurnId, message);
      } catch (err) {
        console.error('Failed to steer turn:', err);
      }
    },
    [activeSessionId, runningTurnId]
  );

  const resolveApproval = useCallback(
    async (requestId: string, approved: boolean, reason?: string) => {
      try {
        await ipc.respondApproval(requestId, approved, reason);
        setPendingApprovals((prev) =>
          prev.filter((a) => a.request_id !== requestId)
        );
      } catch (err) {
        console.error('Failed to respond to approval:', err);
        throw err;
      }
    },
    []
  );

  const resolveQuestion = useCallback(
    async (questionId: string, answers: string[]) => {
      try {
        await ipc.respondQuestion(questionId, answers);
        setPendingQuestions((prev) =>
          prev.filter((q) => q.question_id !== questionId)
        );
      } catch (err) {
        console.error('Failed to respond to question:', err);
        throw err;
      }
    },
    []
  );

  // Subscribe to runtime events and reconcile snapshots after event loss.
  useEffect(() => {
    let mounted = true;
    let unlistenEvents: (() => void) | undefined;
    let unlistenGaps: (() => void) | undefined;

    refreshRuntime();
    refreshSessions();

    subscribeToRuntimeEvents(handleRuntimeEvent)
      .then((unlisten) => {
        if (mounted) {
          unlistenEvents = unlisten;
          // Subscribe first, then reconcile to close the reconnect race.
          refreshPendingInteractions();
        } else {
          unlisten();
        }
      })
      .catch((err) => {
        console.error('Failed to initialize runtime event bridge listener:', err);
      });

    subscribeToRuntimeEventGaps((skipped) => {
      console.warn(`Runtime event stream dropped ${skipped} events; reconciling pending interactions.`);
      refreshPendingInteractions();
      refreshRuntime();
    })
      .then((unlisten) => {
        if (mounted) {
          unlistenGaps = unlisten;
        } else {
          unlisten();
        }
      })
      .catch((err) => {
        console.error('Failed to initialize runtime event-gap listener:', err);
      });

    return () => {
      mounted = false;
      unlistenEvents?.();
      unlistenGaps?.();
    };
  }, [
    refreshRuntime,
    refreshSessions,
    refreshPendingInteractions,
    handleRuntimeEvent,
  ]);

  const isRuntimeReady =
    runtimeStatus?.is_operational === true && !runtimeError;

  return (
    <AppContext.Provider
      value={{
        runtimeStatus,
        environmentInfo,
        workspaceInfo,
        availableModels,
        activeModel,
        isRuntimeReady,
        runtimeError,
        sessions,
        activeSessionId,
        activeSessionDetails,
        isLoadingSessions,
        messages,
        runningTurnId,
        isSubmittingTurn,
        collaborationMode,
        pendingApprovals,
        pendingQuestions,
        refreshRuntime,
        switchModel,
        selectWorkspace,
        authorizeCurrentWorkspace,
        selectSession,
        createNewSession,
        submitTurn,
        cancelActiveTurn,
        steerActiveTurn,
        setCollaborationMode,
        resolveApproval,
        resolveQuestion,
      }}
    >
      {children}
    </AppContext.Provider>
  );
};

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error('useApp must be used within an AppProvider');
  }
  return ctx;
}
