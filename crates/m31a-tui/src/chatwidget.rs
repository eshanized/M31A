//! Interactive chat message list widget displaying turns, thought traces, tool runs, and streaming output.
//!
//! Renders user messages, model reasoning blocks, animated spinners, and expandable tool execution boxes.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::time::{Duration, Instant};

/// Spinner animation frames for interactive model reasoning and tool actions.
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Lifecycle state machine for an autonomous conversation turn in M31A.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnExecutionState {
    /// Agent is idle and waiting for user input.
    Idle,
    /// Model is streaming reasoning/thinking tokens before formulating its response.
    Thinking {
        started_at: Instant,
        tokens: usize,
        preview: String,
    },
    /// Model is streaming visible response content chunks.
    StreamingContent { started_at: Instant, tokens: usize },
    /// Agent is executing an external tool or command in the sandbox.
    ToolExecuting {
        tool_id: String,
        tool_name: String,
        started_at: Instant,
    },
    /// Turn is paused awaiting human-in-the-loop permission or input.
    AwaitingApproval {
        request_id: String,
        action_summary: String,
    },
    /// Agent is running context compaction and history summarization.
    CompactingHistory { started_at: Instant },
    /// Turn finished execution successfully.
    TurnCompleted {
        duration_ms: u64,
        tokens_used: usize,
    },
    /// Turn halted due to an unrecoverable error.
    Failed { error: String },
}

impl TurnExecutionState {
    /// Returns true if the turn is actively processing an asynchronous step.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            TurnExecutionState::Thinking { .. }
                | TurnExecutionState::StreamingContent { .. }
                | TurnExecutionState::ToolExecuting { .. }
                | TurnExecutionState::CompactingHistory { .. }
        )
    }

    /// Returns human-readable label for status lines.
    pub fn label(&self) -> &'static str {
        match self {
            TurnExecutionState::Idle => "Ready",
            TurnExecutionState::Thinking { .. } => "Thinking",
            TurnExecutionState::StreamingContent { .. } => "Generating",
            TurnExecutionState::ToolExecuting { .. } => "Executing Tool",
            TurnExecutionState::AwaitingApproval { .. } => "Awaiting Approval",
            TurnExecutionState::CompactingHistory { .. } => "Compacting Context",
            TurnExecutionState::TurnCompleted { .. } => "Completed",
            TurnExecutionState::Failed { .. } => "Failed",
        }
    }
}

/// Performance metrics for token streaming (TTFT, velocity, token count).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamMetricsSnapshot {
    pub ttft_ms: Option<u64>,
    pub tokens_per_second: f64,
    pub total_tokens: usize,
    pub duration_ms: u64,
}

/// Accumulating buffer for streaming token deltas with velocity tracking.
#[derive(Debug, Clone)]
pub struct StreamingTokenBuffer {
    buffer: String,
    token_count: usize,
    turn_started_at: Option<Instant>,
    first_token_at: Option<Instant>,
    started_at: Option<Instant>,
    last_chunk_at: Option<Instant>,
    last_turn_metrics: Option<StreamMetricsSnapshot>,
}

