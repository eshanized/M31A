//! TUI autocomplete / command-rendering repair matrix.
//!
//! Regression coverage for the autocomplete state machine, popup geometry,
//! cursor/placeholder semantics, structured command output, dispatch parity,
//! and responsive framebuffer behavior. Every assertion targets the concrete
//! screenshot failures: empty popup shells, clipped popups, Enter swallowing
//! exact commands, cursor-on-placeholder, and dense `/help` / merged
//! `/config` output.

use std::path::{Path, PathBuf};

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandOutput, SlashCommandRegistry};
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::parser::InteractionParser;
use m31a::interaction::state::SessionPromptState;
use m31a::tui::TuiApplication;
use m31a::tui::composer::{AutocompleteKind, ComposerAction, TuiComposer};
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::theme::{ThemeMode, ThemeTokens};

// ── helpers ────────────────────────────────────────────────────────────────

fn tokens() -> ThemeTokens {
    ThemeTokens::resolve(ThemeMode::Default)
}

fn composer_with_registry() -> TuiComposer {
    TuiComposer::new(PathBuf::from("."))
        .with_slash_registry(std::sync::Arc::new(SlashCommandRegistry::new_standard()))
}

fn type_text(composer: &mut TuiComposer, s: &str) {
    for c in s.chars() {
        composer.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
}

fn type_app_text(app: &mut TuiApplication, s: &str) {
    for c in s.chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
}

fn enter() -> KeyEvent {
    KeyEvent::from(KeyCode::Enter)
}

fn shift_enter() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)
}

