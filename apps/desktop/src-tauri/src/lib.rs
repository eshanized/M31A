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

#[cfg(target_os = "linux")]
fn ensure_webkit_environment() {
    let link_target = std::path::Path::new("/var/tmp/webkit2gtk");
    if !link_target.exists() {
        if link_target.is_symlink() {
            let _ = std::fs::remove_file(link_target);
        }
        if let Ok(home) = std::env::var("HOME") {
            let user_webkit = std::path::PathBuf::from(home).join(".local/usr/lib/webkit2gtk-4.1");
            if user_webkit.exists() {
                let _ = std::os::unix::fs::symlink(&user_webkit, link_target);
            }
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    ensure_webkit_environment();

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
            commands::pick_workspace_directory,
            commands::list_workspace_files,
            commands::read_workspace_file_preview,
            commands::rename_session,
            commands::delete_session,
            commands::fork_session,
            commands::export_session_transcript,
            commands::list_registered_tools,
            commands::get_runtime_config,
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
