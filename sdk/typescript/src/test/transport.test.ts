import { describe, it } from 'node:test';
import * as assert from 'node:assert';
import { MockTransport } from '../transport.js';

describe('Transport Layer', () => {
  it('should send requests and receive responses using MockTransport', async () => {
    const transport = new MockTransport();
    await transport.connect();
    assert.strictEqual(transport.isConnected(), true);

    transport.registerHandler('echo', (params: any) => ({
      echoed: params.message,
    }));

    const responsePromise = new Promise<any>((resolve) => {
      transport.on('response', (res) => resolve(res));
    });

    await transport.send({
      jsonrpc: '2.0',
      id: 101,
      method: 'echo',
      params: { message: 'hello world' },
    });

    const res = await responsePromise;
    assert.strictEqual(res.id, 101);
    assert.deepStrictEqual(res.result, { echoed: 'hello world' });

    await transport.close();
    assert.strictEqual(transport.isConnected(), false);
  });

  it('should emit notifications received from transport', async () => {
    const transport = new MockTransport();
    await transport.connect();

    const notifPromise = new Promise<any>((resolve) => {
      transport.on('notification', (notif) => resolve(notif));
    });

    transport.simulateNotification('notification/test_event', { foo: 'bar' });

    const notif = await notifPromise;
    assert.strictEqual(notif.method, 'notification/test_event');
    assert.deepStrictEqual(notif.params, { foo: 'bar' });

    await transport.close();
  });
});
