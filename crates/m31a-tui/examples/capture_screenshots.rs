use m31a_core::Config;
use m31a_tui::screens::Screen;
use m31a_tui::transcript::{NoticeLevel, ToolCallEntry, TranscriptItem};
use m31a_tui::{App, ThemeVariant, init_theme, modals::ModalState, renderer::render};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use std::process::Command;

fn color_to_fg_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Reset => (0xda, 0xda, 0xda), // Everblush text_primary
        Color::Black => (0x2d, 0x34, 0x37),
        Color::Red | Color::LightRed => (0xe5, 0x74, 0x74),
        Color::Green | Color::LightGreen => (0x8c, 0xcf, 0x7e),
        Color::Yellow | Color::LightYellow => (0xe5, 0xc7, 0x6b),
        Color::Blue | Color::LightBlue => (0x67, 0xb0, 0xe8),
        Color::Magenta | Color::LightMagenta => (0xc4, 0x7f, 0xd5),
        Color::Cyan | Color::LightCyan => (0x6c, 0xbf, 0xbf),
        Color::Gray | Color::DarkGray => (0x67, 0x6e, 0x71),
        Color::White => (0xda, 0xda, 0xda),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => match i {
            0 => (0x2d, 0x34, 0x37),
            1 => (0xe5, 0x74, 0x74),
            2 => (0x8c, 0xcf, 0x7e),
            3 => (0xe5, 0xc7, 0x6b),
            4 => (0x67, 0xb0, 0xe8),
            5 => (0xc4, 0x7f, 0xd5),
            6 => (0x6c, 0xbf, 0xbf),
            7 => (0xb3, 0xb9, 0xb8),
            8 => (0x67, 0x6e, 0x71),
            _ => (0xda, 0xda, 0xda),
        },
    }
}

