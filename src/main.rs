//! M31 Autonomous (M31A) - Main Entry Point.
//!
//! "The model proposes. The runtime decides." (PRD §01, CLI-01, CLI-04)

use clap::Parser;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

use m31a::cli::args::{Cli, Commands, OutputFormat};
use m31a::cli::dispatch::CliDispatcher;
use m31a::events::bus::BroadcastEventBus;
use m31a::persistence::paths::project_local_dir;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::install_panic_hook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Install terminal panic restoration hook (TDS-01, F-15)
    install_panic_hook();

    let cli = Cli::parse();

    // 1. Resolve workspace root and project data directory.
    // The raw root is canonicalized by the workspace-instance authority
    // (`resolve_startup`); all startup layers below consume that canonical
    // resolution so wizard and runtime always address the same workspace.
    let workspace_root = cli
        .workspace
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    m31a::config::load_dotenv_from_workspace(&workspace_root);
    let data_dir = project_local_dir(&workspace_root);
    std::fs::create_dir_all(&data_dir)?;

    // 1b. Build authoritative ResolvedConfiguration (CFG-01, CFX-04)
    let config = match m31a::config::ResolvedConfiguration::builder(&workspace_root)
        .with_explicit_config(cli.config.clone())
        .with_profile(cli.profile.clone())
        .with_model(cli.model.clone())
        .with_autonomy(cli.autonomy.clone())
        .build()
    {
        Ok(c) => Arc::new(c),
        Err(e) => {
            if cli.config.is_some() {
                eprintln!("Error: failed to load explicit configuration: {e}");
                std::process::exit(1);
            }
            eprintln!(
                "Warning: failed to build resolved configuration ({e}), falling back to safe defaults"
            );
            Arc::new(m31a::config::ResolvedConfiguration::build_fallback(
                &workspace_root,
            ))
        }
    };

    // 2. Initialize SQLite persistence and execute pending migrations
    let db_path = data_dir.join("m31a.db");
    let pool = initialize_database(&db_path).await?;

    // 3. Initialize core event bus
    let event_bus = Arc::new(BroadcastEventBus::new(2048));

    // 4. Startup crash recovery scanning is owned canonically by AppRuntime.
    // AppRuntime::from_pool_workspace_and_config runs StartupCrashRecoveryScanner
    // exactly once; no second scan is performed here.

    // 5. CANONICAL STARTUP AUTHORITY (single): resolve the persistent
    // workspace instance once. The onboarding decision depends ONLY on durable
    // workspace state — never on "a new process just started". Every path
    // below consumes this decision; no layer re-checks onboarding itself.
    // Fail-closed: corrupt / version-skewed / inconsistent state aborts with
    // an explicit error instead of silently re-running the wizard.
    let startup = match m31a::init::resolve_startup(&workspace_root, &pool).await {
        Ok(decision) => decision,
        Err(e) => {
            eprintln!("Error: workspace initialization state is unusable: {e}");
            eprintln!(
                "Resolve the underlying issue (inspect {} and the canonical SQLite system_state record) and retry; onboarding was NOT started.",
                project_local_dir(&workspace_root)
                    .join("init.json")
                    .display()
            );
            std::process::exit(1);
        }
    };
    let startup_needs_onboarding = startup.needs_onboarding();

    // 6. Build the CLI dispatcher (runtime attached later, after onboarding).
    let mut dispatcher =
        CliDispatcher::production(pool.clone(), workspace_root.clone(), event_bus.clone())
            .with_config(config.clone());

    // 7. Construct the complete AppRuntime — AFTER the onboarding decision,
    // never before. When first-run onboarding is pending, the runtime is built
    // after the wizard completes (inside run_tui_or_fallback) so startup
    // performs no double initialization.
    //
    // Exception: explicit subcommands below construct the runtime on demand
    // via `ensure_runtime`.
    let mut runtime_arc: Option<Arc<AppRuntime>> = None;
    let ensure_runtime = async |pool: &sqlx::SqlitePool,
                                workspace_root: &PathBuf,
                                event_bus: &Arc<BroadcastEventBus>,
                                config: &Arc<m31a::config::ResolvedConfiguration>|
           -> Option<Arc<AppRuntime>> {
        match AppRuntime::from_pool_workspace_and_config(
            pool.clone(),
            workspace_root.clone(),
            event_bus.clone(),
            config.clone(),
        )
        .await
        {
            Ok(rt) => Some(Arc::new(rt)),
            Err(e) => {
                eprintln!("Warning: failed to assemble complete AppRuntime: {e}");
                None
            }
        }
    };

    // 8. Handle interactive session, TUI cockpit, or command dispatch
    if cli.command.is_none() {
        if startup_needs_onboarding {
            let runner = m31a::cli::doctor::DoctorRunner::with_default_probes();
            let report = runner.run(None).await;
            println!("{}", report.format_text());

            use std::io::IsTerminal;
            if std::io::stdin().is_terminal()
                && std::io::stdout().is_terminal()
                && std::env::var("M31A_HEADLESS").is_err()
            {
                println!("\nWorkspace is not onboarded. Launching setup wizard...\n");
                return run_tui_or_fallback(
                    dispatcher.clone(),
                    workspace_root.clone(),
                    event_bus.clone(),
                    pool.clone(),
                    config.clone(),
                    startup,
                )
                .await;
            } else {
                println!(
                    "\nWorkspace is not onboarded. Run 'm31a init' or 'm31a tui' to complete first-run setup."
                );
                return Ok(());
            }
        }

        runtime_arc = ensure_runtime(&pool, &workspace_root, &event_bus, &config).await;
        if let Some(rt) = runtime_arc.clone() {
            let mut runner = m31a::interaction::InteractiveSessionRunner::new(rt);
            runner.run_loop().await?;
            return Ok(());
        } else {
            eprintln!("Error: failed to initialize AppRuntime for interactive session.");
            std::process::exit(1);
        }
    }

    // Explicit session commands construct the runtime on demand (startup
    // already resolved the workspace instance above; no second onboarding
    // check is performed here — subcommands never trigger the wizard).
    if runtime_arc.is_none()
        && matches!(
            cli.command,
            Some(Commands::Session(_))
                | Some(Commands::Mission(_))
                | Some(Commands::Task(_))
                | Some(Commands::Agent(_))
                | Some(Commands::Capability(_))
                | Some(Commands::Policy(_))
                | Some(Commands::Checkpoint(_))
                | Some(Commands::Artifact(_))
                | Some(Commands::Doctor(_))
                | Some(Commands::Config(_))
                | Some(Commands::Telemetry(_))
                | Some(Commands::Eval(_))
                | Some(Commands::Init(_))
                | Some(Commands::Version)
        )
    {
        runtime_arc = ensure_runtime(&pool, &workspace_root, &event_bus, &config).await;
        if let Some(rt) = runtime_arc.clone() {
            dispatcher = dispatcher.with_runtime(rt);
        }
    }

    if let Some(Commands::Session(ref s)) = cli.command {
        match &s.command {
            None | Some(m31a::cli::args::SessionCommands::New) => {
                if let Some(rt) = runtime_arc.clone() {
                    let mut runner = m31a::interaction::InteractiveSessionRunner::new(rt);
                    runner.run_loop().await?;
                    return Ok(());
                } else {
                    eprintln!("Error: failed to initialize AppRuntime for interactive session.");
                    std::process::exit(1);
                }
            }
            Some(m31a::cli::args::SessionCommands::Resume { id })
                if std::io::stdin().is_terminal() =>
            {
                if let (Some(rt), Ok(uuid)) = (runtime_arc.clone(), id.parse::<uuid::Uuid>()) {
                    let sid = m31a::ids::SessionId::from(uuid);
                    let mut runner = m31a::interaction::InteractiveSessionRunner::new(rt);
                    runner.init_session(Some(sid)).await?;
                    runner.run_loop().await?;
                    return Ok(());
                }
            }
            _ => {}
        }
    }

    if matches!(cli.command, Some(Commands::Tui)) {
        run_tui_or_fallback(
            dispatcher,
            workspace_root,
            event_bus,
            pool.clone(),
            config,
            startup,
        )
        .await?;
        return Ok(());
    }

    let runtime_cmd = match dispatcher.parse_command(&cli) {
        Some(cmd) => cmd,
        None => return Ok(()),
    };

    match dispatcher.dispatch(runtime_cmd).await {
        Ok(output) => {
            match cli.output {
                OutputFormat::Text => {
                    if !cli.quiet {
                        println!("{}", output.text);
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&output.data)?);
                }
                OutputFormat::StreamJson => {
                    println!("{}", serde_json::to_string(&output.data)?);
                }
            }
            if output.exit_code != 0 {
                std::process::exit(output.exit_code);
            }
        }
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Launches the interactive TUI application, or displays headless instructions if non-interactive.
///
/// Receives the already-resolved [`m31a::init::StartupDecision`] from the
/// canonical startup authority in `main`. The TUI NEVER decides onboarding
/// itself: no `InitManager` check lives on this path.
async fn run_tui_or_fallback(
    mut dispatcher: CliDispatcher,
    workspace_root: PathBuf,
    event_bus: Arc<BroadcastEventBus>,
    pool: sqlx::SqlitePool,
    config: Arc<m31a::config::ResolvedConfiguration>,
    startup: m31a::init::StartupDecision,
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::event::{self, Event, KeyCode};
    use ratatui::Terminal;
    use ratatui::backend::CrosstermBackend;
    use std::io::{IsTerminal, stdout};
    use std::time::Duration;

    // Check if terminal raw mode is supported and environment is interactive
    if std::env::var("M31A_HEADLESS").is_ok()
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        println!("M31 Autonomous (M31A) - Interactive Cockpit");
        println!("Terminal does not support raw mode (non-interactive or piped environment).");
        println!("Run 'm31a --help' for available CLI commands.");
        return Ok(());
    }

    // Acquire RAII terminal guard (F-15, TDS-01)
    let mut guard = match m31a::tui::TerminalGuard::acquire() {
        Ok(g) => g,
        Err(_) => {
            println!("M31 Autonomous (M31A) - Interactive Cockpit");
            println!("Terminal does not support raw mode (non-interactive or piped environment).");
            println!("Run 'm31a --help' for available CLI commands.");
            return Ok(());
        }
    };

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // First-run onboarding (F-05, FRX-02, FRX-03), driven ONLY by the
    // resolved startup decision. The wizard below runs at most once per
    // workspace: successful completion is durably persisted before the
    // cockpit starts.
    let onboarding_ran = startup.needs_onboarding();
    if let m31a::init::StartupDecision::NeedsOnboarding { instance, .. } = &startup {
        let canonical = instance.workspace_root().to_path_buf();
        let mut wizard = m31a::tui::screens::wizard::SetupWizardScreen::new(canonical.clone());
        let mut wizard_done = false;
        while !wizard_done {
            terminal.draw(|f| {
                wizard.render(f, f.area());
            })?;

            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                match wizard.handle_key(key) {
                    m31a::tui::screens::wizard::WizardOutcome::Completed => {
                        // Fail-closed completion: configuration persistence
                        // AND durable onboarding persistence must BOTH
                        // succeed. Any failure leaves the workspace
                        // uninitialized and keeps the wizard open with an
                        // explicit message (recoverable, cancellable).
                        match wizard.persist_configuration() {
                            Err(e) => {
                                wizard.status_message = Some(format!(
                                    "Failed to persist configuration: {e}. Fix the issue or press Esc to cancel (workspace remains uninitialized)."
                                ));
                            }
                            Ok(()) => {
                                match m31a::init::persist_successful_onboarding(&canonical, &pool)
                                    .await
                                {
                                    Err(e) => {
                                        wizard.status_message = Some(format!(
                                            "Failed to record onboarding completion: {e}. Workspace remains uninitialized; fix the issue or press Esc to cancel."
                                        ));
                                    }
                                    Ok(_) => {
                                        wizard_done = true;
                                    }
                                }
                            }
                        }
                    }
                    m31a::tui::screens::wizard::WizardOutcome::Cancelled => {
                        // Cancelled onboarding never marks the workspace
                        // onboarded; durable state is untouched.
                        let _ = guard.restore();
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }

    let config = if onboarding_ran {
        // Rebuild from the workspace so wizard-written configuration takes
        // effect immediately for this session.
        match m31a::config::ResolvedConfiguration::builder(&workspace_root).build() {
            Ok(c) => Arc::new(c),
            Err(_) => config,
        }
    } else {
        config
    };

    // The runtime is (re-)constructed against the resolved instance AFTER
    // onboarding, so the cockpit always observes post-onboarding state with
    // no double initialization.
    let runtime = match AppRuntime::from_pool_workspace_and_config(
        pool.clone(),
        workspace_root.clone(),
        event_bus.clone(),
        config.clone(),
    )
    .await
    {
        Ok(rt) => Some(Arc::new(rt)),
        Err(e) => {
            eprintln!("Warning: failed to assemble complete AppRuntime: {e}");
            None
        }
    };
    if let Some(ref rt) = runtime {
        dispatcher = dispatcher.with_runtime(rt.clone());
    }

    // Connect bounded event bridge to TUI render loop (F-12)
    let (tui_tx, tui_rx) =
        m31a::tui::channel::create_tui_channel(m31a::tui::channel::DEFAULT_TUI_QUEUE_CAPACITY);
    let bridge_bus = event_bus.clone();
    tokio::spawn(async move {
        use futures::StreamExt;
        use m31a::events::bus::EventBus;
        let mut rx = bridge_bus
            .subscribe(m31a::events::bus::EventFilter::all())
            .await;
        while let Some(Ok(envelope)) = rx.next().await {
            let _ = tui_tx.try_send(envelope);
        }
    });

    let mut app = m31a::tui::TuiApp::new()
        .with_receiver(tui_rx)
        .with_workspace_root(workspace_root.clone())
        .with_config(&config)
        .with_composer_focused(true);

    let _bridge_handle = if let Some(ref rt) = runtime {
        app.hydrate_from_runtime(rt).await;
        let (mut bridge, handle) = m31a::tui::TuiRuntimeBridge::spawn(rt.clone(), None).await?;
        app = app.with_bridge_tx(bridge.sender());
        if let Some(irx) = bridge.take_event_receiver() {
            app = app.with_interaction_rx(irx);
        }
        Some(handle)
    } else {
        None
    };

    while app.is_running {
        app.tick(&mut terminal)?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    let is_exit = (key.code == KeyCode::Char('q')
                        && !app.approval_modal.is_open
                        && !app.palette.is_open
                        && !app.is_composer_focused)
                        || (key.code == KeyCode::Char('c')
                            && key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            && !app.approval_modal.is_open
                            && !app.palette.is_open)
                        || (key.code == KeyCode::Char('d')
                            && key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            && !app.approval_modal.is_open
                            && !app.palette.is_open);

                    if is_exit {
                        app.is_running = false;
                        break;
                    }

                    // Dispatch mutations directly from TUI keyboard events (F-03)
                    if let Some(cmd) = app.handle_key(key) {
                        let _ = dispatcher.dispatch(cmd).await;
                    }
                }
                Event::Paste(text) => {
                    app.handle_paste(&text);
                }
                _ => {}
            }
        }
    }

    // Restore terminal safely (F-15)
    let _ = guard.restore();
    terminal.show_cursor()?;

    Ok(())
}
