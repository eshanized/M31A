//! Desktop application library for M31A.
//!
//! Provides Tauri 2 runtime initialization, IPC command registration,
//! state management, and lifecycle hooks.

pub mod commands;
pub mod events;
pub mod lifecycle;
pub mod security;
pub mod state;

use tauri::Manager;
use tracing::info;

pub fn run() {
    // Initialize tracing subscriber for desktop host logs if not already active
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,m31a_desktop=debug")),
        )
        .try_init();

    info!("Starting M31A Desktop Host...");

    let app = tauri::Builder::default()
        .setup(|app| {
            // Synchronously / blocking load runtime for startup
            let runtime = tauri::async_runtime::block_on(async {
                lifecycle::init_runtime(None)
                    .await
                    .expect("Failed to initialize authoritative M31A runtime")
            });

            lifecycle::setup_desktop_application(app, runtime)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_runtime_status,
            commands::get_environment_info,
            commands::shutdown_runtime,
            commands::get_active_workspace,
            commands::set_workspace,
            commands::authorize_workspace,
            commands::list_available_models,
            commands::get_active_model,
            commands::activate_model,
            commands::list_sessions,
            commands::create_session,
            commands::load_session,
            commands::submit_turn,
            commands::cancel_turn,
            commands::steer_turn,
            commands::respond_approval,
            commands::respond_question,
        ])
        .build(tauri::generate_context!())
        .expect("Error building M31A desktop application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            info!("Desktop exit requested, tearing down runtime...");
            let state = app_handle.state::<state::DesktopState>();
            tauri::async_runtime::block_on(async {
                lifecycle::teardown_desktop_application(&state).await;
            });
        }
    });
}
