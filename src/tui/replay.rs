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
}

/// Controller for navigating historical mission checkpoints and event timelines.
pub struct ReplayController {
    pub is_active: bool,
    pub history: Vec<EventEnvelope>,
    pub cursor: usize,
    pub speed: ReplaySpeed,
    pub is_playing: bool,
    pub tick_accumulator: usize,
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
            tick_accumulator: 0,
        }
    }

    /// Enter replay mode with specified historical events.
    pub fn load_history(&mut self, history: Vec<EventEnvelope>) {
        self.cursor = history.len().saturating_sub(1);
        self.history = history;
        self.is_active = true;
        self.is_playing = false;
        self.tick_accumulator = 0;
    }

    /// Exit replay mode and return to live mission monitoring.
    pub fn exit(&mut self) {
        self.is_active = false;
        self.is_playing = false;
        self.tick_accumulator = 0;
    }

    /// Step backward 1 event along the timeline.
    pub fn step_backward(&mut self) -> bool {
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
        }
    }

    /// Seek to beginning of timeline.
    pub fn seek_to_start(&mut self) -> bool {
        if self.cursor != 0 {
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    /// Seek to end of timeline.
    pub fn seek_to_end(&mut self) -> bool {
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
        self.tick_accumulator = 0;
    }

    /// Advance playback according to chosen speed multiplier.
    /// Returns true if cursor advanced and view should re-render.
    pub fn tick_playback(&mut self) -> bool {
        if !self.is_playing || self.history.is_empty() {
            return false;
        }

        let interval = match self.speed {
            ReplaySpeed::Normal => 4,
            ReplaySpeed::Fast => 2,
            ReplaySpeed::Hyper => 1,
        };

        self.tick_accumulator = self.tick_accumulator.saturating_add(1);
        if self.tick_accumulator >= interval {
            self.tick_accumulator = 0;
            if self.step_forward() {
                true
            } else {
                // Reached the end of the timeline
                self.is_playing = false;
                false
            }
        } else {
            false
        }
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