fn color_to_bg_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Reset => (0x14, 0x1b, 0x1e), // Everblush base background
        Color::Black => (0x14, 0x1b, 0x1e),
        Color::Red | Color::LightRed => (0x4a, 0x1f, 0x1f),
        Color::Green | Color::LightGreen => (0x1f, 0x3d, 0x22),
        Color::Yellow | Color::LightYellow => (0x4a, 0x3e, 0x1f),
        Color::Blue | Color::LightBlue => (0x1a, 0x2e, 0x40),
        Color::Magenta | Color::LightMagenta => (0x3a, 0x1f, 0x40),
        Color::Cyan | Color::LightCyan => (0x1c, 0x33, 0x36),
        Color::Gray | Color::DarkGray => (0x23, 0x2a, 0x2d),
        Color::White => (0xda, 0xda, 0xda),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => match i {
            0 => (0x14, 0x1b, 0x1e),
            1 => (0x4a, 0x1f, 0x1f),
            2 => (0x1f, 0x3d, 0x22),
            3 => (0x4a, 0x3e, 0x1f),
            4 => (0x1a, 0x2e, 0x40),
            5 => (0x3a, 0x1f, 0x40),
            6 => (0x1c, 0x33, 0x36),
            7 => (0x23, 0x2a, 0x2d),
            8 => (0x2d, 0x34, 0x37),
            _ => (0x23, 0x2a, 0x2d),
        },
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn buffer_to_svg(buffer: &Buffer, title: &str) -> String {
    let width = buffer.area.width as usize;
    let height = buffer.area.height as usize;

    let char_w = 9.2f64;
    let char_h = 19.0f64;
    let pad_x = 16.0f64;
    let pad_top = 44.0f64;
    let pad_bottom = 16.0f64;

    let svg_w = (width as f64 * char_w + pad_x * 2.0).ceil();
    let svg_h = (height as f64 * char_h + pad_top + pad_bottom).ceil();

    let mut svg = String::with_capacity(256 * 1024);
    svg.push_str(&format!(
        r##"<svg width="{:.0}" height="{:.0}" viewBox="0 0 {:.0} {:.0}" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <style>
      .window-bg {{ fill: #141b1e; stroke: #2e3538; stroke-width: 1.5; }}
      .title-bar {{ fill: #192124; }}
      .title-text {{ fill: #b3b9b8; font-family: 'JetBrainsMono Nerd Font', 'JetBrains Mono', 'Noto Sans Mono', monospace; font-size: 12px; font-weight: 500; }}
      .term-text {{ font-family: 'JetBrainsMono Nerd Font', 'JetBrains Mono', 'Noto Sans Mono', monospace; font-size: 13.5px; }}
    </style>
  </defs>

  <!-- Window Container with Rounded Border -->
  <rect x="1" y="1" width="{:.0}" height="{:.0}" rx="10" ry="10" class="window-bg" />

  <!-- Window Titlebar -->
  <path d="M 1 10 C 1 5 5 1 10 1 L {:.0} 1 C {:.0} 1 {:.0} 5 {:.0} 10 L {:.0} 38 L 1 38 Z" class="title-bar" />
  <line x1="1" y1="38" x2="{:.0}" y2="38" stroke="#232a2d" stroke-width="1" />

  <!-- macOS Window Controls -->
  <circle cx="24" cy="19" r="6" fill="#e57474" />
  <circle cx="44" cy="19" r="6" fill="#e5c76b" />
  <circle cx="64" cy="19" r="6" fill="#8ccf7e" />

  <!-- Title Text -->
  <text x="{:.1}" y="23" text-anchor="middle" class="title-text">{}</text>

  <!-- Terminal Area Background -->
  <g transform="translate({:.1}, {:.1})">
"##,
        svg_w, svg_h, svg_w, svg_h,
        svg_w - 2.0, svg_h - 2.0,
        svg_w - 10.0, svg_w - 1.0, svg_w - 1.0, svg_w - 1.0, svg_w - 1.0,
        svg_w - 1.0,
        svg_w / 2.0,
        escape_xml(title),
        pad_x, pad_top
    ));

    // 1. Draw cell backgrounds
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let cell = buffer.cell((x as u16, y as u16)).unwrap();
            let (br, bg, bb) = color_to_bg_rgb(cell.bg);
            if (br, bg, bb) != (0x14, 0x1b, 0x1e) {
                let start_x = x;
                while x < width {
                    let next = buffer.cell((x as u16, y as u16)).unwrap();
                    if color_to_bg_rgb(next.bg) != (br, bg, bb) {
                        break;
                    }
                    x += 1;
                }
                let run_len = x - start_x;
                let rx = start_x as f64 * char_w;
                let ry = y as f64 * char_h;
                let rw = run_len as f64 * char_w;
                svg.push_str(&format!(
                    r##"    <rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="rgb({},{},{})" />
"##,
                    rx, ry, rw, char_h, br, bg, bb
                ));
            } else {
                x += 1;
            }
        }
    }

    // 2. Draw text spans row by row
    svg.push_str("    <!-- Terminal Text Spans -->\n");
    for y in 0..height {
        let y_pos = (y as f64 * char_h) + 14.0;
        let mut row_spans = String::new();
        let mut x = 0;

        while x < width {
            let cell = buffer.cell((x as u16, y as u16)).unwrap();
            let sym = cell.symbol();
            if sym.is_empty() || sym == " " {
                x += 1;
                continue;
            }

            let (fr, fg, fb) = color_to_fg_rgb(cell.fg);
            let bold = cell.modifier.contains(Modifier::BOLD);
            let italic = cell.modifier.contains(Modifier::ITALIC);
            let start_x = x;
            let mut text_buf = String::new();

            while x < width {
                let next = buffer.cell((x as u16, y as u16)).unwrap();
                let next_sym = next.symbol();
                let next_rgb = color_to_fg_rgb(next.fg);
                let next_bold = next.modifier.contains(Modifier::BOLD);
                let next_italic = next.modifier.contains(Modifier::ITALIC);

                if next_rgb != (fr, fg, fb) || next_bold != bold || next_italic != italic {
                    break;
                }
                text_buf.push_str(if next_sym.is_empty() { " " } else { next_sym });
                x += 1;
            }

            let span_x = start_x as f64 * char_w;
            let mut style_extra = String::new();
            if bold {
                style_extra.push_str(r#" font-weight="bold""#);
            }
            if italic {
                style_extra.push_str(r#" font-style="italic""#);
            }

            row_spans.push_str(&format!(
                r##"<tspan x="{:.1}" fill="rgb({},{},{})"{}>{}</tspan>"##,
                span_x,
                fr,
                fg,
                fb,
                style_extra,
                escape_xml(&text_buf)
            ));
        }

        if !row_spans.is_empty() {
            svg.push_str(&format!(
                r##"    <text y="{:.1}" class="term-text">{}</text>
"##,
                y_pos, row_spans
            ));
        }
    }

    svg.push_str(
        r##"  </g>
</svg>
"##,
    );
    svg
}

fn svg_to_png(svg_path: &str, png_path: &str) {
    let status = Command::new("ffmpeg")
        .args(["-y", "-i", svg_path, png_path])
        .status()
        .expect("ffmpeg must run");
    assert!(status.success(), "ffmpeg conversion must succeed");
}

fn main() -> anyhow::Result<()> {
    println!("=== M31A TUI Production Screenshot Generator ===");
    init_theme(ThemeVariant::Everblush);

    let config = Config {
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        model_provider: Some("nvidia".to_string()),
        api_key: Some(
            "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".to_string(),
        ),
        api_base: "https://integrate.api.nvidia.com/v1".to_string(),
        reasoning_effort: Some("medium".to_string()),
        ..Default::default()
    };

    let mut app = App::new(config)?;
    app.active_model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
    app.collaboration_mode = m31a_core::prompts::CollaborationMode::Default;

    // Viewport dimensions (160 cols x 46 rows)
    let w = 160;
    let h = 46;
    app.terminal_width = w;
    app.terminal_height = h;

    // --- SCREENSHOT 1: Interactive Chat with Streaming Reasoning & Tool Call ---
    println!("Rendering Screenshot 1: Chat with Reasoning & Tool Call...");
    app.current_screen = Screen::Chat;
    app.transcript.clear();

    app.transcript.push(TranscriptItem::notice(
        "M31A Autonomous Software Engineering Runtime active. Provider: NVIDIA NIM (Nemotron 3.5 Lightning 30B). Policy: WorkspaceWrite.",
        NoticeLevel::Info,
    ));

    app.transcript.push(TranscriptItem::user(
        "Audit crates/m31a-core/src/prompts.rs and verify our new 11-layer canonical prompt architecture invariants.",
    ));

    app.transcript.push(TranscriptItem::thinking(
        "Analyzing crates/m31a-core/src/prompts.rs...\n\
         • Inspecting CanonicalPromptAssembler and InstructionResolver hierarchy.\n\
         • Verifying Layer 1..11 section placement: Identity, Operating Invariants, Instruction Precedence, Runtime Envelope, Mode, Role, Tool Protocol, Model Adapter, Project Instructions, Skills/Memories, Workspace Status.\n\
         • Confirming zero optimistic capability defaults in PromptRuntimeSnapshot::gather_from_environment().\n\
         • Ensuring <untrusted_content_boundary> fences external user and file content against prompt injection.\n\
         • Validating active collaboration mode [Build] and structured role contracts.",
    ));

    app.transcript.push(TranscriptItem::ToolCall(ToolCallEntry {
        id: "call_audit_safety".to_string(),
        name: "audit_codebase_safety".to_string(),
        arguments: r#"{"target_path": "crates/m31a-core/src/prompts.rs", "strict_mode": true}"#.to_string(),
        output: Some("✓ Invariants Verified: 11 layers assembled, 136 tools gated, pure deterministic output.".to_string()),
        success: Some(true),
        in_progress: false,
        collapsed: false,
        duration_ms: Some(16),
    }));

    app.transcript.push(TranscriptItem::assistant(
        "### Architectural Audit: Canonical System Prompt Compiler\n\n\
         I have verified `crates/m31a-core/src/prompts.rs`. The implementation adheres strictly to the runtime source-of-truth principle:\n\n\
         1. **Runtime Truthfulness**: Optimistic defaults eliminated. Provider metadata, context limits, and tool authorizations reflect actual runtime components.\n\
         2. **Untrusted Content Quarantine**: External input, git diffs, and issue context are fenced within `<untrusted_content_boundary>` tags.\n\
         3. **11-Layer Order**: Deterministic section ordering (`identity` → `operating_invariants` → `instruction_precedence` → `runtime_envelope` → `collaboration_mode` → `role_contract` → `tool_protocol` → `model_adapter` → `project_instructions` → `skills`/`memories` → `workspace_status`).\n\
         4. **Parity**: Parity verified across CLI, TUI, App Server, and child subagents; system instructions preserved across compaction.\n\n\
         ```rust\n\
         // Deterministic assembly from immutable runtime snapshot\n\
         let snapshot = PromptRuntimeSnapshot::gather_from_environment(&cwd, &tools, &config)?;\n\
         let world_state = CanonicalPromptAssembler::assemble(&snapshot)?;\n\
         ```",
    ));

    app.sync_virtualizer();
    app.chat_widget.scroll_controller.scroll_to_top();

    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render(f, &mut app))?;
    let svg1 = buffer_to_svg(
        terminal.backend().buffer(),
        "M31A — Interactive Chat & Reasoning [Build Mode]",
    );
    std::fs::write("assets/screenshot_1.svg", &svg1)?;
    svg_to_png("assets/screenshot_1.svg", "assets/screenshot_1.png");
    println!("✓ Saved assets/screenshot_1.png");

    // --- SCREENSHOT 2: Frontier Model Catalog Screen ---
    println!("Rendering Screenshot 2: Models Screen...");
    app.current_screen = Screen::Models;
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render(f, &mut app))?;
    let svg2 = buffer_to_svg(
        terminal.backend().buffer(),
        "M31A — Frontier Model Catalog & Real-Time Switching (F3)",
    );
    std::fs::write("assets/screenshot_models.svg", &svg2)?;
    svg_to_png(
        "assets/screenshot_models.svg",
        "assets/screenshot_models.png",
    );
    println!("✓ Saved assets/screenshot_models.png");

    // --- SCREENSHOT 3: Agents & Swarm Orchestration Screen ---
    println!("Rendering Screenshot 3: Agents Screen...");
    app.current_screen = Screen::Agents;
    app.bottom_pane.subagent_tabs.clear();
    app.bottom_pane
        .subagent_tabs
        .push(m31a_tui::bottom_pane::SubagentTab {
            thread_id: "m31a-architect-01".to_string(),
            role: "architect".to_string(),
            status: m31a_tui::bottom_pane::SubagentStatus::Completed,
        });
    app.bottom_pane
        .subagent_tabs
        .push(m31a_tui::bottom_pane::SubagentTab {
            thread_id: "m31a-coder-02".to_string(),
            role: "coder".to_string(),
            status: m31a_tui::bottom_pane::SubagentStatus::Thinking,
        });
    app.bottom_pane
        .subagent_tabs
        .push(m31a_tui::bottom_pane::SubagentTab {
            thread_id: "m31a-reviewer-03".to_string(),
            role: "reviewer".to_string(),
            status: m31a_tui::bottom_pane::SubagentStatus::RunningTool,
        });

    let now_ms = chrono::Utc::now().timestamp_millis();
    let wf = m31a_core::workflow::WorkflowRun {
        id: "wf-prompt-overhaul".to_string(),
        thread_id: Some("current".to_string()),
        workspace_root: std::path::PathBuf::from("/home/snigdha/Desktop/M31A"),
        objective: "System Prompt Architecture Overhaul & Production Parity".to_string(),
        workflow_type: "parallel_wave".to_string(),
        state: m31a_core::workflow::WorkflowState::Running,
        plan_revision: 2,
        configuration_json: "{}".to_string(),
        created_at_epoch_ms: now_ms - 120_000,
        updated_at_epoch_ms: now_ms,
        started_at_epoch_ms: Some(now_ms - 120_000),
        completed_at_epoch_ms: None,
        cancellation_reason: None,
        overall_result_json: None,
        resource_usage_json: None,
    };
    let _ = app.storage.workflow_store().create_workflow(&wf);

    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render(f, &mut app))?;
    let svg3 = buffer_to_svg(
        terminal.backend().buffer(),
        "M31A — Multi-Agent Swarm & Subagent Governance (F5)",
    );
    std::fs::write("assets/screenshot_agents.svg", &svg3)?;
    svg_to_png(
        "assets/screenshot_agents.svg",
        "assets/screenshot_agents.png",
    );
    println!("✓ Saved assets/screenshot_agents.png");

    // --- SCREENSHOT 4: Interactive Diff Viewer Modal ---
    println!("Rendering Screenshot 4: Interactive Diff Viewer Modal...");
    app.current_screen = Screen::Chat;
    let real_diff = r#"diff --git a/crates/m31a-core/src/prompts.rs b/crates/m31a-core/src/prompts.rs
index 8c6d85e1..d078827e 100644
--- a/crates/m31a-core/src/prompts.rs
+++ b/crates/m31a-core/src/prompts.rs
@@ -4,24 +31,35 @@
 //! Unifies runtime snapshotting, instruction precedence, capability-truthful tool guidance,
 //! hierarchical workspace instruction resolution, dynamic context pinning, and section-level
 //! diffing via [`WorldState`].
+//!
+//! # Core Architectural Invariants
+//! 1. The runtime is the source of truth. The system prompt describes the runtime, never invents it.
+//! 2. Prompt rendering is pure, deterministic, and side-effect-free (no IO or process execution).
+//! 3. Unavailable tools, permissions, and features are never advertised.
+//! 4. Unknown capabilities fail conservatively.
 
-use crate::agent_roles::AgentRole;
+use crate::agent_roles::{AgentRole, AgentRoleContract};
 use crate::world_state::{Placement, WorldState};
 
-/// Collaboration modes controlling autonomous mutation permissions.
+/// Collaboration modes controlling autonomous mutation permissions and agent objectives.
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
+#[serde(rename_all = "kebab-case")]
 pub enum CollaborationMode {
     #[default]
     Default,
     Plan,
+    Review,
 }
 
+/// Supported model families across providers for capability adaptation and behavioral guidance.
+#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
+pub enum ModelFamily {
+    Nemotron35,
+    DeepSeek,
+    CodeStral,
+    Llama,
+    #[default]
+    Generic,
+}
"#;

    app.push_modal(ModalState::diff_viewer(real_diff.to_string()));
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render(f, &mut app))?;
    let svg4 = buffer_to_svg(
        terminal.backend().buffer(),
        "M31A — Interactive Code Review & Diff Viewer (Ctrl+D)",
    );
    std::fs::write("assets/screenshot_diff.svg", &svg4)?;
    svg_to_png("assets/screenshot_diff.svg", "assets/screenshot_diff.png");
    println!("✓ Saved assets/screenshot_diff.png");
    let _ = app.pop_modal();

    // --- SCREENSHOT 5: Tools & MCP Integrations Screen ---
    println!("Rendering Screenshot 5: Tools & Integrations Screen...");
    app.current_screen = Screen::ToolsIntegrations;
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render(f, &mut app))?;
    let svg5 = buffer_to_svg(
        terminal.backend().buffer(),
        "M31A — Tools Registry, MCP & Sandboxing Policies (F8)",
    );
    std::fs::write("assets/screenshot_tools.svg", &svg5)?;
    svg_to_png("assets/screenshot_tools.svg", "assets/screenshot_tools.png");
    println!("✓ Saved assets/screenshot_tools.png");

    println!("All screenshots captured and generated successfully!");
    Ok(())
}
