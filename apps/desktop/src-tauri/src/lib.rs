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

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber for desktop host logs if not already active.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,m31a_desktop=debug")),
        )
        .try_init();

    info!("Starting M31A Desktop Host...");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // A GUI app may start with '/' or another unrelated process cwd.
            // Use the user's home directory as an explicit initial workspace.
            let initial_workspace = app.path().home_dir()?;
            let runtime = tauri::async_runtime::block_on(async {
                lifecycle::init_runtime(Some(initial_workspace)).await
            })?;

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
            commands::get_pending_interactions,
            commands::respond_approval,
            commands::respond_question,
        ])
        .build(tauri::generate_context!())?;

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            info!("Desktop exit requested, tearing down runtime...");
            let state = app_handle.state::<state::DesktopState>();
            tauri::async_runtime::block_on(async {
                lifecycle::teardown_desktop_application(&state).await;
            });
        }
    });

    Ok(())
}
