import { describe, it, beforeEach, afterEach } from 'node:test';
import * as assert from 'node:assert';
import { M31AClient } from '../client.js';
import { MockTransport } from '../transport.js';
import { JsonRpcError, TimeoutError } from '../errors.js';

describe('M31AClient', () => {
  let transport: MockTransport;
  let client: M31AClient;

  beforeEach(async () => {
    transport = new MockTransport();
    client = new M31AClient({ transport, timeoutMs: 1000 });

    transport.registerHandler('initialize', () => ({
      server_name: 'm31a-app-server',
      version: '0.1.13',
      protocol_version: '2.0',
      capabilities: { threads: true, turns: true, code_mode: true, guardian_review: true },
    }));

    transport.registerHandler('server/diagnostics', () => ({
      status: 'healthy',
      api_key_configured: true,
      bubblewrap_available: true,
      active_model: 'nvidia/nemotron-3.5-lightning-30b-a3b',
      sandbox_policy: 'WorkspaceWrite',
      approval_policy: 'OnRequest',
    }));

    transport.registerHandler('config/read', () => ({
      model: 'nvidia/nemotron-3.5-lightning-30b-a3b',
      api_base: 'https://integrate.api.nvidia.com/v1',
      sandbox: 'workspace-write',
      approval: 'on-request',
    }));

    transport.registerHandler('models/list', () => [
      {
        slug: 'nvidia/nemotron-3.5-lightning-30b-a3b',
        display_name: 'Nemotron 3.5 Lightning',
        description: 'Frontier reasoning model',
        context_window: 131072,
        max_output_tokens: 8192,
        supports_tools: true,
        supports_thinking: true,
        supports_vision: false,
        supports_audio: false,
        originator: 'nvidia',
      },
    ]);

    transport.registerHandler('thread/create', (params: any) => ({
      id: 'thread_xyz123',
      title: 'New Session',
      model: params?.model || 'nvidia/nemotron-3.5-lightning-30b-a3b',
      cwd: params?.cwd || '.',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }));

    transport.registerHandler('thread/read', (params: any) => ({
      thread: {
        id: params.thread_id,
        title: 'New Session',
        model: 'nvidia/nemotron-3.5-lightning-30b-a3b',
        cwd: '.',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      items: [
        {
          id: 'item_1',
          thread_id: params.thread_id,
          role: 'user',
          content: 'Hello M31A',
          created_at: new Date().toISOString(),
        },
      ],
    }));

    transport.registerHandler('thread/section/create', (params: any) => ({
      id: 'sec_1',
      name: params.name,
      color: params.color,
      icon: params.icon,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }));

    transport.registerHandler('checkpoint/create', (params: any) => ({
      id: 'cp_1',
      thread_id: params.thread_id,
      label: params.label,
      turn_index: params.turn_index,
      modified_files: params.modified_files,
      created_at: new Date().toISOString(),
    }));

    transport.registerHandler('locks/acquire', (params: any) => ({
      token: 'lock_token_abc',
      path: params.path,
      mode: params.mode,
    }));

    transport.registerHandler('locks/release', (params: any) => ({
      token: params.token,
      released: true,
    }));

    transport.registerHandler('board/post', (params: any) => ({
      post_id: 'post_999',
    }));

    transport.registerHandler('board/list', () => [
      {
        id: 'post_999',
        board: 'default',
        channel: 'general',
        author: 'agent',
        body: 'Mission accomplished',
        created_at: new Date().toISOString(),
      },
    ]);

    transport.registerHandler('goal/set', (params: any) => ({
      id: 'goal_1',
      thread_id: params.thread_id,
      objective: params.objective,
      budget: params.budget,
      status: 'in_progress',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }));

    transport.registerHandler('inbox/enqueue', (params: any) => ({
      id: 'inbox_1',
      thread_id: params.thread_id,
      kind: params.kind,
      payload: params.payload,
      created_at: new Date().toISOString(),
    }));

    transport.registerHandler('inbox/dequeue', (params: any) => ({
      id: 'inbox_1',
      thread_id: params.thread_id,
      kind: 'prompt',
      payload: 'Deferred user instruction',
      created_at: new Date().toISOString(),
    }));

    transport.registerHandler('turn/start', (params: any) => ({
      turn_id: 'turn_test_1',
      thread_id: params.thread_id,
      lease_id: 'lease_abc',
      status: 'started',
    }));

    await client.connect();
  });

  afterEach(async () => {
    await client.close();
  });

  it('should initialize and fetch diagnostics, config, and models', async () => {
    const diag = await client.getDiagnostics();
    assert.strictEqual(diag.status, 'healthy');
    assert.strictEqual(diag.api_key_configured, true);

    const config = await client.readConfig();
    assert.strictEqual(config.model, 'nvidia/nemotron-3.5-lightning-30b-a3b');

    const models = await client.listModels();
    assert.strictEqual(models.length, 1);
    assert.strictEqual(models[0].slug, 'nvidia/nemotron-3.5-lightning-30b-a3b');
  });

  it('should manage thread lifecycle and thread handles', async () => {
    const thread = await client.createThread({ cwd: '/workspace' });
    assert.strictEqual(thread.id, 'thread_xyz123');

    const details = await thread.read();
    assert.strictEqual(details.thread.id, 'thread_xyz123');
    assert.strictEqual(details.items.length, 1);

    const history = await thread.history();
    assert.strictEqual(history[0].content, 'Hello M31A');
  });

  it('should create and list thread sections', async () => {
    const section = await client.createThreadSection('Audits', '#ff0000', 'shield');
    assert.strictEqual(section.name, 'Audits');
    assert.strictEqual(section.color, '#ff0000');
  });

  it('should create checkpoints and manage path locks', async () => {
    const cp = await client.createCheckpoint('thread_xyz123', 'Feature Snapshot', ['src/index.ts']);
    assert.strictEqual(cp.id, 'cp_1');
    assert.strictEqual(cp.label, 'Feature Snapshot');

    const lock = await client.acquireLock('crates/m31a-core', 'write', 'test-runner');
    assert.strictEqual(lock.token, 'lock_token_abc');

    const release = await client.releaseLock(lock.token);
    assert.strictEqual(release.released, true);
  });

  it('should post to multi-agent board and query goals/inboxes', async () => {
    const post = await client.postToBoard('Task completed', 'ops');
    assert.strictEqual(post.post_id, 'post_999');

    const posts = await client.listBoardPosts('ops');
    assert.strictEqual(posts.length, 1);
    assert.strictEqual(posts[0].body, 'Mission accomplished');

    const goal = await client.setGoal('thread_xyz123', 'Pass all integration tests', 10000);
    assert.strictEqual(goal.objective, 'Pass all integration tests');

    const inboxItem = await client.enqueueInbox('thread_xyz123', 'Run linter next');
    assert.strictEqual(inboxItem.payload, 'Run linter next');

    const dequeued = await client.dequeueInbox('thread_xyz123');
    assert.strictEqual(dequeued?.payload, 'Deferred user instruction');
  });

  it('should run streaming turn with fluent active turn handle', async () => {
    const thread = client.thread('thread_xyz123');
    const chunks: string[] = [];

    const turn = await thread.run('Explain quantum entanglement', {
      onContent: (chunk) => chunks.push(chunk),
    });

    assert.strictEqual(turn.turnId, 'turn_test_1');
    assert.strictEqual(turn.status, 'started');

    // Simulate streaming events from app-server
    transport.simulateNotification('notification/turn_content_chunk', {
      turn_id: 'turn_test_1',
      chunk: 'Entanglement is ',
    });
    transport.simulateNotification('notification/turn_content_chunk', {
      turn_id: 'turn_test_1',
      chunk: 'a quantum phenomenon.',
    });
    transport.simulateNotification('notification/turn_completed', {
      turn_id: 'turn_test_1',
      thread_id: 'thread_xyz123',
    });

    const finalRes = await turn.result();
    assert.strictEqual(finalRes, 'Entanglement is a quantum phenomenon.');
    assert.strictEqual(chunks.join(''), 'Entanglement is a quantum phenomenon.');
  });

  it('should throw JsonRpcError on server error', async () => {
    transport.registerHandler('faulty_method', () => {
      throw new Error('Something went wrong');
    });

    await assert.rejects(
      async () => {
        await client.request('faulty_method');
      },
      (err: any) => {
        assert.ok(err instanceof JsonRpcError);
        assert.strictEqual(err.code, -32603);
        assert.ok(err.message.includes('Something went wrong'));
        return true;
      }
    );
  });
});
