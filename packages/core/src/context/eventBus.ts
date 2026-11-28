/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { EventEmitter } from 'node:events';
import type { ConcreteNode } from './graph/types.js';

export interface ContextConsolidationEvent {
  nodes: readonly ConcreteNode[];
  targetDeficit: number;
  targetNodeIds: Set<string>;
}

export interface ChunkReceivedEvent {
  nodes: readonly ConcreteNode[];
  targetNodeIds: Set<string>;
}

export interface TokenGroundTruthEvent {
  actualTokens: number;
  promptBaseUnits: number;
}

export interface NormalizeNeededEvent {
  nodes: readonly ConcreteNode[];
  targetDeficit: number;
  targetNodeIds: Set<string>;
}

export class ContextEventBus extends EventEmitter {
  emitTokenGroundTruth(event: TokenGroundTruthEvent) {
    this.emit('TOKEN_GROUND_TRUTH', event);
  }

  onTokenGroundTruth(listener: (event: TokenGroundTruthEvent) => void) {
    this.on('TOKEN_GROUND_TRUTH', listener);
  }

  emitChunkReceived(event: ChunkReceivedEvent) {
    this.emit('IR_CHUNK_RECEIVED', event);
  }

  onChunkReceived(listener: (event: ChunkReceivedEvent) => void) {
    this.on('IR_CHUNK_RECEIVED', listener);
  }

  emitConsolidationNeeded(event: ContextConsolidationEvent) {
    this.emit('BUDGET_RETAINED_CROSSED', event);
  }

  onConsolidationNeeded(listener: (event: ContextConsolidationEvent) => void) {
    this.on('BUDGET_RETAINED_CROSSED', listener);
  }

  emitNormalizeNeeded(event: NormalizeNeededEvent) {
    this.emit('BUDGET_NORMALIZED_CROSSED', event);
  }

  onNormalizeNeeded(listener: (event: NormalizeNeededEvent) => void) {
    this.on('BUDGET_NORMALIZED_CROSSED', listener);
  }
}