/// Render only the composer into a test framebuffer.
fn render_composer(composer: &TuiComposer, w: u16, h: u16) -> (ratatui::buffer::Buffer, String) {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    let toks = tokens();
    terminal
        .draw(|f| composer.render(f, f.area(), &toks))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    let text = (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (buf, text)
}

/// Render the full app into a test framebuffer.
fn render_app(app: &mut TuiApplication, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    app.force_redraw = true;
    app.render_frame(&mut terminal).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn composer_area_for(w: u16, h: u16) -> Rect {
    Rect::new(0, h.saturating_sub(4), w, 4.min(h))
}

fn terminal_rect(w: u16, h: u16) -> Rect {
    Rect::new(0, 0, w, h)
}

// ── 1. autocomplete trigger matrix ─────────────────────────────────────────

#[test]
fn test_autocomplete_opens_for_bare_slash() {
    let mut c = composer_with_registry();
    c.set_text("/");
    assert!(c.is_autocomplete_open(), "'/' must open suggestions");
    assert!(!c.autocomplete_suggestions().is_empty());
    assert_eq!(c.autocomplete_kind(), Some(AutocompleteKind::SlashCommand));
}

#[test]
fn test_autocomplete_progressive_prefix_narrows() {
    for prefix in ["/h", "/he", "/hel"] {
        let mut c = composer_with_registry();
        c.set_text(prefix);
        assert!(
            c.is_autocomplete_open(),
            "{prefix} must keep suggestions open"
        );
        assert!(
            c.autocomplete_suggestions()
                .iter()
                .any(|s| s.label == "/help"),
            "{prefix} must suggest /help"
        );
    }
    // Narrowing must actually narrow.
    let mut c1 = composer_with_registry();
    c1.set_text("/h");
    let mut c2 = composer_with_registry();
    c2.set_text("/hel");
    assert!(c2.autocomplete_suggestions().len() <= c1.autocomplete_suggestions().len());
}

#[test]
fn test_autocomplete_exact_help_still_consistent() {
    let mut c = composer_with_registry();
    c.set_text("/help");
    // Exact input may keep the popup (single exact match) but the state must
    // stay consistent: open implies non-empty, selection valid.
    if c.is_autocomplete_open() {
        assert!(!c.autocomplete_suggestions().is_empty());
        assert!(c.autocomplete_selected_index() < c.autocomplete_suggestions().len());
    }
    // And Enter must submit it with a single keystroke (Case B).
    let action = c.handle_key(enter());
    match action {
        ComposerAction::Submit(text) => assert_eq!(text, "/help"),
        ComposerAction::None => {
            // Accepted a completion instead — then the text must be the exact
            // command and a second Enter must submit.
            assert_eq!(c.text(), "/help");
            let second = c.handle_key(enter());
            assert_eq!(second, ComposerAction::Submit("/help".to_string()));
        }
        ComposerAction::Cancel => panic!("exact /help must never cancel"),
    }
}

#[test]
fn test_autocomplete_model_arg_values() {
    let mut c = composer_with_registry();
    c.set_text("/model ");
    assert!(c.is_autocomplete_open(), "'/model ' must suggest models");
    assert_eq!(c.autocomplete_kind(), Some(AutocompleteKind::SlashCommand));
    assert!(
        c.autocomplete_suggestions()
            .iter()
            .all(|s| s.insert_text.starts_with("/model "))
    );
}

#[test]
fn test_autocomplete_profile_arg_values() {
    let mut c = composer_with_registry();
    c.set_text("/profile ");
    assert!(c.is_autocomplete_open());
    let labels: Vec<_> = c
        .autocomplete_suggestions()
        .iter()
        .map(|s| s.label.clone())
        .collect();
    assert!(labels.contains(&"autonomous".to_string()));
    c.set_text("/profile a");
    assert!(c.is_autocomplete_open());
    assert!(
        c.autocomplete_suggestions()
            .iter()
            .all(|s| s.label.starts_with('a'))
    );
}

#[test]
fn test_autocomplete_mentions() {
    let mut c = composer_with_registry();
    c.set_text("@");
    // Workspace-dependent: when files are cached, '@' opens; otherwise the
    // composer must be closed-but-consistent (never an empty shell).
    if c.is_autocomplete_open() {
        assert!(!c.autocomplete_suggestions().is_empty());
        assert_eq!(c.autocomplete_kind(), Some(AutocompleteKind::Mention));
    } else {
        assert!(c.autocomplete_suggestions().is_empty());
        assert!(
            c.popup_rect(terminal_rect(120, 30), composer_area_for(120, 30))
                .is_none()
        );
    }
    let mut c2 = composer_with_registry();
    c2.set_text("fix @src/");
    if c2.is_autocomplete_open() {
        assert_eq!(c2.autocomplete_kind(), Some(AutocompleteKind::Mention));
        assert!(!c2.autocomplete_suggestions().is_empty());
    }
}

#[test]
fn test_autocomplete_never_open_but_empty() {
    // The empty-shell invariant across the full lifecycle matrix.
    for input in [
        "/",
        "/h",
        "/he",
        "/hel",
        "/help",
        "/model",
        "/model ",
        "/profile",
        "/profile a",
        "@",
        "fix @src/",
        "/zzz_no_such_command_xyz",
        "",
        "plain natural language",
    ] {
        let mut c = composer_with_registry();
        c.set_text(input);
        if c.is_autocomplete_open() {
            assert!(
                !c.autocomplete_suggestions().is_empty(),
                "open popup must never be empty for input {input:?}"
            );
            assert!(
                c.autocomplete_selected_index() < c.autocomplete_suggestions().len(),
                "selection must be valid for input {input:?}"
            );
            assert!(
                c.popup_rect(terminal_rect(120, 30), Rect::new(0, 26, 120, 4))
                    .is_some(),
                "open popup must have viewport-bounded geometry for {input:?}"
            );
        } else {
            assert!(
                c.autocomplete_suggestions().is_empty(),
                "closed popup must carry no items for {input:?}"
            );
            assert!(
                c.popup_rect(terminal_rect(120, 30), Rect::new(0, 26, 120, 4))
                    .is_none(),
                "closed popup must have no geometry for {input:?}"
            );
        }
    }
}

// ── 2. Enter state machine (Cases A–E) ─────────────────────────────────────

#[test]
fn test_case_a_partial_accepts_without_submitting() {
    let mut c = composer_with_registry();
    c.set_text("/hel");
    assert!(c.is_autocomplete_open());
    let action = c.handle_key(enter());
    assert_eq!(
        action,
        ComposerAction::None,
        "partial must accept, not submit"
    );
    assert_eq!(c.text(), "/help", "no trailing space after bare completion");
}

#[test]
fn test_case_b_exact_submits_with_single_enter() {
    let mut c = composer_with_registry();
    // Type char-by-char like a real user (not set_text).
    type_text(&mut c, "/help");
    let action = c.handle_key(enter());
    assert_eq!(
        action,
        ComposerAction::Submit("/help".to_string()),
        "exact /help must submit with a single Enter"
    );
}

#[test]
fn test_case_c_command_with_args_submits_normally() {
    let mut c = composer_with_registry();
    type_text(&mut c, "/help foo");
    let action = c.handle_key(enter());
    assert_eq!(
        action,
        ComposerAction::Submit("/help foo".to_string()),
        "command + args must submit normally"
    );
}

#[test]
fn test_case_d_explicit_navigation_accepts() {
    let mut c = composer_with_registry();
    c.set_text("/");
    let count = c.autocomplete_suggestions().len();
    assert!(count >= 2, "need multiple suggestions for navigation test");
    c.handle_key(KeyEvent::from(KeyCode::Down));
    assert!(c.autocomplete_user_navigated());
    assert_eq!(c.autocomplete_selected_index(), 1);
    let selected = c.selected_suggestion().unwrap().insert_text.clone();
    let action = c.handle_key(enter());
    assert_eq!(action, ComposerAction::None, "navigated Enter must accept");
    assert_eq!(c.text(), selected.trim_end());
}

#[test]
fn test_case_e_esc_preserves_input() {
    let mut c = composer_with_registry();
    c.set_text("/hel");
    assert!(c.is_autocomplete_open());
    let action = c.handle_key(KeyEvent::from(KeyCode::Esc));
    assert_eq!(action, ComposerAction::None);
    assert!(!c.is_autocomplete_open());
    assert_eq!(c.text(), "/hel", "Esc must never delete user input");
}

#[test]
fn test_tab_always_accepts() {
    let mut c = composer_with_registry();
    c.set_text("/sta");
    let action = c.handle_key(KeyEvent::from(KeyCode::Tab));
    assert_eq!(action, ComposerAction::None);
    assert_eq!(c.text(), "/status");
    assert!(!c.is_autocomplete_open());
}

#[test]
fn test_shift_enter_inserts_newline() {
    let mut c = composer_with_registry();
    c.set_text("first");
    let action = c.handle_key(shift_enter());
    assert_eq!(action, ComposerAction::None);
    assert!(c.text().contains('\n'), "Shift+Enter must insert newline");
}

#[test]
fn test_up_down_wrap_around() {
    let mut c = composer_with_registry();
    c.set_text("/");
    let n = c.autocomplete_suggestions().len();
    assert!(n >= 2);
    c.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(
        c.autocomplete_selected_index(),
        n - 1,
        "Up from 0 wraps to last"
    );
    c.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(
        c.autocomplete_selected_index(),
        0,
        "Down from last wraps to 0"
    );
}

// ── 3. completion whitespace + cursor ──────────────────────────────────────

#[test]
fn test_bare_completion_has_no_trailing_space() {
    let mut c = composer_with_registry();
    c.set_text("/hel");
    c.handle_key(KeyEvent::from(KeyCode::Tab));
    assert_eq!(c.text(), "/help");
}

#[test]
fn test_model_arg_completion_expands_full_line() {
    let mut c = composer_with_registry();
    c.set_text("/model ");
    assert!(c.is_autocomplete_open());
    let first = c.selected_suggestion().unwrap().insert_text.clone();
    assert!(first.starts_with("/model "));
    c.accept_selected();
    assert_eq!(c.text(), first, "arg completion must produce the full line");
    assert!(!c.text().ends_with(' '), "no blind trailing whitespace");
}

#[test]
fn test_mention_completion_replaces_only_token() {
    let mut c = composer_with_registry();
    c.set_text("@");
    if !c.is_autocomplete_open() {
        return; // No cached files in this environment; nothing to replace.
    }
    let file = c.selected_suggestion().unwrap().insert_text.clone();
    c.set_text(&format!("fix @{file} and more"));
    // Token followed by text is no longer live → popup closes, text kept.
    assert!(!c.is_autocomplete_open());
    let mut c2 = composer_with_registry();
    c2.set_text("fix @");
    if !c2.is_autocomplete_open() {
        return;
    }
    let pick = c2.selected_suggestion().unwrap().insert_text.clone();
    c2.accept_selected();
    assert_eq!(c2.text(), format!("fix @{pick}"));
    assert!(c2.text().starts_with("fix @"));
}

// ── 4. popup geometry ──────────────────────────────────────────────────────

#[test]
fn test_popup_width_adapts_to_terminal() {
    for w in [80u16, 100, 120, 160] {
        let mut c = composer_with_registry();
        c.set_text("/");
        let rect = c
            .popup_rect(terminal_rect(w, 30), composer_area_for(w, 30))
            .expect("popup must exist for '/'");
        assert!(rect.width >= 20, "popup usable at width {w}");
        assert!(
            rect.x + rect.width <= w,
            "popup must not exceed terminal width {w}"
        );
        assert!(
            rect.y + rect.height <= 30,
            "popup must not exceed terminal height at {w}"
        );
        // No hardcoded universal 50-column rule: narrow terminals shrink it.
        if w == 80 {
            assert!(rect.width <= 76, "80-col popup must respect margins");
        }
    }
}

#[test]
fn test_popup_height_reflects_visible_rows_and_scrolls() {
    let mut c = composer_with_registry();
    c.set_text("/");
    let total = c.autocomplete_suggestions().len();
    assert!(total > 8, "need overflow to test scrolling, got {total}");
    let rect = c
        .popup_rect(terminal_rect(120, 30), composer_area_for(120, 30))
        .unwrap();
    // 8 visible rows + 2 border rows.
    assert_eq!(rect.height, 10, "popup height must reflect visible rows");
    // Drive selection to the end; it must stay visible via scrolling.
    for _ in 0..total {
        c.handle_key(KeyEvent::from(KeyCode::Down));
    }
    let visible = 8usize;
    let scroll = c.autocomplete_scroll_offset();
    let selected = c.autocomplete_selected_index();
    assert!(
        selected >= scroll && selected < scroll + visible,
        "selected {selected} must be inside window [{scroll}, {})",
        scroll + visible
    );
}

#[test]
fn test_long_descriptions_are_clipped_in_framebuffer() {
    // Single-match popup: content must be the real suggestion…
    let mut c = composer_with_registry();
    c.set_text("/hel");
    assert!(c.is_autocomplete_open());
    let full_desc = c
        .autocomplete_suggestions()
        .iter()
        .find(|s| s.label == "/help")
        .map(|s| s.description.clone())
        .unwrap();
    assert!(full_desc.chars().count() > 20);
    let (buf, text) = render_composer(&c, 80, 30);
    assert!(
        text.contains("/help"),
        "popup must show real suggestion content"
    );
    // …and no rendered line exceeds the terminal width.
    for (y, line) in text.lines().enumerate() {
        assert!(
            line.chars().count() <= 80,
            "row {y} exceeds 80 cols: {:?}",
            line.chars().take(90).collect::<String>()
        );
    }
    // Popup rect itself is bounded.
    let rect = c
        .popup_rect(terminal_rect(80, 30), composer_area_for(80, 30))
        .unwrap();
    assert!(rect.x + rect.width <= 80);
    let _ = (buf, full_desc);
}

#[test]
fn test_overflow_popup_shows_first_suggestion_and_scroll_hint() {
    let mut c = composer_with_registry();
    c.set_text("/");
    let total = c.autocomplete_suggestions().len();
    assert!(total > 8);
    // Fullscreen composer leaves no room above, so the popup clamps to a
    // small but valid window with a scroll indicator instead of an
    // empty/broken shell.
    let rect = c
        .popup_rect(terminal_rect(80, 30), terminal_rect(80, 30))
        .unwrap();
    assert!(rect.x + rect.width <= 80);
    assert!(rect.y + rect.height <= 30);
    let (_, text) = render_composer(&c, 80, 30);
    let first_label = c.autocomplete_suggestions()[0].label.clone();
    assert!(
        text.contains(&first_label),
        "visible suggestion {first_label} must render, got:\n{text}"
    );
}

#[test]
fn test_single_suggestion_renders_correctly() {
    let mut c = composer_with_registry();
    // Narrow to (almost) a single match.
    c.set_text("/clear-sessio");
    assert!(c.is_autocomplete_open());
    assert_eq!(c.autocomplete_suggestions().len(), 1);
    let rect = c
        .popup_rect(terminal_rect(100, 30), composer_area_for(100, 30))
        .unwrap();
    assert_eq!(rect.height, 3, "one row + two border rows");
    let (_, text) = render_composer(&c, 100, 30);
    assert!(text.contains("/clear-session"));
}

#[test]
fn test_popup_anchored_above_composer_when_room() {
    let mut c = composer_with_registry();
    c.set_text("/");
    let composer_area = composer_area_for(120, 30);
    let rect = c.popup_rect(terminal_rect(120, 30), composer_area).unwrap();
    assert!(
        rect.y + rect.height <= composer_area.y,
        "popup must sit above the composer when room allows"
    );
}

// ── 5. cursor + placeholder ────────────────────────────────────────────────

#[test]
fn test_empty_composer_shows_placeholder_intact() {
    let c = composer_with_registry();
    assert_eq!(c.text(), "");
    let (_, text) = render_composer(&c, 120, 30);
    assert!(
        text.contains("Ask M31A to build, inspect, fix, or explain..."),
        "placeholder must render intact, got:\n{text}"
    );
    // The placeholder must not be faked with cursor artifacts.
    assert!(!text.contains("Ask M31A to build, inspect, fix, or explain..._"));
}

#[test]
fn test_typing_replaces_placeholder_with_real_text() {
    let mut c = composer_with_registry();
    type_text(&mut c, "/hel");
    let (_, text) = render_composer(&c, 120, 30);
    assert!(text.contains("/hel"));
    // Placeholder disappears once the user types.
    assert!(!text.contains("Ask M31A to build"));
}

#[test]
fn test_multiline_cursor_line_tracking() {
    let mut c = composer_with_registry();
    c.set_text("line one\nline two");
    // Move cursor up: implementation detail is exercised via no-panic
    // rendering at the second line.
    let (_, text) = render_composer(&c, 120, 30);
    assert!(text.contains("line one"));
    assert!(text.contains("line two"));
}

// ── 6. command output + /help rendering ───────────────────────────────────

#[test]
fn test_help_lists_every_command_stacked_not_tabular() {
    let reg = SlashCommandRegistry::new_standard();
    let help = reg.generate_help(None);
    assert!(help.contains("Available commands"));
    for cmd in reg.commands() {
        assert!(
            help.contains(&format!("/{}\n", cmd.name)),
            "help must list /{} as its own stacked entry",
            cmd.name
        );
    }
    // No fixed-width table cells: the old `  /cmd<16> usage<24>` shape is gone.
    for line in help.lines() {
        assert!(
            line.chars().count() <= 80,
            "help line exceeds 80 cols: {line:?}"
        );
    }
}

#[test]
fn test_help_detail_shows_structured_sections() {
    let reg = SlashCommandRegistry::new_standard();
    let detail = reg.generate_help(Some("plan"));
    for section in ["Command", "Usage", "Aliases", "Effect", "Description"] {
        assert!(
            detail.contains(section),
            "missing section {section}:\n{detail}"
        );
    }
    let unknown = reg.generate_help(Some("nope_nonexistent"));
    assert!(unknown.contains("Unknown command"));
}

#[test]
fn test_help_wraps_at_narrow_widths() {
    let reg = SlashCommandRegistry::new_standard();
    for width in [40usize, 60, 80] {
        let help = reg.generate_help_for_width(None, width);
        for line in help.lines() {
            assert!(
                line.chars().count() <= width,
                "help line exceeds {width} cols: {line:?}"
            );
        }
        let detail = reg.generate_help_for_width(Some("workflow"), width);
        for line in detail.lines() {
            assert!(
                line.chars().count() <= width,
                "detail line exceeds {width} cols: {line:?}"
            );
        }
    }
}

#[test]
fn test_system_command_output_splits_into_lines_and_wraps() {
    let reg = SlashCommandRegistry::new_standard();
    let help = reg.generate_help(None);
    let toks = tokens();
    for width in [80u16, 100, 120, 160, 220] {
        let item = TuiConversationItem::System {
            text: help.clone(),
            timestamp: Utc::now(),
        };
        let lines = item.render_lines(width, &toks);
        assert!(
            lines.len() > 10,
            "help output must be structured lines, not one merged line (width {width})"
        );
        for line in &lines {
            let w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(
                w <= width as usize,
                "conversation line exceeds {width} cols ({w}): {line:?}"
            );
        }
    }
}

#[test]
fn test_long_values_wrap_without_collision() {
    let toks = tokens();
    let long_path = format!("/home/user/{}", "very-long-workspace-name/".repeat(20));
    assert!(long_path.chars().count() > 200);
    let text =
        format!("Workspace\n  {long_path}\n\nModel\n  deepseek-v4.1-flash-extended-identifier");
    for width in [80u16, 96, 100, 120, 160, 220] {
        let item = TuiConversationItem::System {
            text: text.clone(),
            timestamp: Utc::now(),
        };
        let lines = item.render_lines(width, &toks);
        assert!(lines.len() >= 4, "fields must stay separated at {width}");
        for line in &lines {
            let w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(
                w <= width as usize,
                "line escapes region at {width}: {line:?}"
            );
        }
        // Labels survive wrapping.
        let joined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Workspace"), "field label lost at {width}");
        assert!(joined.contains("Model"), "field label lost at {width}");
    }
}

#[test]
fn test_config_output_shape_is_readable() {
    // Shape contract: stacked `Label\\n  value` blocks, no `Key:   value` merge.
    let reg = SlashCommandRegistry::new_standard();
    let help = reg.generate_help(Some("config"));
    assert!(help.contains("Usage"));
    let toks = tokens();
    let item = TuiConversationItem::System {
        text: "Configuration\n\nWorkspace\n  /home/user/project\n\nModel\n  deepseek-v4.1-flash\n"
            .to_string(),
        timestamp: Utc::now(),
    };
    let lines = item.render_lines(80, &toks);
    let joined: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("Workspace"));
    assert!(joined.contains("/home/user/project"));
}

// ── 7. registry authority (no hardcoded UI command lists) ──────────────────

#[test]
fn test_autocomplete_derives_from_registry_not_hardcoded() {
    let reg = SlashCommandRegistry::new_standard();
    let mut c = composer_with_registry();
    c.set_text("/");
    let labels: Vec<String> = c
        .autocomplete_suggestions()
        .iter()
        .map(|s| s.label.clone())
        .collect();
    // Every suggestion resolves in the registry…
    for label in &labels {
        assert!(
            reg.find(label).is_some(),
            "suggestion {label} is not registry-backed"
        );
    }
    // …and every registry command (plus aliases) is discoverable.
    for cmd in reg.commands() {
        assert!(
            labels.contains(&format!("/{}", cmd.name)),
            "/{} missing from autocomplete",
            cmd.name
        );
        for alias in &cmd.aliases {
            assert!(
                labels.contains(&format!("/{alias}")),
                "alias /{alias} missing from autocomplete"
            );
        }
    }
}

#[test]
fn test_help_derives_from_registry() {
    let reg = SlashCommandRegistry::new_standard();
    let help = reg.generate_help(None);
    for cmd in reg.commands() {
        assert!(help.contains(&cmd.description[..12.min(cmd.description.len())]));
    }
}

// ── 8. command parity matrix ───────────────────────────────────────────────

fn all_command_names() -> Vec<(String, Vec<String>)> {
    SlashCommandRegistry::new_standard()
        .commands()
        .iter()
        .map(|c| (c.name.clone(), c.aliases.clone()))
        .collect()
}

#[test]
fn test_every_command_parses_to_an_action() {
    let parser = InteractionParser::default();
    let ws = Path::new(".");
    for (name, aliases) in all_command_names() {
        let action = parser.parse(
            &format!("/{name}"),
            ws,
            SessionPromptState::Idle,
            None,
            false,
        );
        assert!(
            action.is_some(),
            "/{name} must parse to an ApplicationAction"
        );
        for alias in aliases {
            let action = parser.parse(
                &format!("/{alias}"),
                ws,
                SessionPromptState::Idle,
                None,
                false,
            );
            assert!(
                action.is_some(),
                "alias /{alias} must parse to an ApplicationAction"
            );
        }
    }
}

#[test]
fn test_every_command_autocompletes_and_selects() {
    for (name, aliases) in all_command_names() {
        // Prefix without the last char matches at least the command itself.
        let prefix = format!("/{}", &name[..name.len().saturating_sub(1)]);
        let mut c = composer_with_registry();
        c.set_text(&prefix);
        assert!(
            c.is_autocomplete_open(),
            "prefix {prefix:?} must open autocomplete"
        );
        assert!(
            c.autocomplete_suggestions()
                .iter()
                .any(|s| s.label == format!("/{name}")),
            "/{name} must be suggested for {prefix:?}"
        );
        for alias in aliases {
            if alias.len() < 2 {
                continue;
            }
            let aprefix = format!("/{}", &alias[..alias.len() - 1]);
            let mut ca = composer_with_registry();
            ca.set_text(&aprefix);
            if ca.is_autocomplete_open() {
                assert!(
                    ca.autocomplete_suggestions()
                        .iter()
                        .any(|s| s.label == format!("/{alias}")),
                    "alias /{alias} must be suggested for {aprefix:?}"
                );
            }
        }
    }
}

#[test]
fn test_unknown_command_is_readable_error() {
    let parser = InteractionParser::default();
    let action = parser
        .parse(
            "/frobnicate_xyz",
            Path::new("."),
            SessionPromptState::Idle,
            None,
            false,
        )
        .expect("unknown slash must still produce an action");
    match action {
        ApplicationAction::SlashCommandSubmitted { command, .. } => {
            assert_eq!(command, "frobnicate_xyz");
        }
        other => panic!("unexpected action for unknown command: {other:?}"),
    }
}

#[test]
fn test_invalid_args_are_readable_errors_via_parser() {
    // Parser level: malformed-but-slash input still routes deterministically.
    let parser = InteractionParser::default();
    assert!(
        parser
            .parse("/", Path::new("."), SessionPromptState::Idle, None, false)
            .is_none_or(|a| matches!(
                a,
                ApplicationAction::UserTextSubmitted(_)
                    | ApplicationAction::SlashCommandSubmitted { .. }
            ))
    );
}

// ── registry execution parity (in-memory pool, no DB files) ───────────────

async fn memory_pool() -> sqlx::SqlitePool {
    sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory pool")
}

fn test_ctx<'a>(
    pool: &'a sqlx::SqlitePool,
    bus: &'a std::sync::Arc<m31a::events::bus::BroadcastEventBus>,
    reg: &'a SlashCommandRegistry,
    ws: &'a Path,
) -> m31a::interaction::commands::CommandContext<'a> {
    m31a::interaction::commands::CommandContext {
        workspace_root: ws,
        session_id: None,
        active_mission_id: None,
        pool,
        event_bus: bus,
        configured_model: "test-model".to_string(),
        configured_provider: "test-provider".to_string(),
        active_profile: "autonomous".to_string(),
        tool_registry: None,
        command_registry: Some(reg),
    }
}

