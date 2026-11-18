/**
 * Runtime Event Bridge for M31A Desktop.
 * Listens to the authoritative event stream emitted by Tauri backend on `m31a://event`.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { RuntimeEvent } from '../types/events';

export const EVENT_CHANNEL_NAME = 'm31a://event';
export const EVENT_GAP_CHANNEL_NAME = 'm31a://event-gap';

export type RuntimeEventListener = (event: RuntimeEvent) => void;

export interface RuntimeSubscription {
  ready: Promise<void>;
  dispose: UnlistenFn;
}

/**
 * Registers a listener and returns a synchronous disposer. If disposal occurs
 * before Tauri finishes registering the listener, the eventual unlisten
 * callback is invoked immediately when it becomes available.
 */
export function subscribeToRuntimeEvents(
  listener: RuntimeEventListener
): RuntimeSubscription {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;

  const ready = listen<RuntimeEvent>(EVENT_CHANNEL_NAME, (tauriEvent) => {
    try {
      listener(tauriEvent.payload);
    } catch (err) {
      console.error('Error in runtime event handler:', err, tauriEvent.payload);
    }
  }).then((removeListener) => {
    if (disposed) {
      removeListener();
    } else {
      unlisten = removeListener;
    }
  });

  return {
    ready,
    dispose: () => {
      disposed = true;
      unlisten?.();
      unlisten = undefined;
    },
  };
}

/**
 * Subscribes to event-loss notifications. The consumer should re-fetch
 * authoritative runtime state and pending interactions after a gap.
 */
export function subscribeToRuntimeEventGaps(
  listener: (skipped: number) => void
): RuntimeSubscription {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;

  const ready = listen<number>(EVENT_GAP_CHANNEL_NAME, (event) => {
    listener(event.payload);
  }).then((removeListener) => {
    if (disposed) {
      removeListener();
    } else {
      unlisten = removeListener;
    }
  });

  return {
    ready,
    dispose: () => {
      disposed = true;
      unlisten?.();
      unlisten = undefined;
    },
  };
}
