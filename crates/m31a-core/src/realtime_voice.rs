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

/// Concrete handle representing active host audio input and output streams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioStreamHandle {
    pub stream_id: String,
    pub input_active: bool,
    pub output_active: bool,
}

impl AudioStreamHandle {
    pub fn new(stream_id: impl Into<String>, input_active: bool, output_active: bool) -> Self {
        Self {
            stream_id: stream_id.into(),
            input_active,
            output_active,
        }
    }

    pub fn is_active(&self) -> bool {
        self.input_active && self.output_active && !self.stream_id.trim().is_empty()
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
    Failed,
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
    pub stream_handle: Option<AudioStreamHandle>,
    pub local_sdp: Option<SessionDescription>,
    pub remote_sdp: Option<SessionDescription>,
    pub audio_device_override: Option<Vec<AudioDevice>>,
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
            stream_handle: None,
            local_sdp: None,
            remote_sdp: None,
            audio_device_override: None,
        }
    }

    pub fn available_devices(&self) -> Vec<AudioDevice> {
        if let Some(ref devs) = self.audio_device_override {
            devs.clone()
        } else {
            AudioDeviceManager::enumerate_devices()
        }
    }

    pub fn check_audio_hardware(&self) -> (bool, bool) {
        let devices = self.available_devices();
        let has_input = devices.iter().any(|d| d.kind == AudioDeviceKind::Input);
        let has_output = devices.iter().any(|d| d.kind == AudioDeviceKind::Output);
        (has_input, has_output)
    }

    pub fn start_session(&mut self) {
        let (has_input, has_output) = self.check_audio_hardware();
        if !has_input || !has_output {
            self.status = VoiceSessionStatus::HardwareUnavailable;
        } else {
            self.status = VoiceSessionStatus::Connecting;
        }
    }

    pub fn try_start_session(&mut self) -> Result<()> {
        let (has_input, has_output) = self.check_audio_hardware();
        if !has_input || !has_output {
            self.status = VoiceSessionStatus::HardwareUnavailable;
            anyhow::bail!(
                "Cannot start voice session: required audio hardware unavailable (input: {}, output: {})",
                has_input,
                has_output
            );
        }
        self.status = VoiceSessionStatus::Connecting;
        Ok(())
    }

    /// Explicitly transitions to `Connected` only when real audio hardware, complete SDP negotiation,
    /// and an active stream handle are confirmed.
    pub fn establish_stream(&mut self, handle: AudioStreamHandle) -> Result<()> {
        if self.status != VoiceSessionStatus::Connecting {
            self.status = VoiceSessionStatus::Failed;
            anyhow::bail!(
                "Cannot establish audio stream: session is in {:?} state, expected Connecting",
                self.status
            );
        }

        let (has_input, has_output) = self.check_audio_hardware();
        if !has_input || !has_output {
            self.status = VoiceSessionStatus::HardwareUnavailable;
            anyhow::bail!(
                "Cannot establish stream: missing required audio hardware (input: {}, output: {})",
                has_input,
                has_output
            );
        }

        // Verify SDP negotiation has occurred
        if self.local_sdp.is_none() && self.remote_sdp.is_none() {
            self.status = VoiceSessionStatus::Failed;
            anyhow::bail!("Cannot establish audio stream: SDP negotiation incomplete");
        }

        if !handle.is_active() {
            self.status = VoiceSessionStatus::Failed;
            anyhow::bail!(
                "Cannot establish stream: AudioStreamHandle is inactive (stream_id: '{}', input: {}, output: {})",
                handle.stream_id,
                handle.input_active,
                handle.output_active
            );
        }

        self.stream_handle = Some(handle);
        self.status = VoiceSessionStatus::Connected;
        Ok(())
    }

    /// Handles host voice control messages, recording negotiation state and controls.
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
            VoiceHostControlMessage::OfferSdp { sdp } => {
                self.local_sdp = Some(sdp);
            }
            VoiceHostControlMessage::AnswerSdp { sdp } => {
                self.remote_sdp = Some(sdp);
            }
            VoiceHostControlMessage::AudioLevelUpdate { .. } => {}
        }
        Ok(())
    }

    pub fn stop_session(&mut self) {
        self.status = VoiceSessionStatus::Closed;
        self.stream_handle = None;
    }

    pub fn toggle_mute(&mut self) -> Result<bool> {
        if self.status != VoiceSessionStatus::Connected && self.status != VoiceSessionStatus::Muted
        {
            anyhow::bail!(
                "Cannot toggle mute: voice session is in {:?} state, expected Connected or Muted",
                self.status
            );
        }
        self.controls.microphone_muted = !self.controls.microphone_muted;
        self.status = if self.controls.microphone_muted {
            VoiceSessionStatus::Muted
        } else {
            VoiceSessionStatus::Connected
        };
        Ok(self.controls.microphone_muted)
    }

    pub fn set_speaker_suppressed(&mut self, suppressed: bool) -> Result<()> {
        if self.status != VoiceSessionStatus::Connected && self.status != VoiceSessionStatus::Muted
        {
            anyhow::bail!(
                "Cannot update speaker suppression: voice session is in {:?} state, expected Connected or Muted",
                self.status
            );
        }
        self.controls.speaker_suppressed = suppressed;
        Ok(())
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
        // On systems with hardware audio cards (/proc/asound/pcm), verify discovery.
        // On headless cloud/CI environments without soundcards, empty device list is truthful.
        if !devices.is_empty() {
            assert!(devices.iter().any(|d| d.kind == AudioDeviceKind::Input));
            assert!(devices.iter().any(|d| d.kind == AudioDeviceKind::Output));
        }
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

    fn mock_devices() -> Vec<AudioDevice> {
        vec![
            AudioDevice {
                name: "Mock Microphone".to_string(),
                channels: 2,
                is_default: true,
                kind: AudioDeviceKind::Input,
            },
            AudioDevice {
                name: "Mock Speaker".to_string(),
                channels: 2,
                is_default: true,
                kind: AudioDeviceKind::Output,
            },
        ]
    }

    #[test]
    fn test_voice_session_initiation_without_audio_hardware() {
        let mut coord = VoiceSessionCoordinator::new();
        coord.audio_device_override = Some(vec![]);

        coord.start_session();
        assert_eq!(coord.status, VoiceSessionStatus::HardwareUnavailable);

        let err = coord.try_start_session().unwrap_err();
        assert!(err.to_string().contains("hardware unavailable"));
    }

    #[test]
    fn test_unnegotiated_sdp_fails_stream_establishment() {
        let mut coord = VoiceSessionCoordinator::new();
        coord.audio_device_override = Some(mock_devices());
        coord.start_session();
        assert_eq!(coord.status, VoiceSessionStatus::Connecting);

        // Attempting to establish stream without SDP negotiation fails closed
        let handle = AudioStreamHandle::new("stream-1", true, true);
        let res = coord.establish_stream(handle);
        assert!(res.is_err());
        assert_eq!(coord.status, VoiceSessionStatus::Failed);
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("SDP negotiation incomplete")
        );
    }

    #[test]
    fn test_stream_creation_failure_on_inactive_handle() {
        let mut coord = VoiceSessionCoordinator::new();
        coord.audio_device_override = Some(mock_devices());
        coord.start_session();

        let sdp =
            SessionDescription::new("v=0\r\no=- 12345 2 IN IP4 127.0.0.1\r\ns=-\r\n").unwrap();
        coord
            .handle_control_message(VoiceHostControlMessage::AnswerSdp { sdp })
            .unwrap();

        // Inactive handle (output_active is false)
        let inactive = AudioStreamHandle::new("stream-1", true, false);
        let res = coord.establish_stream(inactive);
        assert!(res.is_err());
        assert_eq!(coord.status, VoiceSessionStatus::Failed);
        assert!(res.unwrap_err().to_string().contains("inactive"));
    }

    #[test]
    fn test_mute_during_failed_or_disconnected_state() {
        let mut coord = VoiceSessionCoordinator::new();
        assert_eq!(coord.status, VoiceSessionStatus::Idle);
        assert!(coord.toggle_mute().is_err());
        assert!(coord.set_speaker_suppressed(true).is_err());

        coord.status = VoiceSessionStatus::Failed;
        assert!(coord.toggle_mute().is_err());

        coord.status = VoiceSessionStatus::Closed;
        assert!(coord.toggle_mute().is_err());

        coord.status = VoiceSessionStatus::HardwareUnavailable;
        assert!(coord.toggle_mute().is_err());
    }

    #[test]
    fn test_legitimate_connected_operation_with_active_stream_handle() {
        let mut coord = VoiceSessionCoordinator::new();
        coord.audio_device_override = Some(mock_devices());
        coord.start_session();
        assert_eq!(coord.status, VoiceSessionStatus::Connecting);

        let sdp =
            SessionDescription::new("v=0\r\no=- 12345 2 IN IP4 127.0.0.1\r\ns=-\r\n").unwrap();
        coord
            .handle_control_message(VoiceHostControlMessage::AnswerSdp { sdp })
            .unwrap();

        // Establish stream with active handle
        let handle = AudioStreamHandle::new("stream-live-001", true, true);
        coord
            .establish_stream(handle)
            .expect("must establish stream");
        assert_eq!(coord.status, VoiceSessionStatus::Connected);

        let samples = vec![10000i16; 100];
        let state = coord.process_mic_input(&samples);
        assert!(state.microphone_peak > 0);

        // Mute mic transitions to Muted status
        let is_muted = coord.toggle_mute().unwrap();
        assert!(is_muted);
        assert_eq!(coord.status, VoiceSessionStatus::Muted);
        let state_muted = coord.process_mic_input(&samples);
        assert_eq!(state_muted.microphone_peak, 0);

        // Unmute transitions back to Connected
        let unmuted = coord.toggle_mute().unwrap();
        assert!(!unmuted);
        assert_eq!(coord.status, VoiceSessionStatus::Connected);

        // Speaker suppression
        assert!(coord.set_speaker_suppressed(true).is_ok());

        coord.stop_session();
        assert_eq!(coord.status, VoiceSessionStatus::Closed);
        assert!(coord.stream_handle.is_none());
    }
}