impl Default for StreamingTokenBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingTokenBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            token_count: 0,
            turn_started_at: None,
            first_token_at: None,
            started_at: None,
            last_chunk_at: None,
            last_turn_metrics: None,
        }
    }

    /// Marks the start of a turn before the model begins streaming tokens.
    pub fn mark_turn_start(&mut self) {
        self.buffer.clear();
        self.token_count = 0;
        self.started_at = None;
        self.last_chunk_at = None;
        self.first_token_at = None;
        self.turn_started_at = Some(Instant::now());
    }

    /// Appends a new incoming token delta chunk to the buffer.
    pub fn append_chunk(&mut self, chunk: &str) {
        let now = Instant::now();
        if self.first_token_at.is_none() {
            self.first_token_at = Some(now);
        }
        if self.started_at.is_none() {
            self.started_at = Some(now);
        }
        self.last_chunk_at = Some(now);
        self.buffer.push_str(chunk);

        // Approximate token count: 1 token ~= 4 chars or whitespace boundary
        let estimated_tokens = (chunk.split_whitespace().count()).max(1);
        self.token_count += estimated_tokens;
    }

    /// Clears the token buffer for the next turn.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.token_count = 0;
        self.started_at = None;
        self.last_chunk_at = None;
        self.first_token_at = None;
        self.turn_started_at = None;
    }

    /// Returns the accumulated content.
    pub fn content(&self) -> &str {
        &self.buffer
    }

    /// Returns whether the buffer contains any text.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns total tokens recorded in this streaming burst.
    pub fn token_count(&self) -> usize {
        self.token_count
    }

    /// Returns elapsed streaming duration.
    pub fn elapsed(&self) -> Duration {
        self.started_at
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Returns Time to First Token (TTFT) in milliseconds, if recorded.
    pub fn ttft_ms(&self) -> Option<u64> {
        match (self.turn_started_at, self.first_token_at) {
            (Some(start), Some(first)) => Some(first.duration_since(start).as_millis() as u64),
            _ => None,
        }
    }

    /// Returns streaming velocity in tokens per second.
    pub fn tokens_per_second(&self) -> f64 {
        let secs = self.elapsed().as_secs_f64();
        if secs > 0.05 && self.token_count > 0 {
            self.token_count as f64 / secs
        } else {
            0.0
        }
    }

    /// Captures current streaming metrics snapshot.
    pub fn snapshot(&self) -> StreamMetricsSnapshot {
        StreamMetricsSnapshot {
            ttft_ms: self.ttft_ms(),
            tokens_per_second: self.tokens_per_second(),
            total_tokens: self.token_count,
            duration_ms: self.elapsed().as_millis() as u64,
        }
    }

    /// Marks turn completed and retains snapshot for status display.
    pub fn mark_turn_completed(&mut self) {
        if self.token_count > 0 {
            self.last_turn_metrics = Some(self.snapshot());
        }
    }

    /// Returns the active or most recent turn's stream metrics.
    pub fn last_or_active_metrics(&self) -> Option<StreamMetricsSnapshot> {
        if self.token_count > 0 {
            Some(self.snapshot())
        } else {
            self.last_turn_metrics
        }
    }
}

/// Viewport scroll manager with bottom pinning behavior and acceleration.
#[derive(Debug, Clone)]
pub struct AutoScrollController {
    pub is_pinned_to_bottom: bool,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub total_lines: usize,
    pub last_scroll_at: Option<Instant>,
    pub consecutive_scrolls: usize,
}

impl AutoScrollController {
    pub fn new(viewport_height: usize) -> Self {
        Self {
            is_pinned_to_bottom: true,
            scroll_offset: 0,
            viewport_height,
            total_lines: 0,
            last_scroll_at: None,
            consecutive_scrolls: 0,
        }
    }

    /// Computes accelerated scroll delta based on turn frequency.
    pub fn accelerate_delta(&mut self, base_delta: usize, use_accel: bool) -> usize {
        let now = Instant::now();
        if use_accel {
            if let Some(last) = self.last_scroll_at {
                if now.duration_since(last) < Duration::from_millis(150) {
                    self.consecutive_scrolls = (self.consecutive_scrolls + 1).min(5);
                } else {
                    self.consecutive_scrolls = 0;
                }
            } else {
                self.consecutive_scrolls = 0;
            }
            self.last_scroll_at = Some(now);
            let multiplier = match self.consecutive_scrolls {
                0 => 1,
                1 => 1,
                2 => 2,
                3 => 3,
                4 => 4,
                _ => 5,
            };
            base_delta * multiplier
        } else {
            self.last_scroll_at = Some(now);
            self.consecutive_scrolls = 0;
            base_delta
        }
    }

    /// Updates viewport dimensions and clamps scroll offset.
    pub fn update_dimensions(&mut self, total_lines: usize, viewport_height: usize) {
        self.total_lines = total_lines;
        self.viewport_height = viewport_height;

        if self.is_pinned_to_bottom {
            self.scroll_to_bottom();
        } else {
            self.clamp_offset();
        }
    }

