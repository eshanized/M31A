//! Realtime conversation state machine, VAD event processing, and audio turn synchronization.
//!
//! Provides the core state machine for bidirectional realtime voice and text agent sessions:
//! - Conversation states (`Disconnected`, `Listening`, `UserSpeaking`, `Thinking`, `AgentSpeaking`, `Interrupted`).
//! - Voice Activity Detection (VAD) events (`SpeechStarted`, `SpeechEnded`, `BargeInDetected`).
//! - Barge-in interruption handling that instantly halts model output and resets playback when the user interrupts.
//! - Audio chunk ring buffers and turn synchronization.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Realtime conversation lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationState {
    /// No active voice or realtime connection.
    Disconnected,
    /// Connected and awaiting user voice or input.
    Listening,
    /// User is currently speaking (VAD active).
    UserSpeaking,
    /// Speech has concluded; model inference is processing.
    Thinking,
    /// Model audio response is actively playing back.
    AgentSpeaking,
    /// User interrupted during agent speech; aborting playback and transitioning to UserSpeaking.
    Interrupted,
}

/// Incoming events processed by the conversation coordinator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RealtimeEvent {
    /// Connection established.
    SessionConnected,
    /// VAD detected user voice onset.
    SpeechStarted { energy_level: u16 },
    /// VAD detected user voice pause/cessation.
    SpeechEnded { duration_ms: u64 },
    /// Model generated start of audio turn.
    AgentTurnStarted { turn_id: String },
    /// Model finished playing response.
    AgentTurnCompleted { turn_id: String },
    /// Explicit cancellation or disconnect.
    SessionDisconnected,
}

/// Action to be taken by the audio runner or TUI in response to state transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationAction {
    None,
    StartAudioCapture,
    StopAudioCapture,
    CancelAgentPlayback,
    BeginThinkingIndicator,
    PlayAudioResponse { turn_id: String },
}

/// Realtime conversation coordinator.
#[derive(Debug)]
pub struct RealtimeConversationCoordinator {
    state: Mutex<ConversationState>,
    active_turn_id: Mutex<Option<String>>,
    audio_frame_buffer: Mutex<VecDeque<Vec<u8>>>,
    max_buffered_frames: usize,
    interruption_count: Mutex<u64>,
}

impl Default for RealtimeConversationCoordinator {
    fn default() -> Self {
        Self::new(100) // Buffer up to 100 frames (~2-5 seconds)
    }
}

impl RealtimeConversationCoordinator {
    pub fn new(max_buffered_frames: usize) -> Self {
        Self {
            state: Mutex::new(ConversationState::Disconnected),
            active_turn_id: Mutex::new(None),
            audio_frame_buffer: Mutex::new(VecDeque::new()),
            max_buffered_frames,
            interruption_count: Mutex::new(0),
        }
    }

    /// Retrieve the current conversation state.
    pub fn current_state(&self) -> ConversationState {
        *self.state.lock().unwrap()
    }

    /// Retrieve total number of barge-in interruptions detected in this session.
    pub fn interruption_count(&self) -> u64 {
        *self.interruption_count.lock().unwrap()
    }

    /// Process an incoming realtime event, driving state transitions and emitting necessary actions.
    pub fn handle_event(&self, event: RealtimeEvent) -> (ConversationState, ConversationAction) {
        let mut state = self.state.lock().unwrap();
        let mut turn_id = self.active_turn_id.lock().unwrap();

        let (new_state, action) = match (*state, event) {
            (ConversationState::Disconnected, RealtimeEvent::SessionConnected) => (
                ConversationState::Listening,
                ConversationAction::StartAudioCapture,
            ),
            (ConversationState::Listening, RealtimeEvent::SpeechStarted { .. }) => {
                (ConversationState::UserSpeaking, ConversationAction::None)
            }
            (ConversationState::UserSpeaking, RealtimeEvent::SpeechEnded { .. }) => (
                ConversationState::Thinking,
                ConversationAction::BeginThinkingIndicator,
            ),
            (ConversationState::Thinking, RealtimeEvent::AgentTurnStarted { turn_id: tid }) => {
                *turn_id = Some(tid.clone());
                (
                    ConversationState::AgentSpeaking,
                    ConversationAction::PlayAudioResponse { turn_id: tid },
                )
            }
            // --- BARGE-IN INTERRUPTION ---
            (ConversationState::AgentSpeaking, RealtimeEvent::SpeechStarted { .. }) => {
                *self.interruption_count.lock().unwrap() += 1;
                *turn_id = None;
                (
                    ConversationState::Interrupted,
                    ConversationAction::CancelAgentPlayback,
                )
            }
            (ConversationState::Interrupted, RealtimeEvent::SpeechEnded { .. }) => (
                ConversationState::Thinking,
                ConversationAction::BeginThinkingIndicator,
            ),
            (ConversationState::AgentSpeaking, RealtimeEvent::AgentTurnCompleted { .. }) => {
                *turn_id = None;
                (ConversationState::Listening, ConversationAction::None)
            }
            (_, RealtimeEvent::SessionDisconnected) => {
                *turn_id = None;
                (
                    ConversationState::Disconnected,
                    ConversationAction::StopAudioCapture,
                )
            }
            // Fallback: maintain current state
            (s, _) => (s, ConversationAction::None),
        };

        *state = new_state;
        (new_state, action)
    }

