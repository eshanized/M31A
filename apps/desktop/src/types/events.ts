/**
 * Strongly-typed presentation-independent RuntimeEvent definitions for M31A Desktop.
 * Mirrors serde serialization in `crates/m31a-runtime/src/events.rs`.
 */

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

export type RuntimeEvent =
  | { type: 'initializing' }
  | { type: 'ready' }
  | { type: 'shutting_down' }
  | { type: 'shutdown_complete'; data: { report: Record<string, unknown> } }
  | { type: 'runtime_error'; data: { error: string } }
  | { type: 'thread_created'; data: { thread_id: string; title: string } }
  | { type: 'turn_submitted'; data: { thread_id: string; turn_id: string } }
  | { type: 'turn_admitted'; data: { thread_id: string; turn_id: string } }
  | {
      type: 'turn_completed';
      data: { thread_id: string; turn_id: string; usage: TokenUsage };
    }
  | { type: 'turn_cancelled'; data: { thread_id: string; turn_id: string } }
  | {
      type: 'turn_failed';
      data: { thread_id: string; turn_id: string; error: string };
    }
  | {
      type: 'reasoning_delta';
      data: { thread_id: string; turn_id: string; delta: string };
    }
  | {
      type: 'content_delta';
      data: { thread_id: string; turn_id: string; delta: string };
    }
  | {
      type: 'tool_started';
      data: {
        thread_id: string;
        turn_id: string;
        call_id: string;
        tool_name: string;
        arguments: string;
      };
    }
  | {
      type: 'tool_finished';
      data: {
        thread_id: string;
        turn_id: string;
        call_id: string;
        tool_name: string;
        output: string;
        success: boolean;
      };
    }
  | {
      type: 'approval_requested';
      data: {
        thread_id: string;
        turn_id: string;
        request_id: string;
        command: string;
        reason: string;
      };
    }
  | {
      type: 'question_requested';
      data: {
        thread_id: string;
        turn_id: string;
        question_id: string;
        question: string;
        options: string[];
        multi_select: boolean;
      };
    }
  | { type: 'workspace_changed'; data: { paths: string[] } }
  | {
      type: 'model_changed';
      data: { old_model: string; new_model: string };
    }
  | {
      type: 'provider_status';
      data: { status: string; message: string };
    }
  | {
      type: 'verification_result';
      data: { check_name: string; passed: boolean; output: string };
    };
