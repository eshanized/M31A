//! Event bridge forwarding runtime events to the desktop presentation layer.
//!
//! Subscribes to the authoritative `M31aRuntime` broadcast channel and emits
//! strongly-typed events to Tauri webviews using the `m31a://event` channel.

use m31a_runtime::M31aRuntime;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, warn};

pub const EVENT_CHANNEL_NAME: &str = "m31a://event";

/// Spawns a background task that streams events from the runtime to the Tauri webview.
pub fn start_event_bridge(app_handle: AppHandle, runtime: M31aRuntime) {
    let mut rx = runtime.subscribe_events();

    tauri::async_runtime::spawn(async move {
        debug!("Starting desktop event bridge listener");

        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(err) = app_handle.emit(EVENT_CHANNEL_NAME, &event) {
                        error!("Failed to emit runtime event to webview: {err}");
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    warn!("Event bridge lagged by {skipped} events; requesting frontend reconciliation");
                    if let Err(err) = app_handle.emit("m31a://event-gap", skipped) {
                        error!("Failed to signal runtime event gap to webview: {err}");
                    }
                }
                Err(RecvError::Closed) => {
                    debug!("Event broadcaster closed; shutting down desktop event bridge");
                    break;
                }
            }
        }
    });
}
