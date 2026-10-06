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
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::install_panic_hook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Install terminal panic restoration hook (TDS-01, F-15)
    install_panic_hook();

    // 0b. Channel-aware `--version` / `-V` (compile-time artifact identity).
    // The channel is part of the artifact, never a runtime env switch: a
    // production binary cannot become development via `M31A_ENV`.
    {
        let raw: Vec<String> = std::env::args().collect();
        if raw.len() == 2 && (raw[1] == "--version" || raw[1] == "-V") {
            println!("{}", m31a::deployment::cli_version_string());
            return Ok(());
        }
    }

    let cli = Cli::parse();

    // 1. Resolve workspace root and canonical storage layout.
    // The raw root is canonicalized by the workspace-instance authority
    // (`resolve_startup`); all startup layers below consume that canonical
    // resolution so wizard and runtime always address the same workspace.
    // Canonical stores (DB, credentials, cache, state) live in platform
    // user dirs via `StorageLayout`; `<ws>/.m31a/` holds only
    // workspace-scoped state.
    let workspace_root = cli
        .workspace
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    m31a::config::load_dotenv_from_workspace(&workspace_root);
    let deployment_channel = m31a::deployment::DeploymentChannel::current();
    let storage_layout = m31a::storage::StorageLayout::new(&workspace_root, deployment_channel);
    let _ = m31a::storage::migrate_legacy_workspace_state(&storage_layout);
    let _ = storage_layout.ensure_global_dirs();
    let _ = storage_layout.ensure_workspace_dir();

    // 1b. Build authoritative ResolvedConfiguration (CFG-01, CFX-04).
    // Strict semantics: a PRESENT-but-INVALID workspace configuration is a
    // hard startup error, never a silent collapse into defaults. Only
    // intentional absence (no config file) uses documented safe defaults.
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
            if m31a::config::ResolvedConfiguration::workspace_config_exists(&workspace_root) {
                eprintln!("Error: workspace configuration is invalid: {e}");
                eprintln!(
                    "Fix {} (or remove it to use documented defaults) and retry; startup on fallback defaults was refused.",
                    workspace_root.join(".m31a").join("config.toml").display()
                );
                std::process::exit(1);
            }
            eprintln!(
                "Warning: no workspace configuration found ({e}); continuing with documented safe defaults"
            );
            Arc::new(m31a::config::ResolvedConfiguration::build_fallback(
                &workspace_root,
            ))
        }
    };

    // 2. Initialize SQLite persistence and execute pending migrations.
    // Canonical database: platform user data (channel-isolated). Legacy
    // project-local DBs are migrated by `StorageLayout` before opening.
    let db_path = storage_layout.global_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
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
                storage_layout.workspace_init_sentinel().display()
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
    // Fail-closed runtime assembly: commands that REQUIRE an executable
    // runtime abort deterministically on assembly failure. The error is
    // returned (never demoted to a warning + `None` continuation), so a
    // broken runtime can never degrade into fake-success dispatch.
    let ensure_runtime = async |pool: &sqlx::SqlitePool,
                                workspace_root: &PathBuf,
                                event_bus: &Arc<BroadcastEventBus>,
                                config: &Arc<m31a::config::ResolvedConfiguration>|
           -> Result<Arc<AppRuntime>, String> {
        match AppRuntime::from_pool_workspace_and_config(
            pool.clone(),
            workspace_root.clone(),
            event_bus.clone(),
            config.clone(),
        )
        .await
        {
            Ok(rt) => Ok(Arc::new(rt)),
            Err(e) => Err(format!("failed to assemble complete AppRuntime: {e}")),
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

        runtime_arc = match ensure_runtime(&pool, &workspace_root, &event_bus, &config).await {
            Ok(rt) => Some(rt),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };
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
                | Some(Commands::Version(_))
                | Some(Commands::Deployment(_))
                | Some(Commands::Update(_))
                | Some(Commands::Rollback(_))
        )
    {
        // These commands REQUIRE the executable runtime: assembly failure
        // aborts here rather than dispatching against a stale standalone
        // dispatcher that could manufacture success.
        match ensure_runtime(&pool, &workspace_root, &event_bus, &config).await {
            Ok(rt) => {
                runtime_arc = Some(rt.clone());
                dispatcher = dispatcher.with_runtime(rt);
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
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
                // Fail closed: an unparseable session id or a missing runtime
                // is an explicit error, never a silent fallthrough into
                // unrelated dispatch.
                let uuid = id.parse::<uuid::Uuid>().map_err(|_| {
                    Box::<dyn std::error::Error>::from(format!(
                        "Invalid session id '{id}': expected a UUID"
                    ))
                })?;
                let rt = runtime_arc.clone().ok_or_else(|| {
                    Box::<dyn std::error::Error>::from(
                        "AppRuntime is required to resume a session but is unavailable",
                    )
                })?;
                let sid = m31a::ids::SessionId::from(uuid);
                let mut runner = m31a::interaction::InteractiveSessionRunner::new(rt);
                runner.init_session(Some(sid)).await?;
                runner.run_loop().await?;
                return Ok(());
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
    _dispatcher: CliDispatcher,
    workspace_root: PathBuf,
    event_bus: Arc<BroadcastEventBus>,
    pool: sqlx::SqlitePool,
    config: Arc<m31a::config::ResolvedConfiguration>,
    startup: m31a::init::StartupDecision,
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::event::{self, Event, KeyCode, MouseEventKind};
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
        // effect immediately for this session. The pre-wizard configuration
        // was valid, so a rebuild failure keeps it with an explicit error
        // (never a silent fallback).
        match m31a::config::ResolvedConfiguration::builder(&workspace_root).build() {
            Ok(c) => Arc::new(c),
            Err(e) => {
                eprintln!(
                    "Error: failed to reload configuration after onboarding ({e}); continuing with the pre-wizard configuration."
                );
                config
            }
        }
    } else {
        config
    };

    // The runtime is (re-)constructed against the resolved instance AFTER
    // onboarding, so the cockpit always observes post-onboarding state with
    // no double initialization. Assembly failure is fatal: the cockpit
    // REQUIRES the executable runtime, and a degraded cockpit without one
    // would misrepresent broken state as operational.
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
            eprintln!("Error: failed to assemble complete AppRuntime for cockpit: {e}");
            std::process::exit(1);
        }
    };

    let mut app = m31a::tui::TuiApp::new()
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

        let poll_interval = if app.model.has_active_animation() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(50)
        };

        if event::poll(poll_interval)? {
            match event::read()? {
                Event::Key(key) => {
                    let is_ctrl_c = key.code == KeyCode::Char('c')
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL);

                    let is_ctrl_d = key.code == KeyCode::Char('d')
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL);

                    // When actively working or when input is in composer, Ctrl+C cancels the current action / clears composer
                    if is_ctrl_c
                        && (app.model.has_active_animation()
                            || (app.is_composer_focused && !app.composer.text().is_empty()))
                    {
                        app.handle_key(key);
                        continue;
                    }

                    let is_exit = (key.code == KeyCode::Char('q')
                        && !app.approval_modal.is_open
                        && !app.palette.is_open
                        && !app.is_composer_focused)
                        || (is_ctrl_c && !app.approval_modal.is_open && !app.palette.is_open)
                        || (is_ctrl_d && !app.approval_modal.is_open && !app.palette.is_open);

                    if is_exit {
                        app.is_running = false;
                        break;
                    }

                    // Dispatch actions directly through TUI bridge via app.handle_key
                    app.handle_key(key);
                }
                Event::Paste(text) => {
                    app.handle_paste(&text);
                }
                // Mouse capture is enabled by the terminal guard; wheel
                // events scroll the authoritative conversation viewport.
                // Keyboard scrolling remains the mandatory path.
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => app.handle_mouse_scroll(true),
                    MouseEventKind::ScrollDown => app.handle_mouse_scroll(false),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // Restore terminal safely (F-15)
    let _ = guard.restore();
    terminal.show_cursor()?;

    Ok(())
}
