//! Audio streaming models and voice interface protocol buffers.
//!
//! Supports bidirectional audio streaming, speech-to-text input, and text-to-speech feedback.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_SDP_SIZE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct AudioControls {
    pub microphone_muted: bool,
    pub speaker_suppressed: bool,
}

/// Peaks contain normalized volume levels (0..65535) for real-time VU meters.
#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioState {
    pub microphone_peak: u16,
    pub speaker_peak: u16,
}

impl AudioState {
    /// Returns visual VU meter bars (e.g. `[====    ]`) for terminal rendering.
    pub fn render_vu_bars(&self, width: usize) -> (String, String) {
        let mic_fill = ((self.microphone_peak as f32 / 65535.0) * width as f32).round() as usize;
        let spk_fill = ((self.speaker_peak as f32 / 65535.0) * width as f32).round() as usize;

        let mic_bar = format!(
            "[{}{}]",
            "=".repeat(mic_fill.min(width)),
            " ".repeat(width.saturating_sub(mic_fill))
        );
        let spk_bar = format!(
            "[{}{}]",
            "=".repeat(spk_fill.min(width)),
            " ".repeat(width.saturating_sub(spk_fill))
        );

        (mic_bar, spk_bar)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioDeviceKind {
    Input,
    Output,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub channels: u16,
    pub is_default: bool,
    pub kind: AudioDeviceKind,
}

/// SDP session description with debug redaction and 64 KiB length validation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SessionDescription(String);

impl TryFrom<String> for SessionDescription {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_SDP_SIZE_BYTES {
            return Err(
                "invalid voice session description length: must be between 1 and 65536 bytes",
            );
        }
        Ok(Self(trimmed.to_string()))
    }
}

impl From<SessionDescription> for String {
    fn from(s: SessionDescription) -> Self {
        s.0
    }
}

impl fmt::Debug for SessionDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionDescription([REDACTED_ICE_CREDENTIALS])")
    }
}

impl SessionDescription {
    pub fn new(sdp: &str) -> Result<Self> {
        Self::try_from(sdp.to_string()).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn sdp(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceHostControlMessage {
    StartVoiceSession {
        device_input: Option<String>,
        device_output: Option<String>,
    },
    StopVoiceSession,
    UpdateControls {
        controls: AudioControls,
    },
    OfferSdp {
        sdp: SessionDescription,
    },
    AnswerSdp {
        sdp: SessionDescription,
    },
    AudioLevelUpdate {
        levels: AudioState,
    },
}

/// Discovers and enumerates host audio input and output devices.
pub struct AudioDeviceManager;

impl AudioDeviceManager {
    /// Enumerate audio devices on Linux from procfs or return fallback defaults.
    pub fn enumerate_devices() -> Vec<AudioDevice> {
        let mut devices = Vec::new();
        let pcm_path = std::path::Path::new("/proc/asound/pcm");

        if let Ok(content) = std::fs::read_to_string(pcm_path) {
            let mut seen_input = false;
            let mut seen_output = false;

            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    let id = parts[0];
                    let name = parts[1];
                    let caps = parts[2..].join(":");

                    let has_playback = caps.contains("playback");
                    let has_capture = caps.contains("capture");

                    if has_capture {
                        devices.push(AudioDevice {
                            name: format!("{} ({})", name, id),
                            channels: 2,
                            is_default: !seen_input,
                            kind: AudioDeviceKind::Input,
                        });
                        seen_input = true;
                    }

                    if has_playback {
                        devices.push(AudioDevice {
                            name: format!("{} ({})", name, id),
                            channels: 2,
                            is_default: !seen_output,
                            kind: AudioDeviceKind::Output,
                        });
                        seen_output = true;
                    }
                }
            }
        }

        // Truthful reporting: do NOT inject fake devices when no hardware exists
        devices
    }
}

/// Adaptive Voice Activity Detector (VAD) measuring RMS energy and speech hangover.
#[derive(Debug, Clone)]
pub struct VoiceActivityDetector {
    energy_threshold: u16,
    hangover_frames: usize,
    active_count: usize,
    speech_detected: bool,
}

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        Self::new(3000, 5) // ~3000 RMS threshold, 5 frames hangover
    }
}

impl VoiceActivityDetector {
    pub fn new(energy_threshold: u16, hangover_frames: usize) -> Self {
        Self {
            energy_threshold,
            hangover_frames,
            active_count: 0,
            speech_detected: false,
        }
    }

