/**
 * Transport Layer for M31A TypeScript SDK.
 *
 * Implements Unix Domain Socket (UDS), Child Process Standard I/O (stdio),
 * HTTP/SSE, and Mock transports with line-delimited JSON-RPC framing.
 */

import * as net from 'net';
import * as child_process from 'child_process';
import * as http from 'http';
import * as https from 'https';
import { EventEmitter } from 'events';
import { JsonRpcRequest, JsonRpcResponse, JsonRpcNotification } from './types.js';
import { TransportError } from './errors.js';

export interface TransportEvents {
  response: (res: JsonRpcResponse) => void;
  notification: (notif: JsonRpcNotification) => void;
  error: (err: Error) => void;
  close: () => void;
}

export interface Transport extends EventEmitter {
  connect(): Promise<void>;
  send(req: JsonRpcRequest): Promise<void>;
  close(): Promise<void>;
  isConnected(): boolean;

  on<U extends keyof TransportEvents>(event: U, listener: TransportEvents[U]): this;
  emit<U extends keyof TransportEvents>(event: U, ...args: Parameters<TransportEvents[U]>): boolean;
}

/**
 * Unix Domain Socket Transport for M31A App Server.
 */
export class UdsTransport extends EventEmitter implements Transport {
  private socket: net.Socket | null = null;
  private buffer = '';
  private connected = false;

  constructor(private socketPath: string, private connectTimeoutMs = 10000) {
    super();
  }

  isConnected(): boolean {
    return this.connected && this.socket !== null && !this.socket.destroyed;
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      let settled = false;
      const timer = setTimeout(() => {
        if (!settled) {
          settled = true;
          if (this.socket) {
            this.socket.destroy();
            this.socket = null;
          }
          reject(new TransportError(`Connection to UDS socket '${this.socketPath}' timed out after ${this.connectTimeoutMs}ms`));
        }
      }, this.connectTimeoutMs);

      try {
        this.socket = net.createConnection(this.socketPath, () => {
          if (!settled) {
            settled = true;
            clearTimeout(timer);
            this.connected = true;
            resolve();
          }
        });

        this.socket.on('data', (data: Buffer) => {
          this.buffer += data.toString('utf-8');
          this.processBuffer();
        });

        this.socket.on('error', (err: Error) => {
          this.emit('error', new TransportError(`Socket error on '${this.socketPath}': ${err.message}`, err));
          if (!settled) {
            settled = true;
            clearTimeout(timer);
            reject(err);
          }
        });

        this.socket.on('close', () => {
          this.connected = false;
          this.emit('close');
        });
      } catch (err) {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(err);
        }
      }
    });
  }

  send(req: JsonRpcRequest): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.isConnected() || !this.socket) {
        return reject(new TransportError('Socket is not connected'));
      }
      const data = JSON.stringify(req) + '\n';
      this.socket.write(data, 'utf-8', (err) => {
        if (err) reject(new TransportError(`Failed to write frame to socket: ${err.message}`, err));
        else resolve();
      });
    });
  }

  close(): Promise<void> {
    return new Promise((resolve) => {
      this.connected = false;
      if (this.socket) {
        this.socket.end(() => {
          this.socket = null;
          resolve();
        });
      } else {
        resolve();
      }
    });
  }

  private processBuffer(): void {
    let newlineIndex = this.buffer.indexOf('\n');
    while (newlineIndex !== -1) {
      const line = this.buffer.substring(0, newlineIndex).trim();
      this.buffer = this.buffer.substring(newlineIndex + 1);

      if (line.length > 0) {
        try {
          const parsed = JSON.parse(line);
          if ('id' in parsed && parsed.id !== null && parsed.id !== undefined) {
            this.emit('response', parsed as JsonRpcResponse);
          } else if ('method' in parsed) {
            this.emit('notification', parsed as JsonRpcNotification);
          }
        } catch (e) {
          this.emit('error', new TransportError(`Failed to parse incoming frame: ${e}`));
        }
      }
      newlineIndex = this.buffer.indexOf('\n');
    }
  }
}

/**
 * Options for StdioTransport.
 */
export interface StdioTransportOptions {
  binaryPath?: string;
  args?: string[];
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  onStderr?: (chunk: string) => void;
}

/**
 * Stdio Transport spawning the M31A binary child process with app-server stdio.
 */
