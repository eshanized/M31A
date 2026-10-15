//! Post-Mortem & Checkpoint Replay Subsystem (TUI-01, D-19).
//!
//! Provides read-only historical execution scrub through event and checkpoint
//! timelines:
//! - `[`: Step backward 1 event
//! - `]`: Step forward 1 event
//! - `Space`: Play / Pause timeline playback
//! - `1` / `2` / `4`: Select scrub playback speed (1x, 2x, 4x)
//! - `Esc`: Exit Replay mode
//!
//! Strictly enforces D-19 and T-11-20: Replay mode is 100% read-only, displays
//! a high-visibility amber banner `[REPLAY MODE - READ ONLY]`, and completely
//! blocks/strips all mutating operator actions to ensure zero side effects.

use crossterm::event::{KeyCode, KeyEvent};
use serde::{Deserialize, Serialize};

use super::model::TuiViewModel;
use crate::events::envelope::EventEnvelope;

/// Playback speed multiplier during timeline replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplaySpeed {
    Normal, // 1x
    Fast,   // 2x
    Hyper,  // 4x
}

impl ReplaySpeed {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "1x",
            Self::Fast => "2x",
            Self::Hyper => "4x",
        }
    }

    /// Stable elapsed interval required to advance one event.
    /// Canonical durations: Normal (1x) = 1000ms, Fast (2x) = 500ms, Hyper (4x) = 250ms.
    pub fn step_interval(&self) -> std::time::Duration {
        match self {
            Self::Normal => std::time::Duration::from_millis(1000),
            Self::Fast => std::time::Duration::from_millis(500),
            Self::Hyper => std::time::Duration::from_millis(250),
        }
    }
}

/// Controller for navigating historical mission checkpoints and event timelines.
pub struct ReplayController {
    pub is_active: bool,
    pub history: Vec<EventEnvelope>,
    pub cursor: usize,
    pub speed: ReplaySpeed,
    pub is_playing: bool,
    pub last_tick_at: Option<std::time::Instant>,
    pub accumulated_elapsed: std::time::Duration,
}

impl Default for ReplayController {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayController {
    pub fn new() -> Self {
        Self {
            is_active: false,
            history: Vec::new(),
            cursor: 0,
            speed: ReplaySpeed::Normal,
            is_playing: false,
            last_tick_at: None,
            accumulated_elapsed: std::time::Duration::ZERO,
        }
    }

    /// Enter replay mode with specified historical events.
    pub fn load_history(&mut self, history: Vec<EventEnvelope>) {
        self.cursor = history.len().saturating_sub(1);
        self.history = history;
        self.is_active = true;
        self.is_playing = false;
        self.last_tick_at = None;
        self.accumulated_elapsed = std::time::Duration::ZERO;
    }

    /// Exit replay mode and return to live mission monitoring.
    pub fn exit(&mut self) {
        self.is_active = false;
        self.is_playing = false;
        self.last_tick_at = None;
        self.accumulated_elapsed = std::time::Duration::ZERO;
    }

    /// Step backward 1 event along the timeline.
    pub fn step_backward(&mut self) -> bool {
        self.accumulated_elapsed = std::time::Duration::ZERO;
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else {
            false
        }
    }

    /// Step forward 1 event along the timeline.
    pub fn step_forward(&mut self) -> bool {
        if !self.history.is_empty() && self.cursor + 1 < self.history.len() {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    /// Jump directly to target event index.
    pub fn jump_to(&mut self, target: usize) {
        if !self.history.is_empty() {
            self.cursor = target.min(self.history.len() - 1);
            self.accumulated_elapsed = std::time::Duration::ZERO;
        }
    }

    /// Seek to beginning of timeline.
    pub fn seek_to_start(&mut self) -> bool {
        self.accumulated_elapsed = std::time::Duration::ZERO;
        if self.cursor != 0 {
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    /// Seek to end of timeline.
    pub fn seek_to_end(&mut self) -> bool {
        self.accumulated_elapsed = std::time::Duration::ZERO;
        if !self.history.is_empty() {
            let target = self.history.len() - 1;
            if self.cursor != target {
                self.cursor = target;
                return true;
            }
        }
        false
    }

    /// Toggle automatic timeline playback.
    pub fn toggle_playback(&mut self) {
        self.is_playing = !self.is_playing;
        self.last_tick_at = if self.is_playing {
            Some(std::time::Instant::now())
        } else {
            None
        };
        self.accumulated_elapsed = std::time::Duration::ZERO;
    }

    /// Advance playback by an explicit elapsed duration.
    ///
    /// Pacing is monotonic and timeline-stable, independent of UI polling
    /// schedule, frame rate, or rerender frequency.
    pub fn advance_elapsed(&mut self, elapsed: std::time::Duration) -> bool {
        if !self.is_playing || self.history.is_empty() {
            return false;
        }

        self.accumulated_elapsed += elapsed;
        let interval = self.speed.step_interval();
        if self.accumulated_elapsed >= interval {
            let steps = (self.accumulated_elapsed.as_millis() / interval.as_millis()) as usize;
            self.accumulated_elapsed = std::time::Duration::from_millis(
                (self.accumulated_elapsed.as_millis() % interval.as_millis()) as u64,
            );

            let mut advanced = false;
            for _ in 0..steps {
                if self.step_forward() {
                    advanced = true;
                } else {
                    self.is_playing = false;
                    self.accumulated_elapsed = std::time::Duration::ZERO;
                    self.last_tick_at = None;
                    break;
                }
            }
            advanced
        } else {
            false
        }
    }

    /// Advance playback using the monotonic system clock.
    /// Returns true if cursor advanced and view should re-render.
    pub fn tick_playback(&mut self) -> bool {
        if !self.is_playing || self.history.is_empty() {
            self.last_tick_at = None;
            return false;
        }

        let now = std::time::Instant::now();
        let elapsed = if let Some(prev) = self.last_tick_at {
            now.saturating_duration_since(prev)
        } else {
            std::time::Duration::ZERO
        };
        self.last_tick_at = Some(now);
        self.advance_elapsed(elapsed)
    }

    /// Reconstruct read-only projection at current timeline cursor (D-19).
    /// Generates zero side effects, zero DB writes, and zero tool invocations.
    pub fn reconstruct_current_view(&self) -> TuiViewModel {
        let mut model = TuiViewModel::new();
        let end_idx = if self.history.is_empty() {
            0
        } else {
            (self.cursor + 1).min(self.history.len())
        };

        model.reconstruct_from_events(&self.history[..end_idx]);
        model
    }

    /// Enforce Law 9 & D-19: Replay mode strictly disallows mutating runtime actions.
    pub fn is_mutation_allowed(&self) -> bool {
        !self.is_active
    }

    /// Process keyboard inputs while in replay mode.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.is_active {
            return false;
        }

        match key.code {
            KeyCode::Char('[') | KeyCode::Left => self.step_backward(),
            KeyCode::Char(']') | KeyCode::Right => self.step_forward(),
            KeyCode::Char(' ') | KeyCode::Char('p') | KeyCode::Char('P') => {
                self.toggle_playback();
                true
            }
            KeyCode::Home => self.seek_to_start(),
            KeyCode::End => self.seek_to_end(),
            KeyCode::Char('1') => {
                self.speed = ReplaySpeed::Normal;
                true
            }
            KeyCode::Char('2') => {
                self.speed = ReplaySpeed::Fast;
                true
            }
            KeyCode::Char('4') => {
                self.speed = ReplaySpeed::Hyper;
                true
            }
            KeyCode::Esc => {
                self.exit();
                true
            }
            _ => false,
        }
    }
}