    /// Push an encoded audio frame into the buffer.
    pub fn push_audio_frame(&self, frame: Vec<u8>) {
        let mut buf = self.audio_frame_buffer.lock().unwrap();
        if buf.len() >= self.max_buffered_frames {
            buf.pop_front();
        }
        buf.push_back(frame);
    }

    /// Drain all buffered audio frames.
    pub fn drain_audio_frames(&self) -> Vec<Vec<u8>> {
        let mut buf = self.audio_frame_buffer.lock().unwrap();
        buf.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_lifecycle_connect_and_turn() {
        let coord = RealtimeConversationCoordinator::default();
        assert_eq!(coord.current_state(), ConversationState::Disconnected);

        // Connect
        let (s1, a1) = coord.handle_event(RealtimeEvent::SessionConnected);
        assert_eq!(s1, ConversationState::Listening);
        assert_eq!(a1, ConversationAction::StartAudioCapture);

        // User speaks
        let (s2, a2) = coord.handle_event(RealtimeEvent::SpeechStarted { energy_level: 500 });
        assert_eq!(s2, ConversationState::UserSpeaking);
        assert_eq!(a2, ConversationAction::None);

        // User stops speaking
        let (s3, a3) = coord.handle_event(RealtimeEvent::SpeechEnded { duration_ms: 1200 });
        assert_eq!(s3, ConversationState::Thinking);
        assert_eq!(a3, ConversationAction::BeginThinkingIndicator);

        // Model responds
        let (s4, a4) = coord.handle_event(RealtimeEvent::AgentTurnStarted {
            turn_id: "turn-v-1".to_string(),
        });
        assert_eq!(s4, ConversationState::AgentSpeaking);
        assert_eq!(
            a4,
            ConversationAction::PlayAudioResponse {
                turn_id: "turn-v-1".to_string()
            }
        );

        // Model finishes
        let (s5, a5) = coord.handle_event(RealtimeEvent::AgentTurnCompleted {
            turn_id: "turn-v-1".to_string(),
        });
        assert_eq!(s5, ConversationState::Listening);
        assert_eq!(a5, ConversationAction::None);
    }

    #[test]
    fn test_barge_in_interruption_cancels_playback() {
        let coord = RealtimeConversationCoordinator::default();
        coord.handle_event(RealtimeEvent::SessionConnected);
        coord.handle_event(RealtimeEvent::SpeechStarted { energy_level: 200 });
        coord.handle_event(RealtimeEvent::SpeechEnded { duration_ms: 800 });
        coord.handle_event(RealtimeEvent::AgentTurnStarted {
            turn_id: "turn-v-2".to_string(),
        });
        assert_eq!(coord.current_state(), ConversationState::AgentSpeaking);

        // User starts speaking while agent is speaking -> BARGE IN!
        let (s_barge, a_barge) =
            coord.handle_event(RealtimeEvent::SpeechStarted { energy_level: 600 });
        assert_eq!(s_barge, ConversationState::Interrupted);
        assert_eq!(a_barge, ConversationAction::CancelAgentPlayback);
        assert_eq!(coord.interruption_count(), 1);

        // User finishes speech
        let (s_end, a_end) = coord.handle_event(RealtimeEvent::SpeechEnded { duration_ms: 500 });
        assert_eq!(s_end, ConversationState::Thinking);
        assert_eq!(a_end, ConversationAction::BeginThinkingIndicator);
    }

    #[test]
    fn test_audio_frame_buffer_push_and_drain() {
        let coord = RealtimeConversationCoordinator::new(3);
        coord.push_audio_frame(vec![1, 2, 3]);
        coord.push_audio_frame(vec![4, 5, 6]);
        coord.push_audio_frame(vec![7, 8, 9]);
        coord.push_audio_frame(vec![10, 11, 12]); // Should evict [1, 2, 3]

        let frames = coord.drain_audio_frames();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], vec![4, 5, 6]);
        assert_eq!(frames[2], vec![10, 11, 12]);
    }
}