#[tokio::test]
async fn test_registry_executes_help_and_unknown() {
    let reg = SlashCommandRegistry::new_standard();
    let pool = memory_pool().await;
    let bus = std::sync::Arc::new(m31a::events::bus::BroadcastEventBus::new(16));
    let ws = Path::new(".");
    let ctx = test_ctx(&pool, &bus, &reg, ws);

    match reg.execute_line("/help", &ctx).await.unwrap() {
        CommandOutput::Info(text) => {
            assert!(text.contains("Available commands"));
            assert!(text.contains("/status"));
        }
        other => panic!("expected Info, got {other:?}"),
    }
    match reg.execute_line("/help status", &ctx).await.unwrap() {
        CommandOutput::Info(text) => assert!(text.contains("Usage")),
        other => panic!("expected Info, got {other:?}"),
    }
    match reg.execute_line("/frobnicate_xyz", &ctx).await.unwrap() {
        CommandOutput::Error(msg) => assert!(msg.contains("Unknown command")),
        other => panic!("expected Error, got {other:?}"),
    }
    // Alias parity at execution level.
    match reg.execute_line("/?", &ctx).await.unwrap() {
        CommandOutput::Info(text) => assert!(text.contains("Available commands")),
        other => panic!("expected Info for /?, got {other:?}"),
    }
}

