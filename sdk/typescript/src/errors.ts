/**
 * Error hierarchy for M31A TypeScript SDK.
 */

/**
 * Base class for all M31A SDK errors.
 */
export class M31AError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'M31AError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/**
 * Represents a standard JSON-RPC 2.0 protocol error returned by the server.
 */
export class JsonRpcError extends M31AError {
  constructor(
    public readonly code: number,
    message: string,
    public readonly data?: unknown
  ) {
    super(`JSON-RPC Error [${code}]: ${message}`);
    this.name = 'JsonRpcError';
  }
}

/**
 * Thrown when an underlying transport (UDS, Stdio, HTTP) fails or disconnects.
 */
export class TransportError extends M31AError {
  constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = 'TransportError';
  }
}

/**
 * Thrown when an RPC request or active turn exceeds its allotted timeout.
 */
export class TimeoutError extends M31AError {
  constructor(message: string, public readonly timeoutMs?: number) {
    super(message);
    this.name = 'TimeoutError';
  }
}

/**
 * Thrown when a turn admission request is rejected (e.g. system busy or queue full).
 */
export class TurnRejectedError extends M31AError {
  constructor(public readonly reason: string) {
    super(`Turn admission rejected: ${reason}`);
    this.name = 'TurnRejectedError';
  }
}

/**
 * Thrown when an action or tool call is denied by Guardian review or policy.
 */
export class ApprovalDeniedError extends M31AError {
  constructor(public readonly rationale: string) {
    super(`Action denied: ${rationale}`);
    this.name = 'ApprovalDeniedError';
  }
}
