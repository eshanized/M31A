//! Models Surface.
//!
//! Renders live model provider state and allows switching providers/models
//! via canonical runtime actions.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Model selector render mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelSelectorMode {
    #[default]
    Providers,
    Models,
    CurrentConfig,
}

/// State for model selector interaction.
#[derive(Debug, Clone, Default)]
pub struct ModelSelectorState {
    pub mode: ModelSelectorMode,
    pub selected_index: usize,
    pub list_state: ListState,
}

impl ModelSelectorState {
    pub fn new() -> Self {
        let mut state = Self::default();
        state.list_state.select(Some(0));
        state
    }
}

/// Model provider info for display.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub is_current: bool,
    pub is_available: bool,
    pub model_count: usize,
    pub base_url: Option<String>,
}

/// Model info for display.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model_id: String,
    pub display_name: Option<String>,
    pub tier: String,
    pub context_capacity: usize,
    pub supports_tools: bool,
    pub is_current_primary: bool,
    pub is_current_fast: bool,
    pub availability: String,
}

/// Render the Model Selector surface.
#[allow(clippy::too_many_arguments)]
pub fn render_model_selector(
    f: &mut Frame,
    area: Rect,
    providers: &[ProviderInfo],
    models: &[ModelInfo],
    current_provider: &str,
    current_model: &str,
    current_fast_model: Option<&str>,
    tokens: &ThemeTokens,
    is_focused: bool,
    ui_state: &mut ModelSelectorState,
) {
    if area.width < 20 || area.height < 8 {
        return;
    }

    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let border_style = if is_focused {
        tokens.border_focused
    } else if is_mono {
        Style::default()
    } else {
        tokens.border_default
    };

    // Header with current config
    let header_height = 5;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_height), Constraint::Min(10)])
        .split(area);

    render_model_selector_header(
        f,
        chunks[0],
        current_provider,
        current_model,
        current_fast_model,
        tokens,
        border_style,
        ui_state.mode,
    );

    // Main content area based on view mode
    match ui_state.mode {
        ModelSelectorMode::Providers => {
            render_providers_list(f, chunks[1], providers, tokens, border_style, ui_state)
        }
        ModelSelectorMode::Models => {
            render_models_list(f, chunks[1], models, tokens, border_style, ui_state)
        }
        ModelSelectorMode::CurrentConfig => render_current_config(
            f,
            chunks[1],
            current_provider,
            current_model,
            current_fast_model,
            tokens,
            border_style,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_model_selector_header(
    f: &mut Frame,
    area: Rect,
    current_provider: &str,
    current_model: &str,
    current_fast_model: Option<&str>,
    tokens: &ThemeTokens,
    border_style: Style,
    view_mode: ModelSelectorMode,
) {
    let view_tabs = [
        ("Providers", ModelSelectorMode::Providers),
        ("Models", ModelSelectorMode::Models),
        ("Current Config", ModelSelectorMode::CurrentConfig),
    ];

    let mut tab_spans = Vec::new();
    for (i, (label, mode)) in view_tabs.iter().enumerate() {
        let is_active = *mode == view_mode;
        let style = if is_active {
            tokens.text_primary
        } else {
            tokens.text_muted
        };
        tab_spans.push(Span::styled(format!(" {} ", label), style));
        if i < view_tabs.len() - 1 {
            tab_spans.push(Span::styled("│", tokens.separator));
        }
    }

    let lines = vec![
        Line::from(vec![
            Span::styled("Current Provider: ", tokens.text_muted),
            Span::styled(
                current_provider,
                tokens.accent_primary.add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Primary Model: ", tokens.text_muted),
            Span::styled(
                current_model,
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Fast Model: ", tokens.text_muted),
            Span::styled(current_fast_model.unwrap_or("none"), tokens.text_secondary),
        ]),
        Line::raw(""),
        Line::from(tab_spans),
    ];

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Models ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_providers_list(
    f: &mut Frame,
    area: Rect,
    providers: &[ProviderInfo],
    tokens: &ThemeTokens,
    border_style: Style,
    ui_state: &mut ModelSelectorState,
) {
    let items: Vec<ListItem> = providers
        .iter()
        .map(|prov| {
            let status = if prov.is_current {
                " (current)"
            } else if prov.is_available {
                ""
            } else {
                " (unavailable)"
            };
            let availability_style = if prov.is_current {
                tokens.status_ok
            } else if prov.is_available {
                tokens.status_running
            } else {
                tokens.status_failed
            };

            let _avail_text = if prov.is_available {
                "Available"
            } else {
                "Unavailable"
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    if prov.is_current { "▸ " } else { "  " },
                    availability_style,
                ),
                Span::styled(&prov.name, availability_style.add_modifier(Modifier::BOLD)),
                Span::raw(status),
                Span::raw("  "),
                Span::styled(format!("({} models)", prov.model_count), tokens.text_muted),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Model Registry · Providers (↑/↓ navigate, Enter=select) ")
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, area, &mut ui_state.list_state);
}

fn render_models_list(
    f: &mut Frame,
    area: Rect,
    models: &[ModelInfo],
    tokens: &ThemeTokens,
    border_style: Style,
    ui_state: &mut ModelSelectorState,
) {
    let items: Vec<ListItem> = models
        .iter()
        .map(|model| {
            let mut role_tags = Vec::new();
            if model.is_current_primary {
                role_tags.push("PRIMARY");
            }
            if model.is_current_fast {
                role_tags.push("FAST");
            }
            let role_str = if role_tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", role_tags.join(","))
            };

            let avail_style = match model.availability.as_str() {
                "Available" => tokens.status_ok,
                "Unavailable" => tokens.status_failed,
                _ => tokens.status_warning,
            };

            let tier_style = match model.tier.as_str() {
                "Reasoning" => tokens.accent_primary,
                "Standard" => tokens.text_primary,
                "Fast" => tokens.status_ok,
                _ => tokens.text_secondary,
            };

            let tools_text = if model.supports_tools { "✓" } else { "✗" };
            let tools_style = if model.supports_tools {
                tokens.status_ok
            } else {
                tokens.status_failed
            };

            let ctx_str = if model.context_capacity >= 1000 {
                format!("{}k", model.context_capacity / 1000)
            } else {
                model.context_capacity.to_string()
            };

            let display_name = model.display_name.as_deref().unwrap_or(&model.model_id);

            ListItem::new(Line::from(vec![
                Span::styled(
                    if model.is_current_primary || model.is_current_fast {
                        "▸ "
                    } else {
                        "  "
                    },
                    tier_style,
                ),
                Span::styled(&model.model_id, tier_style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(display_name, tokens.text_secondary),
                Span::raw(role_str),
                Span::raw("  "),
                Span::styled(format!("[{}]", model.tier), tier_style),
                Span::raw(" "),
                Span::styled(format!("[{} ctx]", ctx_str), tokens.text_muted),
                Span::raw(" "),
                Span::styled(format!("[Tools: {}]", tools_text), tools_style),
                Span::raw(" "),
                Span::styled(format!("[{}]", model.availability), avail_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Model Registry · Available Models (↑/↓ navigate, 1=Primary, 2=Fast) ")
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, area, &mut ui_state.list_state);
}

fn render_current_config(
    f: &mut Frame,
    area: Rect,
    current_provider: &str,
    current_model: &str,
    current_fast_model: Option<&str>,
    tokens: &ThemeTokens,
    border_style: Style,
) {
    let lines = vec![
        Line::styled(
            "CURRENT MODEL CONFIGURATION",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Provider:      ", tokens.text_muted),
            Span::styled(
                current_provider,
                tokens.accent_primary.add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Primary Model: ", tokens.text_muted),
            Span::styled(
                current_model,
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Fast Model:    ", tokens.text_muted),
            Span::styled(
                current_fast_model.unwrap_or("not set"),
                tokens.text_secondary,
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "Press [Tab] to switch between Providers, Models, and Config views.",
            tokens.text_muted,
        ),
        Line::styled(
            "In Providers view: [Enter] to select provider.",
            tokens.text_muted,
        ),
        Line::styled(
            "In Models view: [1] set as Primary, [2] set as Fast.",
            tokens.text_muted,
        ),
    ];

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Model Registry · Current Configuration ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

/// Handle keyboard input for model selector.
pub fn handle_model_selector_key(
    key: crossterm::event::KeyEvent,
    ui_state: &mut ModelSelectorState,
    providers: &[ProviderInfo],
    models: &[ModelInfo],
) -> Option<ModelSelectorAction> {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Tab => {
            ui_state.mode = match ui_state.mode {
                ModelSelectorMode::Providers => ModelSelectorMode::Models,
                ModelSelectorMode::Models => ModelSelectorMode::CurrentConfig,
                ModelSelectorMode::CurrentConfig => ModelSelectorMode::Providers,
            };
            ui_state.selected_index = 0;
            ui_state.list_state.select(Some(0));
            None
        }
        KeyCode::Char('1') => {
            if ui_state.mode == ModelSelectorMode::Models {
                models
                    .get(ui_state.selected_index)
                    .map(|model| ModelSelectorAction::SetPrimaryModel(model.model_id.clone()))
            } else {
                None
            }
        }
        KeyCode::Char('2') => {
            if ui_state.mode == ModelSelectorMode::Models {
                models
                    .get(ui_state.selected_index)
                    .map(|model| ModelSelectorAction::SetFastModel(model.model_id.clone()))
            } else {
                None
            }
        }
        KeyCode::Enter => {
            if ui_state.mode == ModelSelectorMode::Providers {
                providers
                    .get(ui_state.selected_index)
                    .map(|provider| ModelSelectorAction::SwitchProvider(provider.id.clone()))
            } else {
                None
            }
        }
        KeyCode::Up => {
            if matches!(
                ui_state.mode,
                ModelSelectorMode::Providers | ModelSelectorMode::Models
            ) && ui_state.selected_index > 0
            {
                ui_state.selected_index -= 1;
                ui_state.list_state.select(Some(ui_state.selected_index));
            }
            None
        }
        KeyCode::Down => {
            if matches!(
                ui_state.mode,
                ModelSelectorMode::Providers | ModelSelectorMode::Models
            ) {
                let count = match ui_state.mode {
                    ModelSelectorMode::Providers => providers.len(),
                    ModelSelectorMode::Models => models.len(),
                    _ => 0,
                };
                if ui_state.selected_index + 1 < count {
                    ui_state.selected_index += 1;
                    ui_state.list_state.select(Some(ui_state.selected_index));
                }
            }
            None
        }
        KeyCode::Esc => Some(ModelSelectorAction::Close),
        _ => None,
    }
}

/// Actions that can be triggered from the model selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelectorAction {
    SwitchProvider(String),  // provider_id
    SetPrimaryModel(String), // model_id
    SetFastModel(String),    // model_id
    Close,
}

/// Resolve display models and providers from runtime catalog state, avoiding mock duplication.
pub fn resolve_display_models_and_providers(
    catalog_models: &[crate::model::router::resolver::ModelCandidate],
    active_provider: &str,
    active_model: &str,
) -> (Vec<ProviderInfo>, Vec<ModelInfo>) {
    let mut models: Vec<ModelInfo> = catalog_models
        .iter()
        .map(|c| ModelInfo {
            model_id: c.model_id.clone(),
            display_name: c.display_name.clone(),
            tier: format!("{:?}", c.tier),
            context_capacity: c.context_capacity,
            supports_tools: c.supports_tools,
            is_current_primary: c.model_id == active_model,
            is_current_fast: false,
            availability: format!("{:?}", c.availability),
        })
        .collect();

    if models.is_empty() {
        models = vec![
            ModelInfo {
                model_id: "meta/llama-3.1-70b-instruct".to_string(),
                display_name: Some("Llama 3.1 70B Instruct".to_string()),
                tier: "Standard".to_string(),
                context_capacity: 131072,
                supports_tools: true,
                is_current_primary: active_model == "meta/llama-3.1-70b-instruct",
                is_current_fast: false,
                availability: "Available".to_string(),
            },
            ModelInfo {
                model_id: "meta/llama-3.2-11b-vision-instruct".to_string(),
                display_name: Some("Llama 3.2 11B Vision Instruct".to_string()),
                tier: "Fast".to_string(),
                context_capacity: 131072,
                supports_tools: true,
                is_current_primary: false,
                is_current_fast: active_model == "meta/llama-3.2-11b-vision-instruct",
                availability: "Available".to_string(),
            },
            ModelInfo {
                model_id: "meta/llama-3.3-70b-instruct".to_string(),
                display_name: Some("Llama 3.3 70B Instruct".to_string()),
                tier: "Reasoning".to_string(),
                context_capacity: 131072,
                supports_tools: true,
                is_current_primary: active_model == "meta/llama-3.3-70b-instruct",
                is_current_fast: false,
                availability: "Available".to_string(),
            },
            ModelInfo {
                model_id: crate::model::catalog::CANONICAL_REAL_MODEL_ID.to_string(),
                display_name: Some("Nemotron 3 Ultra 550B".to_string()),
                tier: "Reasoning".to_string(),
                context_capacity: 131072,
                supports_tools: true,
                is_current_primary: active_model == crate::model::catalog::CANONICAL_REAL_MODEL_ID,
                is_current_fast: false,
                availability: "Available".to_string(),
            },
        ];
    }

    let is_nvidia = active_provider == "nvidia_nim" || active_provider == "nvidia";
    let is_mock = active_provider == "mock";
    let providers = vec![
        ProviderInfo {
            id: "nvidia_nim".to_string(),
            name: "NVIDIA NIM".to_string(),
            is_current: is_nvidia || (!is_mock && active_provider != "none"),
            is_available: true,
            model_count: models.len(),
            base_url: Some(crate::model::catalog::CANONICAL_REAL_MODEL_BASE_URL.to_string()),
        },
        ProviderInfo {
            id: "mock".to_string(),
            name: "Mock Provider (Test/Offline)".to_string(),
            is_current: is_mock,
            is_available: true,
            model_count: 2,
            base_url: None,
        },
    ];

    (providers, models)
}