#[tokio::test]
async fn test_registry_rejects_invalid_model_profile() {
    let reg = SlashCommandRegistry::new_standard();
    let pool = memory_pool().await;
    let bus = std::sync::Arc::new(m31a::events::bus::BroadcastEventBus::new(16));
    let ws = Path::new(".");
    let ctx = test_ctx(&pool, &bus, &reg, ws);

    match reg.execute_line("/model openai/gpt-4", &ctx).await.unwrap() {
        CommandOutput::Error(msg) => assert!(!msg.is_empty()),
        other => panic!("expected Error, got {other:?}"),
    }
    match reg
        .execute_line("/profile nope_not_a_profile", &ctx)
        .await
        .unwrap()
    {
        CommandOutput::Error(msg) => assert!(msg.contains("Unknown profile")),
        other => panic!("expected Error, got {other:?}"),
    }
    match reg
        .execute_line("/config definitely_not_a_key_xyz", &ctx)
        .await
        .unwrap()
    {
        CommandOutput::Error(msg) => assert!(msg.contains("not found")),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_registry_valid_forms_return_actions_or_info() {
    let reg = SlashCommandRegistry::new_standard();
    let pool = memory_pool().await;
    let bus = std::sync::Arc::new(m31a::events::bus::BroadcastEventBus::new(16));
    let ws = Path::new(".");
    let ctx = test_ctx(&pool, &bus, &reg, ws);

    // (input, expects Action?)
    let cases = [
        ("/diff", true),
        ("/d", true),
        ("/commit ship it", true),
        ("/cancel", true),
        ("/clear", true),
        ("/clear-session", true),
        ("/exit", true),
        ("/quit", true),
        ("/profile autonomous", true),
        ("/genesis build a widget", true),
        ("/plan accept", true),
        ("/tasks accept", true),
        ("/task remove T1", true),
        ("/authorize yes", true),
        ("/workflow inspect run-1", true),
        ("/genesis", false), // usage error
        ("/plan", false),    // usage info
        ("/tasks", false),   // usage info
        ("/task", false),    // usage info
        ("/authorize", false),
        ("/workflow", false),
        ("/model", false),   // status info
        ("/profile", false), // status info
        ("/config", false),  // status info
        ("/skills", false),  // inventory info
        ("/roadmap", false), // info or genesis-state action
    ];
    for (input, expect_action) in cases {
        let out = reg.execute_line(input, &ctx).await.unwrap();
        match (&out, expect_action) {
            (CommandOutput::ApplicationAction(_), true) => {}
            (CommandOutput::Info(_), false) => {}
            (CommandOutput::Error(_), false) => {}
            (CommandOutput::ApplicationAction(_), false) if input == "/roadmap" => {}
            _ => panic!("{input} produced unexpected output: {out:?}"),
        }
    }
}

#[tokio::test]
async fn test_registry_status_alias_parity() {
    let reg = SlashCommandRegistry::new_standard();
    let pool = memory_pool().await;
    let bus = std::sync::Arc::new(m31a::events::bus::BroadcastEventBus::new(16));
    let ws = Path::new(".");
    let ctx = test_ctx(&pool, &bus, &reg, ws);
    // /status executes against an empty in-memory DB: must still answer with
    // a readable stacked card (missing tables degrade to defaults).
    match reg.execute_line("/status", &ctx).await.unwrap() {
        CommandOutput::Info(text) => {
            assert!(text.contains("Session"));
            assert!(text.contains("Workspace"));
        }
        other => panic!("expected Info, got {other:?}"),
    }
    match reg.execute_line("/st", &ctx).await.unwrap() {
        CommandOutput::Info(text) => assert!(text.contains("Session")),
        other => panic!("expected Info for /st, got {other:?}"),
    }
}

// ── 9. TUI dispatch parity (composer → parser → bridge) ────────────────────

#[test]
fn test_app_exact_help_submits_to_bridge() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = TuiApplication::new()
        .with_composer_focused(true)
        .with_bridge_tx(tx);
    type_app_text(&mut app, "/help");
    app.handle_key(enter());
    match rx.try_recv() {
        Ok(ApplicationAction::SlashCommandSubmitted { command, args }) => {
            assert_eq!(command, "help");
            assert!(args.is_empty());
        }
        other => panic!("expected SlashCommandSubmitted, got {other:?}"),
    }
}

#[test]
fn test_app_partial_help_accepts_instead_of_submitting() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = TuiApplication::new()
        .with_composer_focused(true)
        .with_bridge_tx(tx);
    type_app_text(&mut app, "/hel");
    assert!(app.composer.is_autocomplete_open());
    app.handle_key(enter());
    assert!(
        rx.try_recv().is_err(),
        "partial /hel + Enter must accept, not dispatch"
    );
    assert_eq!(app.composer.text(), "/help");
    // A second Enter now submits the exact command.
    app.handle_key(enter());
    match rx.try_recv() {
        Ok(ApplicationAction::SlashCommandSubmitted { command, .. }) => {
            assert_eq!(command, "help")
        }
        other => panic!("expected submission on second Enter, got {other:?}"),
    }
}

