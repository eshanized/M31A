//! Reusable TUI fixtures for Multi-Resolution Framebuffer Regression and PTY Testing.

use chrono::Utc;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use std::collections::VecDeque;

use m31a::tui::TuiApp;
use m31a::tui::model::{
    TuiAgentSnapshot, TuiApprovalRequest, TuiJobSnapshot, TuiLogLine, TuiModelUsage,
    TuiSystemStats, TuiTaskSnapshot, TuiToolSnapshot, TuiViewModel,
};

/// Create a fully populated mock `TuiApp` state.
pub fn create_mock_tui_app() -> TuiApp {
    let mut app = TuiApp::new();
    let mut model = TuiViewModel::new();

    model.mission_id = Some("01918a24-10f3".to_string());
    model.mission_name = "Auth & Encryption Refactor".to_string();
    model.mission_status = "executing".to_string();
    model.objective =
        "Implement OAuth2 authentication flow with AES-256 SQLite secret encryption".to_string();

    // Add tasks
    model.tasks = vec![
        TuiTaskSnapshot {
            id: "task-01".to_string(),
            title: "Database schema migration for OAuth tokens".to_string(),
            status: "completed".to_string(),
            agent_role: Some("DatabaseArchitect".to_string()),
            progress_pct: 100,
            dependencies: vec![],
        },
        TuiTaskSnapshot {
            id: "task-02".to_string(),
            title: "OAuth2 client provider implementation".to_string(),
            status: "running".to_string(),
            agent_role: Some("SystemsEngineer".to_string()),
            progress_pct: 65,
            dependencies: vec!["task-01".to_string()],
        },
        TuiTaskSnapshot {
            id: "task-03".to_string(),
            title: "Verification test suite and security gate".to_string(),
            status: "pending".to_string(),
            agent_role: Some("SecurityAuditor".to_string()),
            progress_pct: 0,
            dependencies: vec!["task-02".to_string()],
        },
    ];

    // Add agents
    model.agents = vec![
        TuiAgentSnapshot {
            id: "agent-lead".to_string(),
            role: "LeadOrchestrator".to_string(),
            state: "idle".to_string(),
            current_task: None,
            total_tokens: 14500,
        },
        TuiAgentSnapshot {
            id: "agent-coder".to_string(),
            role: "SystemsEngineer".to_string(),
            state: "executing".to_string(),
            current_task: Some("task-02".to_string()),
            total_tokens: 48200,
        },
    ];

    // Add tools
    model.tools = vec![
        TuiToolSnapshot {
            name: "fs_write".to_string(),
            executions_count: 14,
            errors_count: 0,
        },
        TuiToolSnapshot {
            name: "git_commit".to_string(),
            executions_count: 3,
            errors_count: 0,
        },
        TuiToolSnapshot {
            name: "exec_cargo_test".to_string(),
            executions_count: 8,
            errors_count: 1,
        },
    ];

    // Add jobs
    model.jobs = vec![TuiJobSnapshot {
        id: "job-01".to_string(),
        name: "cargo test --test oauth2".to_string(),
        status: "running".to_string(),
        duration_ms: 1420,
    }];

    // Add logs
    let mut logs = VecDeque::new();
    let now = Utc::now();
    logs.push_back(TuiLogLine {
        timestamp: now,
        level: "INFO".to_string(),
        source: "kernel".to_string(),
        message: "Runtime initialized. Scheduler active.".to_string(),
    });
    logs.push_back(TuiLogLine {
        timestamp: now,
        level: "INFO".to_string(),
        source: "scheduler".to_string(),
        message: "Dispatched task-02 to SystemsEngineer.".to_string(),
    });
    logs.push_back(TuiLogLine {
        timestamp: now,
        level: "WARN".to_string(),
        source: "policy".to_string(),
        message: "Network egress restricted to authorized endpoints.".to_string(),
    });
    model.logs = logs;

    // Add approvals
    model.approvals = vec![TuiApprovalRequest {
        id: "appr-001".to_string(),
        tool_name: "fs_write".to_string(),
        agent_role: "SystemsEngineer".to_string(),
        justification: "Modify security critical auth file".to_string(),
        parameters_summary: "path: /src/auth/oauth.rs".to_string(),
        risk_tier: "High".to_string(),
        timestamp: now,
    }];

    // Model usage & stats
    model.model_usage = TuiModelUsage {
        prompt_tokens: 38200,
        completion_tokens: 12400,
        total_cost_cents: Some(250),
        api_calls: 18,
        processed_invocations: std::collections::HashSet::new(),
    };

    model.system_stats = TuiSystemStats {
        uptime_secs: 420,
        events_processed: 84,
        memory_rss_mb: 72,
    };

    app.model = model;
    app
}

/// Render the app to a Ratatui TestBackend of specified width and height.
/// Returns both the raw cell Buffer and a string representation.
pub fn render_to_buffer(app: &mut TuiApp, width: u16, height: u16) -> (Buffer, String) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    app.force_redraw = true;
    app.render_frame(&mut terminal).expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let content = buffer_to_string(&buffer);
    (buffer, content)
}

/// Convert a Ratatui Buffer to a multiline string.
pub fn buffer_to_string(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Assert that none of the cleartext secrets appear in the rendered buffer (T-13-14).
#[allow(dead_code)]
pub fn assert_buffer_secret_scrubbing(buffer_str: &str, secrets: &[&str]) {
    for secret in secrets {
        assert!(
            !buffer_str.contains(secret),
            "Security violation: cleartext secret '{}' detected in rendered buffer!",
            secret
        );
    }
}
