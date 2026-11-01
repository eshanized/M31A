import { describe, it } from 'node:test';
import * as assert from 'node:assert';
import { ActiveTurn, TurnEvent } from '../turn.js';

describe('Turn Management and Streaming', () => {
  it('should stream content chunks and resolve result()', async () => {
    let steeredMsg = '';
    const bridge = {
      async steerTurn(msg: string) {
        steeredMsg = msg;
        return { steered: true, turn_id: 'turn_1' };
      },
      async interruptTurn() {
        return { interrupted: true, turn_id: 'turn_1' };
      },
      async approveTurn() {
        return { resolved: true };
      },
      async answerQuestion() {
        return { resolved: true };
      },
    };

    const turn = new ActiveTurn('turn_1', 'thread_1', 'started', 'lease_1', bridge);

    const chunks: string[] = [];
    turn.onContent((c) => chunks.push(c));

    turn.dispatchServerEvent('notification/turn_content_chunk', { turn_id: 'turn_1', chunk: 'Hello ' });
    turn.dispatchServerEvent('notification/turn_content_chunk', { turn_id: 'turn_1', chunk: 'M31A!' });
    turn.dispatchServerEvent('notification/turn_completed', { turn_id: 'turn_1', thread_id: 'thread_1' });

    assert.deepStrictEqual(chunks, ['Hello ', 'M31A!']);
    assert.strictEqual(turn.content, 'Hello M31A!');
    assert.strictEqual(await turn.result(), 'Hello M31A!');

    await turn.steer('Focus on tests');
    assert.strictEqual(steeredMsg, 'Focus on tests');
  });

  it('should support async iteration over turn events', async () => {
    const bridge = {
      async steerTurn() { return { steered: true, turn_id: 'turn_2' }; },
      async interruptTurn() { return { interrupted: true, turn_id: 'turn_2' }; },
      async approveTurn() { return { resolved: true }; },
      async answerQuestion() { return { resolved: true }; },
    };

    const turn = new ActiveTurn('turn_2', 'thread_1', 'started', undefined, bridge);

    setTimeout(() => {
      turn.dispatchServerEvent('notification/turn_reasoning_chunk', { turn_id: 'turn_2', chunk: 'Thinking...' });
      turn.dispatchServerEvent('notification/turn_content_chunk', { turn_id: 'turn_2', chunk: '42' });
      turn.dispatchServerEvent('notification/turn_completed', { turn_id: 'turn_2', thread_id: 'thread_1' });
    }, 10);

    const events: TurnEvent[] = [];
    for await (const event of turn) {
      events.push(event);
    }

    assert.strictEqual(events.length, 3);
    assert.strictEqual(events[0].type, 'reasoning');
    assert.strictEqual(events[1].type, 'content');
    assert.strictEqual(events[2].type, 'completed');
  });

  it('should handle automated approval and question callbacks', async () => {
    let approvalSent = false;
    let answerSent = false;

    const bridge = {
      async steerTurn() { return { steered: true, turn_id: 'turn_3' }; },
      async interruptTurn() { return { interrupted: true, turn_id: 'turn_3' }; },
      async approveTurn(approvalId: string, decision: any) {
        if (approvalId === 'appr_123' && decision.decision === 'allow') {
          approvalSent = true;
        }
        return { resolved: true };
      },
      async answerQuestion(qId: string, answers: string[]) {
        if (qId === 'q_123' && answers[0] === 'TypeScript') {
          answerSent = true;
        }
        return { resolved: true };
      },
    };

    const turn = new ActiveTurn('turn_3', 'thread_1', 'started', undefined, bridge);

    const callbacks = {
      onApprovalRequested: async (req: any) => ({ decision: 'allow' as const }),
      onQuestionRequested: async (req: any) => ['TypeScript'],
    };

    turn.dispatchServerEvent('notification/approval_requested', {
      approval_id: 'appr_123',
      turn_id: 'turn_3',
      thread_id: 'thread_1',
      tool: 'exec_command',
      arguments: '{"command": "ls"}',
      reason: 'Requires approval',
    }, callbacks);

    turn.dispatchServerEvent('notification/question_requested', {
      question_id: 'q_123',
      turn_id: 'turn_3',
      thread_id: 'thread_1',
      prompt: 'Pick language',
      options: ['TypeScript', 'Python'],
      is_multi_select: false,
    }, callbacks);

    // Yield control to let async callbacks resolve
    await new Promise((r) => setTimeout(r, 20));

    assert.strictEqual(approvalSent, true);
    assert.strictEqual(answerSent, true);
  });
});