#[test]
fn test_app_esc_closes_popup_preserving_text() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    type_app_text(&mut app, "/hel");
    assert!(app.composer.is_autocomplete_open());
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.composer.is_autocomplete_open());
    assert_eq!(app.composer.text(), "/hel");
}

#[test]
fn test_app_doctor_navigates_without_bridge_error() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = TuiApplication::new()
        .with_composer_focused(true)
        .with_bridge_tx(tx);
    type_app_text(&mut app, "/doctor");
    let cmd = app.handle_key(enter());
    assert!(
        matches!(cmd, Some(m31a::cli::RuntimeCommand::RunDoctor { .. })),
        "doctor dispatches its view-owned diagnostic"
    );
    assert_eq!(
        app.navigation.current_screen,
        m31a::tui::navigation::ScreenId::Doctor
    );
}

#[test]
fn test_app_help_fallback_uses_registry_not_hardcoded_list() {
    // No bridge: degraded help must still derive from the registry.
    let mut app = TuiApplication::new().with_composer_focused(true);
    assert!(app.bridge_tx.is_none());
    type_app_text(&mut app, "/help");
    app.handle_key(enter());
    let system_texts: Vec<String> = app
        .model
        .conversation
        .iter()
        .filter_map(|i| match i {
            TuiConversationItem::System { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        !system_texts.is_empty(),
        "degraded /help must still answer visibly"
    );
    let joined = system_texts.join("\n");
    // Registry-derived proof: the old hardcoded fallback never mentioned
    // these registry commands.
    assert!(
        joined.contains("/workflow"),
        "fallback must be registry-derived"
    );
    assert!(
        joined.contains("/model"),
        "fallback must be registry-derived"
    );
}

#[test]
fn test_app_ctrl_p_opens_palette() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.palette.is_open, "Ctrl+P must open the command palette");
}

