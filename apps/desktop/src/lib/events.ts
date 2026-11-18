/**
 * Runtime Event Bridge for M31A Desktop.
 * Listens to the authoritative event stream emitted by Tauri backend on `m31a://event`.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { RuntimeEvent } from '../types/events';

export const EVENT_CHANNEL_NAME = 'm31a://event';

export type RuntimeEventListener = (event: RuntimeEvent) => void;

/**
 * Subscribes to the runtime event bridge channel.
 * Returns an unlisten function for cleanup.
 */
export async function subscribeToRuntimeEvents(
  listener: RuntimeEventListener
): Promise<UnlistenFn> {
  return listen<RuntimeEvent>(EVENT_CHANNEL_NAME, (tauriEvent) => {
    try {
      listener(tauriEvent.payload);
    } catch (err) {
      console.error('Error in runtime event handler:', err, tauriEvent.payload);
    }
  });
}

export const EVENT_GAP_CHANNEL_NAME = 'm31a://event-gap';

/**
 * Subscribe to loss notifications from the Rust broadcast bridge.
 * Callers should re-fetch authoritative runtime state after a gap.
 */
export async function subscribeToRuntimeEventGaps(
  listener: (skipped: number) => void
): Promise<UnlistenFn> {
  return listen<number>(EVENT_GAP_CHANNEL_NAME, (event) => {
    listener(event.payload);
  });
}