    /// Called when new content lines are added to the transcript.
    pub fn on_new_content(&mut self, new_total_lines: usize) {
        self.total_lines = new_total_lines;
        if self.is_pinned_to_bottom {
            self.scroll_to_bottom();
        }
    }

    /// Scrolls up by delta lines, breaking bottom pinning.
    pub fn scroll_up(&mut self, delta: usize) {
        if self.scroll_offset > delta {
            self.scroll_offset -= delta;
        } else {
            self.scroll_offset = 0;
        }
        self.is_pinned_to_bottom = self.is_at_bottom();
    }

    /// Scrolls down by delta lines, re-engaging pin if bottom is reached.
    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(delta);
        self.clamp_offset();
        self.is_pinned_to_bottom = self.is_at_bottom();
    }

    /// Jump directly to top of transcript.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.is_pinned_to_bottom = self.is_at_bottom();
    }

    /// Jump directly to bottom of transcript and engage pinning.
    pub fn scroll_to_bottom(&mut self) {
        if self.total_lines > self.viewport_height {
            self.scroll_offset = self.total_lines.saturating_sub(self.viewport_height);
        } else {
            self.scroll_offset = 0;
        }
        self.is_pinned_to_bottom = true;
    }

    /// Returns true if the viewport is currently scrolled to the absolute bottom.
    pub fn is_at_bottom(&self) -> bool {
        if self.total_lines <= self.viewport_height {
            true
        } else {
            self.scroll_offset >= self.total_lines.saturating_sub(self.viewport_height)
        }
    }

    /// Clamps scroll offset so we never scroll past the end of content.
    pub fn clamp_offset(&mut self) {
        if self.total_lines > self.viewport_height {
            let max_offset = self.total_lines.saturating_sub(self.viewport_height);
            if self.scroll_offset > max_offset {
                self.scroll_offset = max_offset;
            }
        } else {
            self.scroll_offset = 0;
        }
    }

    /// Returns the active scroll offset for rendering.
    pub fn offset(&self) -> usize {
        self.scroll_offset
    }

    /// Returns the number of hidden lines below the current viewport window.
    pub fn lines_below(&self) -> usize {
        let visible_end = self.scroll_offset + self.viewport_height;
        self.total_lines.saturating_sub(visible_end)
    }
}

/// Chat widget state combining turn execution, streaming buffer, and scrolling.
#[derive(Debug, Clone)]
pub struct ChatWidgetState {
    pub turn_state: TurnExecutionState,
    pub streaming_buffer: StreamingTokenBuffer,
    pub scroll_controller: AutoScrollController,
    pub spinner_tick: usize,
}

impl Default for ChatWidgetState {
    fn default() -> Self {
        Self::new(24)
    }
}

impl ChatWidgetState {
    pub fn new(viewport_height: usize) -> Self {
        Self {
            turn_state: TurnExecutionState::Idle,
            streaming_buffer: StreamingTokenBuffer::new(),
            scroll_controller: AutoScrollController::new(viewport_height),
            spinner_tick: 0,
        }
    }

    /// Advances the spinner animation frame.
    pub fn tick_spinner(&mut self) {
        self.spinner_tick = (self.spinner_tick + 1) % SPINNER_FRAMES.len();
    }