#[test]
fn test_command_output_reaches_conversation_projection() {
    let mut app = TuiApplication::new();
    let reg = SlashCommandRegistry::new_standard();
    let help = reg.generate_help(None);
    app.model
        .apply_interaction_event(&InteractionEvent::CommandOutput { text: help.clone() });
    assert!(
        app.model.conversation.iter().any(|i| matches!(
            i,
            TuiConversationItem::System { text, .. } if text == &help
        )),
        "CommandOutput must reach the conversation projection"
    );
    app.model.apply_interaction_event(&InteractionEvent::Error {
        message: "boom".to_string(),
    });
    assert!(
        app.model.conversation.iter().any(|i| matches!(
            i,
            TuiConversationItem::Error { message, .. } if message == "boom"
        )),
        "errors must be visible in the conversation"
    );
    let content = render_app(&mut app, 120, 30);
    // The stream auto-follows, so the tail (last command + error) is visible.
    assert!(content.contains("/settings"));
    assert!(content.contains("/help"));
    assert!(content.contains("boom"));
}

// ── 10. responsive framebuffer matrix ───────────────────────────────────────

#[test]
fn test_responsive_matrix_no_collision() {
    for (w, h) in [
        (80u16, 24u16),
        (96, 24),
        (100, 30),
        (120, 30),
        (120, 36),
        (160, 40),
        (220, 50),
    ] {
        // Scenario: help output in history + autocomplete open in composer.
        let mut app = TuiApplication::new().with_composer_focused(true);
        let reg = SlashCommandRegistry::new_standard();
        app.model
            .apply_interaction_event(&InteractionEvent::CommandOutput {
                text: reg.generate_help(None),
            });
        app.composer.set_text("/h");
        let content = render_app(&mut app, w, h);
        assert!(
            content.contains("/settings"),
            "{w}x{h}: help output tail must be visible"
        );
        // Popup geometry stays inside the viewport.
        if app.composer.is_autocomplete_open() {
            let composer_area = Rect::new(0, h.saturating_sub(4 + 1 + 3), w, 4);
            if let Some(rect) = app
                .composer
                .popup_rect(Rect::new(0, 0, w, h), composer_area)
            {
                assert!(
                    rect.x + rect.width <= w && rect.y + rect.height <= h,
                    "{w}x{h}: popup {rect:?} escapes viewport"
                );
            }
        }
        // Every physical row fits its viewport (no unbounded string escapes).
        for (y, line) in content.lines().enumerate() {
            assert!(
                line.chars().count() <= w as usize,
                "{w}x{h} row {y} escapes region"
            );
        }
    }
}