    /// Evaluates whether an incoming PCM audio frame contains human voice activity.
    pub fn process_frame(&mut self, samples: &[i16]) -> bool {
        if samples.is_empty() {
            if self.active_count > 0 {
                self.active_count -= 1;
            } else {
                self.speech_detected = false;
            }
            return self.speech_detected;
        }

        let mut sum_sq: f64 = 0.0;
        for &s in samples {
            let val = s as f64;
            sum_sq += val * val;
        }
        let rms = ((sum_sq / samples.len() as f64).sqrt() as u32 * 65535 / 32767).min(65535) as u16;

        if rms >= self.energy_threshold {
            self.active_count = self.hangover_frames;
            self.speech_detected = true;
        } else if self.active_count > 0 {
            self.active_count -= 1;
            self.speech_detected = true;
        } else {
            self.speech_detected = false;
        }

        self.speech_detected
    }

    pub fn is_speech_active(&self) -> bool {
        self.speech_detected
    }
}

/// Circular jitter buffer for processing 16-bit PCM audio frames.
#[derive(Debug, Clone)]
pub struct AudioJitterBuffer {
    capacity_samples: usize,
    buffer: std::collections::VecDeque<i16>,
}

impl AudioJitterBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            capacity_samples,
            buffer: std::collections::VecDeque::with_capacity(capacity_samples),
        }
    }

    /// Push raw PCM samples into the jitter buffer, evicting oldest if capacity reached.
    pub fn push_samples(&mut self, samples: &[i16]) {
        for &sample in samples {
            if self.buffer.len() >= self.capacity_samples {
                self.buffer.pop_front();
            }
            self.buffer.push_back(sample);
        }
    }

    /// Computes instantaneous peak amplitude (0..65535) and RMS energy over current buffer.
    pub fn calculate_levels(&self) -> (u16, u16) {
        if self.buffer.is_empty() {
            return (0, 0);
        }

        let mut max_abs: u16 = 0;
        let mut sum_sq: f64 = 0.0;

        for &sample in &self.buffer {
            let abs_val = sample.unsigned_abs();
            if abs_val > max_abs {
                max_abs = abs_val;
            }
            let s_f64 = sample as f64;
            sum_sq += s_f64 * s_f64;
        }

        let rms = (sum_sq / self.buffer.len() as f64).sqrt() as u16;
        let normalized_peak = ((max_abs as u32 * 65535) / 32767).min(65535) as u16;
        let normalized_rms = ((rms as u32 * 65535) / 32767).min(65535) as u16;

        (normalized_peak, normalized_rms)
    }

    /// Drain up to `count` samples from the front of the buffer.
    pub fn drain_samples(&mut self, count: usize) -> Vec<i16> {
        let to_take = count.min(self.buffer.len());
        self.buffer.drain(..to_take).collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// Real-time voice session state machine and coordinator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceSessionStatus {
    Idle,
    Connecting,
    Connected,
    Muted,
    Closed,
    HardwareUnavailable,
    ProviderUnsupported,
}

#[derive(Debug)]
pub struct VoiceSessionCoordinator {
    pub status: VoiceSessionStatus,
    pub controls: AudioControls,
    pub mic_buffer: AudioJitterBuffer,
    pub speaker_buffer: AudioJitterBuffer,
    pub vad: VoiceActivityDetector,
}

impl Default for VoiceSessionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceSessionCoordinator {
    pub fn new() -> Self {
        Self {
            status: VoiceSessionStatus::Idle,
            controls: AudioControls::default(),
            mic_buffer: AudioJitterBuffer::new(48000), // ~2s of 24kHz PCM
            speaker_buffer: AudioJitterBuffer::new(48000),
            vad: VoiceActivityDetector::default(),
        }
    }

    pub fn start_session(&mut self) {
        if AudioDeviceManager::enumerate_devices().is_empty() {
            self.status = VoiceSessionStatus::HardwareUnavailable;
        } else {
            self.status = VoiceSessionStatus::Connecting;
        }
    }

    pub fn try_start_session(&mut self) -> Result<()> {
        let devices = AudioDeviceManager::enumerate_devices();
        if devices.is_empty() {
            self.status = VoiceSessionStatus::HardwareUnavailable;
            anyhow::bail!(
                "Cannot start voice session: no audio input/output devices detected on host"
            );
        }
        self.status = VoiceSessionStatus::Connecting;
        Ok(())
    }

    /// Explicitly transitions to `Connected` only when an active audio stream or SDP session is established.
    pub fn establish_stream(&mut self) -> Result<()> {
        if self.status != VoiceSessionStatus::Connecting {
            anyhow::bail!(
                "Cannot establish audio stream: session is in {:?} state, expected Connecting",
                self.status
            );
        }
        self.status = VoiceSessionStatus::Connected;
        Ok(())
    }

    /// Handles host voice control messages, transitioning session status accordingly.
    pub fn handle_control_message(&mut self, msg: VoiceHostControlMessage) -> Result<()> {
        match msg {
            VoiceHostControlMessage::StartVoiceSession { .. } => {
                self.try_start_session()?;
            }
            VoiceHostControlMessage::StopVoiceSession => {
                self.stop_session();
            }
            VoiceHostControlMessage::UpdateControls { controls } => {
                self.controls = controls;
            }
            VoiceHostControlMessage::OfferSdp { .. }
            | VoiceHostControlMessage::AnswerSdp { .. } => {
                if self.status == VoiceSessionStatus::Connecting {
                    self.establish_stream()?;
                }
            }
            VoiceHostControlMessage::AudioLevelUpdate { .. } => {}
        }
        Ok(())
    }