    /// Returns current spinner character.
    pub fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_tick % SPINNER_FRAMES.len()]
    }

    /// Transitions to thinking state.
    pub fn start_thinking(&mut self) {
        self.turn_state = TurnExecutionState::Thinking {
            started_at: Instant::now(),
            tokens: 0,
            preview: String::new(),
        };
        self.streaming_buffer.mark_turn_start();
    }

    /// Transitions to streaming content state.
    pub fn start_streaming(&mut self) {
        self.turn_state = TurnExecutionState::StreamingContent {
            started_at: Instant::now(),
            tokens: 0,
        };
        self.streaming_buffer.mark_turn_start();
    }

    /// Transitions to executing tool state.
    pub fn start_tool(&mut self, tool_id: impl Into<String>, tool_name: impl Into<String>) {
        self.turn_state = TurnExecutionState::ToolExecuting {
            tool_id: tool_id.into(),
            tool_name: tool_name.into(),
            started_at: Instant::now(),
        };
    }

    /// Transitions to awaiting approval state.
    pub fn await_approval(
        &mut self,
        request_id: impl Into<String>,
        action_summary: impl Into<String>,
    ) {
        self.turn_state = TurnExecutionState::AwaitingApproval {
            request_id: request_id.into(),
            action_summary: action_summary.into(),
        };
    }

    /// Transitions to completed state.
    pub fn complete_turn(&mut self, duration_ms: u64, tokens_used: usize) {
        self.streaming_buffer.mark_turn_completed();
        self.turn_state = TurnExecutionState::TurnCompleted {
            duration_ms,
            tokens_used,
        };
    }

    /// Transitions to failed state.
    pub fn fail_turn(&mut self, error: impl Into<String>) {
        self.turn_state = TurnExecutionState::Failed {
            error: error.into(),
        };
    }

    /// Formats the active status badge text and color.
    pub fn format_status_badge(&self) -> (String, Color) {
        let spinner = self.spinner_char();
        match &self.turn_state {
            TurnExecutionState::Idle => ("Ready".to_string(), Color::DarkGray),
            TurnExecutionState::Thinking {
                started_at, tokens, ..
            } => {
                let elapsed = started_at.elapsed().as_secs_f32();
                (
                    format!("{spinner} Thinking ({elapsed:.1}s, {tokens} tokens)"),
                    Color::Cyan,
                )
            }
            TurnExecutionState::StreamingContent { started_at, tokens } => {
                let elapsed = started_at.elapsed().as_secs_f32();
                let v = self.streaming_buffer.tokens_per_second();
                (
                    format!("{spinner} Generating ({elapsed:.1}s, {tokens} tok, {v:.0} t/s)"),
                    Color::Green,
                )
            }
            TurnExecutionState::ToolExecuting {
                tool_name,
                started_at,
                ..
            } => {
                let elapsed = started_at.elapsed().as_secs_f32();
                (
                    format!("Executing {tool_name} ({elapsed:.1}s)"),
                    Color::Yellow,
                )
            }
            TurnExecutionState::AwaitingApproval { .. } => {
                ("Approval required".to_string(), Color::Magenta)
            }
            TurnExecutionState::CompactingHistory { started_at } => {
                let elapsed = started_at.elapsed().as_secs_f32();
                (format!("Compacting context ({elapsed:.1}s)"), Color::Blue)
            }
            TurnExecutionState::TurnCompleted {
                duration_ms,
                tokens_used,
            } => {
                let secs = *duration_ms as f64 / 1000.0;
                (
                    format!("Turn complete ({secs:.2}s, {tokens_used} tokens)"),
                    Color::Green,
                )
            }
            TurnExecutionState::Failed { error } => (format!("Error: {error}"), Color::Red),
        }
    }

    /// Renders a styled Ratatui Line for the status indicator bar.
    pub fn render_status_line(&self) -> Line<'static> {
        let (badge, color) = self.format_status_badge();
        let scroll_hint = if self.scroll_controller.is_pinned_to_bottom {
            Span::styled(" [Pinned to Bottom]", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(
                format!(" [Scrolled +{}]", self.scroll_controller.offset()),
                Style::default().fg(Color::Yellow),
            )
        };

        Line::from(vec![
            Span::styled(
                badge,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            scroll_hint,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_state_lifecycle() {
        let mut state = ChatWidgetState::new(20);
        assert_eq!(state.turn_state, TurnExecutionState::Idle);
        assert!(!state.turn_state.is_active());

        state.start_thinking();
        assert!(state.turn_state.is_active());
        assert_eq!(state.turn_state.label(), "Thinking");

        state.start_streaming();
        assert!(state.turn_state.is_active());
        assert_eq!(state.turn_state.label(), "Generating");

        state.start_tool("t-1", "read_file");
        assert!(state.turn_state.is_active());
        assert_eq!(state.turn_state.label(), "Executing Tool");

        state.await_approval("req-123", "Write to /etc/hosts");
        assert!(!state.turn_state.is_active());
        assert_eq!(state.turn_state.label(), "Awaiting Approval");

        state.complete_turn(1500, 350);
        assert_eq!(
            state.turn_state,
            TurnExecutionState::TurnCompleted {
                duration_ms: 1500,
                tokens_used: 350
            }
        );

        state.fail_turn("Network timeout");
        assert_eq!(
            state.turn_state,
            TurnExecutionState::Failed {
                error: "Network timeout".to_string()
            }
        );
    }

    #[test]
    fn test_streaming_token_buffer_append_and_clear() {
        let mut buf = StreamingTokenBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.token_count(), 0);

        buf.append_chunk("Hello world, this is a streaming test.");
        assert!(!buf.is_empty());
        assert!(buf.token_count() > 0);
        assert_eq!(buf.content(), "Hello world, this is a streaming test.");

        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.token_count(), 0);
        assert_eq!(buf.content(), "");
    }

    #[test]
    fn test_auto_scroll_pinning_and_unpinning() {
        let mut scroller = AutoScrollController::new(10);
        assert!(scroller.is_pinned_to_bottom);
        assert_eq!(scroller.offset(), 0);

        // Content smaller than viewport
        scroller.on_new_content(5);
        assert_eq!(scroller.offset(), 0);
        assert!(scroller.is_pinned_to_bottom);

        // Content exceeds viewport
        scroller.on_new_content(25);
        // Total lines = 25, viewport = 10, max offset = 15
        assert_eq!(scroller.offset(), 15);
        assert!(scroller.is_pinned_to_bottom);

        // User scrolls up 5 lines
        scroller.scroll_up(5);
        assert_eq!(scroller.offset(), 10);
        assert!(!scroller.is_pinned_to_bottom);
        assert_eq!(scroller.lines_below(), 5);

        // Adding content while unpinned does not force jump to bottom
        scroller.total_lines = 30;
        assert_eq!(scroller.offset(), 10);
        assert!(!scroller.is_pinned_to_bottom);
        assert_eq!(scroller.lines_below(), 10);

        // User scrolls down to bottom
        scroller.scroll_down(20);
        assert_eq!(scroller.offset(), 20); // 30 - 10
        assert!(scroller.is_pinned_to_bottom);
        assert_eq!(scroller.lines_below(), 0);

        // Jump to top
        scroller.scroll_to_top();
        assert_eq!(scroller.offset(), 0);
        assert!(!scroller.is_pinned_to_bottom);

        // Jump to bottom
        scroller.scroll_to_bottom();
        assert_eq!(scroller.offset(), 20);
        assert!(scroller.is_pinned_to_bottom);
    }

    #[test]
    fn test_status_badge_formatting() {
        let mut state = ChatWidgetState::new(20);
        let (badge, color) = state.format_status_badge();
        assert_eq!(badge, "Ready");
        assert_eq!(color, Color::DarkGray);

        state.fail_turn("Disk full");
        let (badge, color) = state.format_status_badge();
        assert!(badge.contains("Disk full"));
        assert_eq!(color, Color::Red);

        state.complete_turn(2400, 1024);
        let (badge, color) = state.format_status_badge();
        assert!(badge.contains("2.40s"));
        assert!(badge.contains("1024 tokens"));
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_spinner_animation_cycling() {
        let mut state = ChatWidgetState::new(20);
        let initial_frame = state.spinner_char();
        assert_eq!(initial_frame, SPINNER_FRAMES[0]);

        for expected in SPINNER_FRAMES.iter().skip(1) {
            state.tick_spinner();
            assert_eq!(&state.spinner_char(), expected);
        }

        // Wraps around
        state.tick_spinner();
        assert_eq!(state.spinner_char(), SPINNER_FRAMES[0]);
    }
}