export class StdioTransport extends EventEmitter implements Transport {
  private child: child_process.ChildProcess | null = null;
  private buffer = '';
  private connected = false;

  constructor(private options: StdioTransportOptions = {}) {
    super();
  }

  isConnected(): boolean {
    return this.connected && this.child !== null && !this.child.killed && this.child.stdin !== null;
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const binaryPath = this.options.binaryPath || 'm31a';
      const args = this.options.args || ['app-server', 'stdio'];

      try {
        this.child = child_process.spawn(binaryPath, args, {
          cwd: this.options.cwd,
          env: { ...process.env, ...this.options.env },
          stdio: ['pipe', 'pipe', 'pipe'],
        });

        this.child.stdout?.on('data', (data: Buffer) => {
          this.buffer += data.toString('utf-8');
          this.processBuffer();
        });

        this.child.stderr?.on('data', (data: Buffer) => {
          const str = data.toString('utf-8');
          if (this.options.onStderr) {
            this.options.onStderr(str);
          }
        });

        this.child.on('error', (err: Error) => {
          this.connected = false;
          this.emit('error', new TransportError(`Child process error: ${err.message}`, err));
          reject(err);
        });

        this.child.on('close', (code, signal) => {
          this.connected = false;
          this.emit('close');
        });

        this.connected = true;
        resolve();
      } catch (err) {
        reject(err);
      }
    });
  }

  send(req: JsonRpcRequest): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.isConnected() || !this.child || !this.child.stdin) {
        return reject(new TransportError('Process stdin is not available'));
      }
      const data = JSON.stringify(req) + '\n';
      this.child.stdin.write(data, 'utf-8', (err) => {
        if (err) reject(new TransportError(`Failed to write to stdin: ${err.message}`, err));
        else resolve();
      });
    });
  }

  close(): Promise<void> {
    return new Promise((resolve) => {
      this.connected = false;
      if (this.child) {
        this.child.kill('SIGTERM');
        this.child = null;
      }
      resolve();
    });
  }

  private processBuffer(): void {
    let newlineIndex = this.buffer.indexOf('\n');
    while (newlineIndex !== -1) {
      const line = this.buffer.substring(0, newlineIndex).trim();
      this.buffer = this.buffer.substring(newlineIndex + 1);

      if (line.length > 0) {
        try {
          const parsed = JSON.parse(line);
          if ('id' in parsed && parsed.id !== null && parsed.id !== undefined) {
            this.emit('response', parsed as JsonRpcResponse);
          } else if ('method' in parsed) {
            this.emit('notification', parsed as JsonRpcNotification);
          }
        } catch (e) {
          this.emit('error', new TransportError(`Failed to parse frame: ${e}`));
        }
      }
      newlineIndex = this.buffer.indexOf('\n');
    }
  }
}

/**
 * Options for HttpTransport.
 */
export interface HttpTransportOptions {
  baseUrl: string; // e.g. "http://127.0.0.1:4096"
  enableSse?: boolean;
  ssePath?: string;
  rpcPath?: string;
}

/**
 * HTTP/SSE Transport for remote M31A App Server HTTP Gateways.
 */
export class HttpTransport extends EventEmitter implements Transport {
  private baseUrl: URL;
  private sseReq: http.ClientRequest | null = null;
  private connected = false;
  private sseBuffer = '';

  constructor(private options: HttpTransportOptions) {
    super();
    let urlStr = options.baseUrl;
    if (!urlStr.startsWith('http://') && !urlStr.startsWith('https://')) {
      urlStr = 'http://' + urlStr;
    }
    this.baseUrl = new URL(urlStr);
  }

  isConnected(): boolean {
    return this.connected;
  }

  async connect(): Promise<void> {
    this.connected = true;
    if (this.options.enableSse !== false) {
      this.connectSse();
    }
  }

  private connectSse(): void {
    const sseUrl = new URL(this.options.ssePath || '/events', this.baseUrl);
    const client = sseUrl.protocol === 'https:' ? https : http;

    this.sseReq = client.get(sseUrl, (res) => {
      if (res.statusCode !== 200) {
        this.emit('error', new TransportError(`SSE connection failed with status: ${res.statusCode}`));
        return;
      }

      res.on('data', (chunk: Buffer) => {
        this.sseBuffer += chunk.toString('utf-8');
        this.processSseBuffer();
      });

      res.on('end', () => {
        if (this.connected) {
          // Reconnect SSE after a small delay if still marked connected
          setTimeout(() => {
            if (this.connected) this.connectSse();
          }, 1000);
        }
      });
    });

    this.sseReq.on('error', (err) => {
      this.emit('error', new TransportError(`SSE request error: ${err.message}`, err));
    });
  }