    pub fn stop_session(&mut self) {
        self.status = VoiceSessionStatus::Closed;
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.controls.microphone_muted = !self.controls.microphone_muted;
        self.controls.microphone_muted
    }

    pub fn set_speaker_suppressed(&mut self, suppressed: bool) {
        self.controls.speaker_suppressed = suppressed;
    }

    /// Ingests incoming microphone samples (unless muted), updates VAD, and computes live audio levels.
    pub fn process_mic_input(&mut self, samples: &[i16]) -> AudioState {
        self.vad.process_frame(samples);
        if !self.controls.microphone_muted {
            self.mic_buffer.push_samples(samples);
        }
        let (mic_peak, _) = self.mic_buffer.calculate_levels();
        let (spk_peak, _) = self.speaker_buffer.calculate_levels();

        AudioState {
            microphone_peak: if self.controls.microphone_muted {
                0
            } else {
                mic_peak
            },
            speaker_peak: if self.controls.speaker_suppressed {
                0
            } else {
                spk_peak
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_controls_defaults() {
        let controls = AudioControls::default();
        assert!(!controls.microphone_muted);
        assert!(!controls.speaker_suppressed);
    }

    #[test]
    fn test_audio_state_vu_meter_rendering() {
        let state = AudioState {
            microphone_peak: 32768, // ~50%
            speaker_peak: 65535,    // 100%
        };
        let (mic_bar, spk_bar) = state.render_vu_bars(10);
        assert_eq!(mic_bar, "[=====     ]");
        assert_eq!(spk_bar, "[==========]");
    }

    #[test]
    fn test_session_description_bounds_and_redaction() {
        assert!(SessionDescription::new("").is_err());
        let valid =
            SessionDescription::new("v=0\r\no=- 12345 2 IN IP4 127.0.0.1\r\ns=-\r\n").unwrap();
        assert_eq!(
            format!("{:?}", valid),
            "SessionDescription([REDACTED_ICE_CREDENTIALS])"
        );
        assert!(valid.sdp().contains("v=0"));
    }

    #[test]
    fn test_audio_device_enumeration() {
        let devices = AudioDeviceManager::enumerate_devices();
        assert!(!devices.is_empty());
        assert!(devices.iter().any(|d| d.kind == AudioDeviceKind::Input));
        assert!(devices.iter().any(|d| d.kind == AudioDeviceKind::Output));
    }

    #[test]
    fn test_audio_jitter_buffer_levels_and_drain() {
        let mut buffer = AudioJitterBuffer::new(100);
        assert!(buffer.is_empty());

        let samples: Vec<i16> = (0..50)
            .map(|i| if i % 2 == 0 { 16384 } else { -16384 })
            .collect();
        buffer.push_samples(&samples);
        assert_eq!(buffer.len(), 50);

        let (peak, rms) = buffer.calculate_levels();
        assert!(peak > 30000);
        assert!(rms > 30000);

        let drained = buffer.drain_samples(20);
        assert_eq!(drained.len(), 20);
        assert_eq!(buffer.len(), 30);
    }

    #[test]
    fn test_voice_session_coordinator_lifecycle_and_mute() {
        let mut coord = VoiceSessionCoordinator::new();
        assert_eq!(coord.status, VoiceSessionStatus::Idle);

        coord.start_session();
        assert_eq!(coord.status, VoiceSessionStatus::Connecting);

        // Stream establishment is required to reach Connected
        coord.establish_stream().expect("must establish stream");
        assert_eq!(coord.status, VoiceSessionStatus::Connected);

        let samples = vec![10000i16; 100];
        let state = coord.process_mic_input(&samples);
        assert!(state.microphone_peak > 0);

        // Mute mic
        let is_muted = coord.toggle_mute();
        assert!(is_muted);
        let state_muted = coord.process_mic_input(&samples);
        assert_eq!(state_muted.microphone_peak, 0);

        coord.stop_session();
        assert_eq!(coord.status, VoiceSessionStatus::Closed);
    }

    #[test]
    fn test_voice_session_sdp_handshake_establishes_connection() {
        let mut coord = VoiceSessionCoordinator::new();
        coord.start_session();
        assert_eq!(coord.status, VoiceSessionStatus::Connecting);

        let sdp =
            SessionDescription::new("v=0\r\no=- 12345 2 IN IP4 127.0.0.1\r\ns=-\r\n").unwrap();
        coord
            .handle_control_message(VoiceHostControlMessage::AnswerSdp { sdp })
            .expect("handshake must succeed");
        assert_eq!(coord.status, VoiceSessionStatus::Connected);
    }
}