#[test]
fn test_config_status_scenarios_at_120x30() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model.apply_interaction_event(&InteractionEvent::CommandOutput {
        text: "Configuration\n\nWorkspace\n  /home/user/some-rather-long-workspace-path-that-keeps-going\n\nModel\n  deepseek-v4.1-flash\n\nProvider\n  nvidia_nim\n".to_string(),
    });
    app.model.apply_interaction_event(&InteractionEvent::CommandOutput {
        text: "Session status\n\nSession\n  none (ephemeral)\n\nTasks\n  0/0 completed\n\nGit state\n  clean\n".to_string(),
    });
    let content = render_app(&mut app, 120, 30);
    // The stream auto-follows: the second (status) card tail is visible and
    // every field stays separated.
    for marker in ["Session status", "Tasks", "Git state", "clean"] {
        assert!(content.contains(marker), "missing {marker}:\n{content}");
    }
}

#[test]
fn test_conversation_prefers_speakers_over_badges() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model.add_conversation_item(TuiConversationItem::User {
        id: "u".to_string(),
        sequence: 1,
        text: "/config".to_string(),
        mentions: vec![],
        timestamp: Utc::now(),
    });
    let content = render_app(&mut app, 120, 30);
    assert!(content.contains("You"));
    assert!(
        !content.contains("[USER #"),
        "raw log badges must not dominate"
    );
}