  private processSseBuffer(): void {
    let boundaryIndex = this.sseBuffer.indexOf('\n\n');
    while (boundaryIndex !== -1) {
      const eventBlock = this.sseBuffer.substring(0, boundaryIndex).trim();
      this.sseBuffer = this.sseBuffer.substring(boundaryIndex + 2);

      let eventType = 'message';
      let dataStr = '';

      for (const line of eventBlock.split('\n')) {
        const trimmed = line.trim();
        if (trimmed.startsWith('event:')) {
          eventType = trimmed.substring(6).trim();
        } else if (trimmed.startsWith('data:')) {
          dataStr += trimmed.substring(5).trim();
        }
      }

      if (dataStr.length > 0) {
        try {
          const parsed = JSON.parse(dataStr);
          if (parsed && typeof parsed === 'object') {
            if ('method' in parsed) {
              this.emit('notification', parsed as JsonRpcNotification);
            } else {
              this.emit('notification', {
                jsonrpc: '2.0',
                method: eventType,
                params: parsed,
              });
            }
          }
        } catch {
          // Non-JSON SSE payload
        }
      }

      boundaryIndex = this.sseBuffer.indexOf('\n\n');
    }
  }

  async send(req: JsonRpcRequest): Promise<void> {
    const rpcUrl = new URL(this.options.rpcPath || '/rpc', this.baseUrl);
    const body = JSON.stringify(req);
    const client = rpcUrl.protocol === 'https:' ? https : http;

    return new Promise((resolve, reject) => {
      const postReq = client.request(
        rpcUrl,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Content-Length': Buffer.byteLength(body),
          },
        },
        (res) => {
          let resData = '';
          res.on('data', (chunk) => {
            resData += chunk;
          });
          res.on('end', () => {
            try {
              const parsed = JSON.parse(resData);
              if ('id' in parsed && parsed.id !== null) {
                this.emit('response', parsed as JsonRpcResponse);
              }
              resolve();
            } catch (e) {
              reject(new TransportError(`Failed to parse HTTP response: ${e}`));
            }
          });
        }
      );

      postReq.on('error', (err) => {
        reject(new TransportError(`HTTP RPC error: ${err.message}`, err));
      });

      postReq.write(body);
      postReq.end();
    });
  }

  async close(): Promise<void> {
    this.connected = false;
    if (this.sseReq) {
      this.sseReq.destroy();
      this.sseReq = null;
    }
    this.emit('close');
  }
}

/**
 * Mock Transport for unit tests and local simulation.
 */
export class MockTransport extends EventEmitter implements Transport {
  public sentRequests: JsonRpcRequest[] = [];
  private connected = false;
  private handlers = new Map<string, (params: unknown) => unknown>();

  isConnected(): boolean {
    return this.connected;
  }

  async connect(): Promise<void> {
    this.connected = true;
  }

  registerHandler(method: string, handler: (params: unknown) => unknown): void {
    this.handlers.set(method, handler);
  }

  async send(req: JsonRpcRequest): Promise<void> {
    this.sentRequests.push(req);
    const handler = this.handlers.get(req.method);

    process.nextTick(() => {
      if (handler) {
        try {
          const result = handler(req.params);
          this.emit('response', {
            jsonrpc: '2.0',
            id: req.id,
            result,
          });
        } catch (e: any) {
          this.emit('response', {
            jsonrpc: '2.0',
            id: req.id,
            error: {
              code: -32603,
              message: e?.message || String(e),
            },
          });
        }
      } else {
        this.emit('response', {
          jsonrpc: '2.0',
          id: req.id,
          result: { ok: true },
        });
      }
    });
  }

  simulateNotification(method: string, params: unknown): void {
    this.emit('notification', {
      jsonrpc: '2.0',
      method,
      params,
    });
  }

  async close(): Promise<void> {
    this.connected = false;
    this.emit('close');
  }
}
