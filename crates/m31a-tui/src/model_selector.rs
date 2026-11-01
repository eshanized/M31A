//! Shared model selector component with proper scrolling and viewport management.

use m31a_provider::models::{ModelInfo, ServiceTier, ToolMode, TruncationPolicy};
use ratatui::{
    layout::{Margin, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

/// Authoritative geometry contract for model selector widgets across BottomPane, Modal, and Screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSelectorGeometry {
    pub outer: Rect,
    pub inner: Rect,
}

impl ModelSelectorGeometry {
    pub fn new(outer: Rect) -> Self {
        let inner = Block::default().borders(Borders::ALL).inner(outer);
        Self { outer, inner }
    }

    pub fn inner_height(&self) -> usize {
        self.inner.height as usize
    }

    pub fn inner_width(&self) -> usize {
        self.inner.width as usize
    }

    /// Maps a screen click coordinate (col, row) directly to a visible row index (0..inner_height),
    /// strictly enforcing that clicks outside the selectable inner list rows return None.
    pub fn row_to_visible_index(&self, col: u16, row: u16) -> Option<usize> {
        let pos = ratatui::layout::Position::new(col, row);
        if !self.inner.contains(pos) {
            return None;
        }
        let rel_row = (row.saturating_sub(self.inner.y)) as usize;
        if rel_row < self.inner_height() {
            Some(rel_row)
        } else {
            None
        }
    }
}

/// State for a scrollable model list with filtering support.
#[derive(Debug, Clone)]
pub struct ModelListState {
    pub models: Vec<ModelInfo>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub search_query: String,
    pub active_model: String,
}

impl ModelListState {
    pub fn new(models: Vec<ModelInfo>, active_model: String) -> Self {
        let filtered_indices: Vec<usize> = (0..models.len()).collect();
        Self {
            models,
            filtered_indices,
            selected_index: 0,
            scroll_offset: 0,
            viewport_height: 0,
            search_query: String::new(),
            active_model,
        }
    }

    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
        self.clamp_scroll();
        self.ensure_selected_visible();
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query.to_lowercase();
        self.filtered_indices = self
            .models
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                if self.search_query.is_empty()
                    || m.slug.to_lowercase().contains(&self.search_query)
                    || m.display_name.to_lowercase().contains(&self.search_query)
                    || m.description.to_lowercase().contains(&self.search_query)
                {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.clamp_scroll();
    }

    pub fn set_active_model(&mut self, model: String) {
        self.active_model = model;
        // Find the new active model in filtered list
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .position(|&i| self.models[i].slug == self.active_model)
        {
            self.selected_index = pos;
            self.ensure_selected_visible();
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
            self.ensure_selected_visible();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_indices.len() - 1
            } else {
                self.selected_index - 1
            };
            self.ensure_selected_visible();
        }
    }

    pub fn select_page_down(&mut self) {
        if !self.filtered_indices.is_empty() {
            let step = if self.viewport_height > 0 {
                self.viewport_height
            } else {
                1
            };
            self.selected_index = (self.selected_index + step).min(self.filtered_indices.len() - 1);
            self.ensure_selected_visible();
        }
    }

    pub fn select_page_up(&mut self) {
        if !self.filtered_indices.is_empty() {
            let step = if self.viewport_height > 0 {
                self.viewport_height
            } else {
                1
            };
            self.selected_index = self.selected_index.saturating_sub(step);
            self.ensure_selected_visible();
        }
    }

    pub fn select_home(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn select_end(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = self.filtered_indices.len() - 1;
            self.ensure_selected_visible();
        }
    }

    pub fn ensure_selected_visible(&mut self) {
        if self.filtered_indices.is_empty() {
            self.scroll_offset = 0;
            self.selected_index = 0;
            return;
        }
        if self.viewport_height == 0 {
            self.scroll_offset = 0;
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.selected_index + 1 - self.viewport_height;
        }
        self.clamp_scroll();
    }

    pub fn clamp_scroll(&mut self) {
        if self.filtered_indices.is_empty() {
            self.scroll_offset = 0;
            self.selected_index = 0;
            return;
        }
        self.selected_index = self.selected_index.min(self.filtered_indices.len() - 1);
        if self.viewport_height == 0 {
            self.scroll_offset = 0;
            return;
        }
        let max_scroll = self
            .filtered_indices
            .len()
            .saturating_sub(self.viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }

    pub fn get_selected_model(&self) -> Option<&ModelInfo> {
        self.filtered_indices
            .get(self.selected_index)
            .map(|&i| &self.models[i])
    }

    pub fn get_visible_models(&self) -> Vec<&ModelInfo> {
        if self.filtered_indices.is_empty() || self.viewport_height == 0 {
            return Vec::new();
        }
        let start = self.scroll_offset.min(self.filtered_indices.len());
        let end = (start + self.viewport_height).min(self.filtered_indices.len());
        if start >= end {
            return Vec::new();
        }
        self.filtered_indices[start..end]
            .iter()
            .map(|&i| &self.models[i])
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.filtered_indices.is_empty()
    }

    pub fn total_filtered(&self) -> usize {
        self.filtered_indices.len()
    }
}

/// Renders a model list with proper scrolling, Ratatui scrollbar, and 1-row compact selection highlighting.
pub fn render_model_list(
    f: &mut ratatui::Frame,
    state: &ModelListState,
    area: Rect,
    theme: &crate::theme::Theme,
    title: &str,
) {
    let geometry = ModelSelectorGeometry::new(area);
    if state.is_empty() {
        let msg = if state.search_query.is_empty() {
            "No models available"
        } else {
            "No models match search"
        };
        let p = Paragraph::new(msg)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(theme.border_subtle()),
            )
            .style(theme.style_muted());
        f.render_widget(p, area);
        return;
    }

    let inner_h = geometry.inner_height();
    let start = state.scroll_offset.min(state.filtered_indices.len());
    let end = (start + inner_h).min(state.filtered_indices.len());
    let visible_indices = if start < end {
        &state.filtered_indices[start..end]
    } else {
        &[]
    };

    let items: Vec<ListItem> = visible_indices
        .iter()
        .enumerate()
        .map(|(rel_idx, &model_idx)| {
            let m = &state.models[model_idx];
            let actual_idx = state.scroll_offset + rel_idx;
            let is_selected = actual_idx == state.selected_index;
            let is_active = m.slug == state.active_model;

            let name_style = if is_selected {
                theme.style_selected().add_modifier(Modifier::BOLD)
            } else if is_active {
                theme
                    .style_primary()
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC)
            } else {
                theme.style_primary()
            };

            let meta_style = if is_selected {
                theme.style_selected()
            } else {
                theme.style_muted()
            };

            let sel_indicator = if is_selected { "▸ " } else { "  " };
            let active_indicator = if is_active { "● " } else { "  " };

            let mut spans = vec![
                Span::styled(
                    sel_indicator,
                    if is_selected {
                        theme.style_selected()
                    } else {
                        theme.style_muted()
                    },
                ),
                Span::styled(
                    active_indicator,
                    if is_active {
                        theme.style_success(false)
                    } else {
                        theme.style_muted()
                    },
                ),
                Span::styled(&m.display_name, name_style),
                Span::raw(" "),
                Span::styled(format!("({})", m.slug), meta_style),
                Span::raw(" "),
                Span::styled(format!("[{}k]", m.context_window / 1024), meta_style),
            ];

            if m.supports_thinking {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "[R]",
                    if is_selected {
                        theme.style_selected()
                    } else {
                        theme.style_warning(false)
                    },
                ));
            }
            if m.supports_tools {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "[T]",
                    if is_selected {
                        theme.style_selected()
                    } else {
                        theme.style_success(false)
                    },
                ));
            }
            if m.supports_vision {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "[V]",
                    if is_selected {
                        theme.style_selected()
                    } else {
                        theme.style_info(false)
                    },
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut list_state = ListState::default();
    if state.selected_index >= state.scroll_offset && state.selected_index < end {
        list_state.select(Some(state.selected_index - state.scroll_offset));
    }

    let scroll_indicator = if state.total_filtered() > inner_h {
        format!(
            " [{}/{}] ",
            state.selected_index + 1,
            state.total_filtered()
        )
    } else {
        String::new()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_focus())
                .title(Line::from(vec![
                    Span::styled(
                        format!(" {} ", title),
                        theme.style_secondary().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            "↑↓ PgUp/PgDn Home/End Enter=Select Esc=Close{}",
                            scroll_indicator
                        ),
                        theme.style_muted(),
                    ),
                ]))
                .style(theme.style_surface1()),
        )
        .highlight_style(theme.style_selected());

    f.render_stateful_widget(list, area, &mut list_state);

    if state.total_filtered() > inner_h && inner_h > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(state.total_filtered().saturating_sub(inner_h))
                .position(state.scroll_offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(theme.style_muted());
        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

/// Renders a detail inspector pane for a selected model.
pub fn render_model_details(
    f: &mut ratatui::Frame,
    model: Option<&ModelInfo>,
    is_active: bool,
    area: Rect,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_subtle())
        .title(Line::from(vec![
            Span::styled(
                " Model Specification ",
                theme.style_secondary().add_modifier(Modifier::BOLD),
            ),
            if is_active {
                Span::styled(" [ACTIVE] ", theme.style_success(true))
            } else {
                Span::raw("")
            },
        ]))
        .style(theme.style_surface1());

    let Some(m) = model else {
        let msg = Paragraph::new("No model selected. Use ↑/↓ to navigate the catalog.")
            .block(block)
            .style(theme.style_muted());
        f.render_widget(msg, area);
        return;
    };

    let tool_mode_str = match m.tool_mode {
        ToolMode::Direct => "Direct function calling",
        ToolMode::CodeMode => "Code execution mode",
        ToolMode::CodeModeOnly => "Code execution only",
        ToolMode::Unsupported => "Not supported",
    };

    let tier_str = match m.service_tier {
        ServiceTier::Standard => "Standard",
        ServiceTier::Premium => "Premium (High reasoning/latency)",
        ServiceTier::Turbo => "Turbo (Low latency)",
        ServiceTier::Preview => "Preview",
    };

    let truncation_str = match m.truncation_policy {
        TruncationPolicy::Truncate => "Truncate at token limit",
        TruncationPolicy::Error => "Error on overflow",
        TruncationPolicy::AllowOverflow => "Allow overflow",
    };

    let context_window_str = if m.context_window > 0 {
        format!(
            "{} tokens (~{}k)",
            m.context_window,
            m.context_window / 1024
        )
    } else {
        "Unknown context limit".to_string()
    };

    let max_output_str = if m.max_output_tokens > 0 {
        format!("{} tokens", m.max_output_tokens)
    } else {
        "Unknown output limit".to_string()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Name: ",
                theme.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &m.display_name,
                theme.style_primary().add_modifier(Modifier::BOLD),
            ),
            if is_active {
                Span::styled(" (ACTIVE)", theme.style_success(true))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled("Slug: ", theme.style_secondary()),
            Span::styled(&m.slug, theme.style_muted()),
        ]),
        Line::from(vec![
            Span::styled("Originator: ", theme.style_secondary()),
            Span::styled(
                if m.originator.is_empty() {
                    "Unknown"
                } else {
                    &m.originator
                },
                theme.style_primary(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Context Window: ", theme.style_secondary()),
            Span::styled(context_window_str, theme.style_primary()),
        ]),
        Line::from(vec![
            Span::styled("Max Output Tokens: ", theme.style_secondary()),
            Span::styled(max_output_str, theme.style_primary()),
        ]),
        Line::from(vec![
            Span::styled("Default Temperature: ", theme.style_secondary()),
            Span::styled(
                format!("{:.2}", m.default_temperature),
                theme.style_primary(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Tool Calling: ", theme.style_secondary()),
            Span::styled(
                tool_mode_str,
                if m.supports_tools {
                    theme.style_success(false)
                } else {
                    theme.style_muted()
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Reasoning / Thinking: ", theme.style_secondary()),
            Span::styled(
                if m.supports_thinking {
                    "Supported"
                } else {
                    "None"
                },
                if m.supports_thinking {
                    theme.style_warning(false)
                } else {
                    theme.style_muted()
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Vision: ", theme.style_secondary()),
            Span::styled(
                if m.supports_vision {
                    "Supported"
                } else {
                    "None"
                },
                if m.supports_vision {
                    theme.style_info(false)
                } else {
                    theme.style_muted()
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Service Tier: ", theme.style_secondary()),
            Span::styled(tier_str, theme.style_primary()),
        ]),
        Line::from(vec![
            Span::styled("Truncation Policy: ", theme.style_secondary()),
            Span::styled(truncation_str, theme.style_muted()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Live Health & Connectivity:",
            theme.style_secondary().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("Catalog Status: ", theme.style_secondary()),
            Span::styled("Registered in catalog", theme.style_success(false)),
        ]),
        Line::from(vec![
            Span::styled("Provider Health: ", theme.style_secondary()),
            Span::styled(
                "Not Probed (Press [t] to probe live connectivity)",
                theme.style_muted(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Live Latency: ", theme.style_secondary()),
            Span::styled("Unavailable (live probe required)", theme.style_muted()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Description:",
            theme.style_secondary().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            if m.description.is_empty() {
                "No description available."
            } else {
                &m.description
            },
            theme.style_muted(),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Actions: ",
                theme.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[Enter] Activate  |  [/] Filter  |  [t] Health  |  [Esc] Chat",
                theme.style_muted(),
            ),
        ]),
    ];

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_provider::models::ModelInfo;

    fn create_test_models() -> Vec<ModelInfo> {
        (0..50)
            .map(|i| ModelInfo {
                slug: format!("model-{}", i),
                display_name: format!("Test Model {}", i),
                description: format!("Description for model {}", i),
                context_window: 32768,
                max_output_tokens: 4096,
                supports_tools: true,
                supports_thinking: i % 2 == 0,
                supports_vision: i % 3 == 0,
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn test_model_list_navigation() {
        let models = create_test_models();
        let mut state = ModelListState::new(models, "model-0".to_string());
        state.set_viewport_height(10);

        // Test initial state
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);

        // Test page down from 0
        state.select_page_down();
        assert_eq!(state.selected_index, 10);
        assert_eq!(state.scroll_offset, 1); // Should scroll

        // Test down navigation
        state.select_next();
        assert_eq!(state.selected_index, 11);
        assert_eq!(state.scroll_offset, 2);

        // Test page up
        state.select_page_up();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.scroll_offset, 1);

        // Test home/end
        state.select_end();
        assert_eq!(state.selected_index, 49);
        assert!(state.scroll_offset > 0);

        state.select_home();
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_model_list_filtering() {
        let models = create_test_models();
        let mut state = ModelListState::new(models, "model-0".to_string());
        state.set_viewport_height(10);

        // Filter for model-5
        state.set_search_query("model-5".to_string());
        assert_eq!(state.total_filtered(), 1); // Only model-5 matches "model-5"
        assert_eq!(state.get_selected_model().unwrap().slug, "model-5");

        // Clear filter
        state.set_search_query("".to_string());
        assert_eq!(state.total_filtered(), 50);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_model_list_active_model_tracking() {
        let models = create_test_models();
        let mut state = ModelListState::new(models, "model-0".to_string());
        state.set_viewport_height(10);

        // Change active model
        state.set_active_model("model-25".to_string());
        assert_eq!(state.active_model, "model-25");
        assert_eq!(state.selected_index, 25);
        assert!(state.scroll_offset > 0); // Should scroll to show active model
    }
}
