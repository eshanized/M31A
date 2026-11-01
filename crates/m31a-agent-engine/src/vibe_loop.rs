//! Continuous Vibe Loop & Autonomous Auto-Healing Engine (`m31a-agent-engine::vibe_loop`).
//!
//! Maintains a non-blocking background perception-action loop:
//! 1. Monitors workspace file modifications with debouncing via `m31a-watcher`.
//! 2. Runs instant non-intrusive compiler and typecheck diagnostics via `m31a-verify`.
//! 3. Synthesizes targeted corrective Codex patches via low-latency NVIDIA NIM models.
//! 4. Records transient Ghost Checkpoints on every verified green state (`VibeTimeline`),
//!    enabling zero-friction timeline rewinding (`/rewind`).
//! 5. Discovers companion test and interface files for predictive ergonomics.

use m31a_core::types::ChatMessage;
use m31a_provider::ProviderClient;
use m31a_tools::patch::{apply_patch_to_workspace, parse_patch};
use m31a_verify::AutonomousVerifier;
use m31a_watcher::{WatcherConfig, WorkspaceWatcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tracing::{info, warn};

fn default_true() -> bool {
    true
}

/// Configuration options for the continuous Vibe Loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeConfig {
    pub debounce_duration: Duration,
    pub auto_heal: bool,
    pub max_heal_attempts: usize,
    pub model_override: Option<String>,
    pub check_timeout: Duration,
    #[serde(default = "default_true")]
    pub enable_timeline: bool,
    #[serde(default = "default_true")]
    pub suggest_companions: bool,
}

impl Default for VibeConfig {
    fn default() -> Self {
        Self {
            debounce_duration: Duration::from_millis(300),
            auto_heal: true,
            max_heal_attempts: 3,
            model_override: None,
            check_timeout: Duration::from_secs(30),
            enable_timeline: true,
            suggest_companions: true,
        }
    }
}

/// A recorded milestone checkpoint in the Vibe timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeCheckpoint {
    pub id: String,
    pub commit_hash: Option<String>,
    pub description: String,
    pub timestamp: u64,
    pub files: Vec<PathBuf>,
    #[serde(skip)]
    pub file_snapshots: HashMap<PathBuf, String>,
}

/// In-memory timeline managing micro-checkpoints and rewind capabilities.
#[derive(Debug, Clone, Default)]
pub struct VibeTimeline {
    checkpoints: Vec<VibeCheckpoint>,
    max_checkpoints: usize,
}

impl VibeTimeline {
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            checkpoints: Vec::new(),
            max_checkpoints: if max_checkpoints == 0 {
                50
            } else {
                max_checkpoints
            },
        }
    }

    /// Captures a checkpoint of modified files.
    pub fn record_checkpoint(
        &mut self,
        root: &Path,
        description: impl Into<String>,
        paths: &[PathBuf],
    ) -> VibeCheckpoint {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let id = format!("ckpt-{}-{}", ts, uuid::Uuid::new_v4().simple());
        let mut snapshots = HashMap::new();

        for p in paths {
            let full = if p.is_absolute() {
                p.clone()
            } else {
                root.join(p)
            };
            if let Ok(content) = fs::read_to_string(&full) {
                snapshots.insert(p.clone(), content);
            }
        }

        // Try getting current Git commit hash if in a repo
        let commit_hash = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    String::from_utf8(out.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        let ckpt = VibeCheckpoint {
            id,
            commit_hash,
            description: description.into(),
            timestamp: ts,
            files: paths.to_vec(),
            file_snapshots: snapshots,
        };

        self.checkpoints.push(ckpt.clone());
        if self.checkpoints.len() > self.max_checkpoints {
            self.checkpoints.remove(0);
        }

        ckpt
    }

    /// Restores workspace files to the specified checkpoint state.
    pub fn rewind_to(&self, root: &Path, checkpoint_id: &str) -> anyhow::Result<Vec<PathBuf>> {
        let ckpt = self
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Checkpoint '{}' not found in timeline", checkpoint_id)
            })?;

        let mut restored = Vec::new();
        for (rel_path, content) in &ckpt.file_snapshots {
            let full_path = if rel_path.is_absolute() {
                rel_path.clone()
            } else {
                root.join(rel_path)
            };
            if let Some(parent) = full_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&full_path, content)?;
            restored.push(rel_path.clone());
        }

        Ok(restored)
    }

    pub fn checkpoints(&self) -> &[VibeCheckpoint] {
        &self.checkpoints
    }
}

/// Events emitted by the Vibe Loop to the TUI or terminal interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VibeEvent {
    WatcherStarted {
        root: PathBuf,
    },
    FileChanged {
        paths: Vec<PathBuf>,
    },
    CheckingWorkspace,
    WorkspaceClean {
        duration_ms: u64,
    },
    ErrorsDetected {
        error_count: usize,
        diagnostics: Vec<String>,
        self_healing_prompt: Option<String>,
    },
    SelfHealingStarted {
        attempt: usize,
        max_attempts: usize,
    },
    SelfHealingSuccess {
        duration_ms: u64,
        files_healed: Vec<PathBuf>,
    },
    SelfHealingFailed {
        reason: String,
    },
    CheckpointCreated {
        checkpoint_id: String,
        commit_hash: Option<String>,
        description: String,
        timestamp: u64,
        files: Vec<PathBuf>,
    },
    RewindCompleted {
        checkpoint_id: String,
        restored_files: Vec<PathBuf>,
    },
    CompanionSuggestion {
        source_file: PathBuf,
        suggested_companion: PathBuf,
        rationale: String,
    },
    VibeStopped,
}

pub struct VibeEngine {
    workspace_root: PathBuf,
    client: ProviderClient,
    config: VibeConfig,
    is_running: Arc<AtomicBool>,
    timeline: Arc<Mutex<VibeTimeline>>,
}

impl VibeEngine {
    pub fn new(
        workspace_root: PathBuf,
        client: impl Into<ProviderClient>,
        config: VibeConfig,
    ) -> Self {
        Self {
            workspace_root,
            client: client.into(),
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            timeline: Arc::new(Mutex::new(VibeTimeline::new(50))),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub fn timeline(&self) -> Arc<Mutex<VibeTimeline>> {
        Arc::clone(&self.timeline)
    }

    /// Rewinds the workspace to a previously recorded checkpoint.
    pub fn rewind(&self, checkpoint_id: &str) -> anyhow::Result<Vec<PathBuf>> {
        let tl = self.timeline.lock().unwrap();
        tl.rewind_to(&self.workspace_root, checkpoint_id)
    }

    /// Discovers potential companion files (e.g., test or implementation counterparts).
    pub fn find_companion(root: &Path, file_path: &Path) -> Option<(PathBuf, String)> {
        let file_name = file_path.file_name()?.to_string_lossy();
        let stem = file_path.file_stem()?.to_string_lossy();
        let ext = file_path.extension()?.to_string_lossy();

        if ext == "rs" {
            if !file_name.contains("test") {
                let test_name = format!("{}_test.rs", stem);
                let cand_tests = root.join("tests").join(&test_name);
                if cand_tests.exists() {
                    return Some((
                        cand_tests,
                        format!("Companion integration test for {}", file_name),
                    ));
                }
            }
        } else if (ext == "ts" || ext == "js")
            && !file_name.contains(".test.")
            && !file_name.contains(".spec.")
        {
            let cand_test = file_path.with_file_name(format!("{}.test.{}", stem, ext));
            if root.join(&cand_test).exists() {
                return Some((cand_test, format!("Companion test suite for {}", file_name)));
            }
        }

        None
    }

    /// Spawns the continuous background loop and returns an event receiver.
    pub fn start(&self) -> (mpsc::UnboundedReceiver<VibeEvent>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let running = Arc::clone(&self.is_running);
        running.store(true, Ordering::SeqCst);

        let ws_root = self.workspace_root.clone();
        let client = self.client.clone();
        let config = self.config.clone();
        let run_flag = Arc::clone(&running);
        let timeline_ref = Arc::clone(&self.timeline);

        tokio::spawn(async move {
            info!(root = ?ws_root, "Vibe Loop started");
            let _ = tx.send(VibeEvent::WatcherStarted {
                root: ws_root.clone(),
            });

            let watcher_cfg = WatcherConfig {
                debounce_duration: config.debounce_duration,
                ..Default::default()
            };
            let watcher = WorkspaceWatcher::new(ws_root.clone(), Some(watcher_cfg));
            let mut change_rx = watcher.subscribe();
            watcher.start();

            // Run an initial healthcheck
            let _ = tx.send(VibeEvent::CheckingWorkspace);
            let init_start = Instant::now();
            if let Ok(report) =
                AutonomousVerifier::verify_check(&ws_root, config.check_timeout).await
            {
                if report.success {
                    let _ = tx.send(VibeEvent::WorkspaceClean {
                        duration_ms: init_start.elapsed().as_millis() as u64,
                    });
                } else {
                    let diag_strings: Vec<String> = report
                        .diagnostics
                        .iter()
                        .map(|d| d.message.clone())
                        .collect();
                    let _ = tx.send(VibeEvent::ErrorsDetected {
                        error_count: diag_strings.len(),
                        diagnostics: diag_strings,
                        self_healing_prompt: report.self_healing_prompt.clone(),
                    });
                }
            }

            while run_flag.load(Ordering::SeqCst) {
                tokio::select! {
                    Ok(batch) = change_rx.recv() => {
                        let changed_paths: Vec<PathBuf> = batch.changes.iter().map(|e| e.path.clone()).collect();
                        if changed_paths.is_empty() {
                            continue;
                        }

                        // Filter out non-code or ephemeral files
                        let relevant_paths: Vec<PathBuf> = changed_paths
                            .into_iter()
                            .filter(|p| {
                                let s = p.to_string_lossy();
                                !s.contains(".git") && !s.contains("target/") && !s.contains("node_modules/")
                            })
                            .collect();

                        if relevant_paths.is_empty() {
                            continue;
                        }

                        let _ = tx.send(VibeEvent::FileChanged { paths: relevant_paths.clone() });

                        // Check for companion files if enabled
                        if config.suggest_companions {
                            for path in &relevant_paths {
                                if let Some((comp, rationale)) = Self::find_companion(&ws_root, path) {
                                    let _ = tx.send(VibeEvent::CompanionSuggestion {
                                        source_file: path.clone(),
                                        suggested_companion: comp,
                                        rationale,
                                    });
                                }
                            }
                        }

                        let _ = tx.send(VibeEvent::CheckingWorkspace);

                        let check_start = Instant::now();
                        match AutonomousVerifier::verify_check(&ws_root, config.check_timeout).await {
                            Ok(report) if report.success => {
                                let duration_ms = check_start.elapsed().as_millis() as u64;
                                let _ = tx.send(VibeEvent::WorkspaceClean { duration_ms });

                                // Capture ghost checkpoint on clean compile
                                if config.enable_timeline {
                                    let mut tl = timeline_ref.lock().unwrap();
                                    let ckpt = tl.record_checkpoint(
                                        &ws_root,
                                        "Workspace compile verified green",
                                        &relevant_paths,
                                    );
                                    let _ = tx.send(VibeEvent::CheckpointCreated {
                                        checkpoint_id: ckpt.id,
                                        commit_hash: ckpt.commit_hash,
                                        description: ckpt.description,
                                        timestamp: ckpt.timestamp,
                                        files: ckpt.files,
                                    });
                                }
                            }
                            Ok(report) => {
                                let diag_strings: Vec<String> = report.diagnostics.iter().map(|d| d.message.clone()).collect();
                                let _ = tx.send(VibeEvent::ErrorsDetected {
                                    error_count: diag_strings.len(),
                                    diagnostics: diag_strings,
                                    self_healing_prompt: report.self_healing_prompt.clone(),
                                });

                                if config.auto_heal && report.self_healing_prompt.is_some() {
                                    Self::attempt_auto_heal(
                                        &ws_root,
                                        &client,
                                        report.self_healing_prompt.as_deref().unwrap(),
                                        &config,
                                        &tx,
                                        &timeline_ref,
                                    ).await;
                                }
                            }
                            Err(e) => {
                                warn!("Error running fast check in Vibe Loop: {e}");
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(200)) => {
                        // Periodic heartbeat
                    }
                }
            }

            watcher.stop();
            let _ = tx.send(VibeEvent::VibeStopped);
            info!("Vibe Loop stopped");
        });

        (rx, running)
    }

    /// Attempts multi-iteration autonomous self-healing via model provider.
    async fn attempt_auto_heal(
        ws_root: &Path,
        client: &ProviderClient,
        prompt: &str,
        config: &VibeConfig,
        tx: &mpsc::UnboundedSender<VibeEvent>,
        timeline: &Arc<Mutex<VibeTimeline>>,
    ) {
        let mut current_prompt = prompt.to_string();

        for attempt in 1..=config.max_heal_attempts {
            let _ = tx.send(VibeEvent::SelfHealingStarted {
                attempt,
                max_attempts: config.max_heal_attempts,
            });

            let heal_start = Instant::now();
            let sys_msg = ChatMessage::system(
                "You are the M31A Autonomous Healing Engine. Fix the compiler and typecheck errors described. Respond ONLY with a standard OpenAI Codex multi-file patch (*** Begin Patch ... *** End Patch). Do not include conversational filler.",
            );
            let user_msg = ChatMessage::user(current_prompt.clone());

            let messages = vec![sys_msg, user_msg];
            let resp = client.stream_chat(&messages, None, None, None).await;

            match resp {
                Ok((resp_msg, _usage)) => {
                    if let Some(ref patch_text) = resp_msg.content
                        && patch_text.contains("*** Begin Patch")
                        && let Ok(parsed) = parse_patch(patch_text)
                    {
                        match apply_patch_to_workspace(ws_root, &parsed) {
                            Ok(apply_res) => {
                                let mut files_healed = apply_res.files_modified;
                                files_healed.extend(apply_res.files_created);

                                // Re-verify
                                if let Ok(re_report) =
                                    AutonomousVerifier::verify_check(ws_root, config.check_timeout)
                                        .await
                                {
                                    if re_report.success {
                                        let duration_ms = heal_start.elapsed().as_millis() as u64;
                                        let _ = tx.send(VibeEvent::SelfHealingSuccess {
                                            duration_ms,
                                            files_healed: files_healed.clone(),
                                        });

                                        // Capture successful self-heal checkpoint
                                        if config.enable_timeline {
                                            let mut tl = timeline.lock().unwrap();
                                            let ckpt = tl.record_checkpoint(
                                                ws_root,
                                                format!(
                                                    "Autonomous auto-heal succeeded (attempt {})",
                                                    attempt
                                                ),
                                                &files_healed,
                                            );
                                            let _ = tx.send(VibeEvent::CheckpointCreated {
                                                checkpoint_id: ckpt.id,
                                                commit_hash: ckpt.commit_hash,
                                                description: ckpt.description,
                                                timestamp: ckpt.timestamp,
                                                files: ckpt.files,
                                            });
                                        }
                                        return;
                                    } else if let Some(ref next_prompt) =
                                        re_report.self_healing_prompt
                                    {
                                        current_prompt = next_prompt.clone();
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Patch apply failed during auto-heal: {e}");
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("LLM completion failed during auto-heal: {e}");
                }
            }
        }

        let _ = tx.send(VibeEvent::SelfHealingFailed {
            reason: format!(
                "Exhausted {} self-healing attempts",
                config.max_heal_attempts
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vibe_timeline_record_and_rewind() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_a = temp_dir.path().join("src.rs");
        fs::write(&file_a, "fn original() {}").unwrap();

        let mut timeline = VibeTimeline::new(10);
        let ckpt = timeline.record_checkpoint(
            temp_dir.path(),
            "Initial state",
            &[PathBuf::from("src.rs")],
        );
        assert_eq!(timeline.checkpoints().len(), 1);
        assert!(ckpt.id.starts_with("ckpt-"));

        // Mutate file
        fs::write(&file_a, "fn broken() { syntax_error }").unwrap();

        // Rewind
        let restored = timeline.rewind_to(temp_dir.path(), &ckpt.id).unwrap();
        assert_eq!(restored, vec![PathBuf::from("src.rs")]);

        let after_rewind = fs::read_to_string(&file_a).unwrap();
        assert_eq!(after_rewind, "fn original() {}");
    }

    #[test]
    fn test_find_companion_test() {
        let temp_dir = tempfile::tempdir().unwrap();
        let tests_dir = temp_dir.path().join("tests");
        fs::create_dir_all(&tests_dir).unwrap();
        let test_file = tests_dir.join("worker_test.rs");
        fs::write(&test_file, "// test").unwrap();

        let worker_src = PathBuf::from("src/worker.rs");
        let companion = VibeEngine::find_companion(temp_dir.path(), &worker_src);
        assert!(companion.is_some());
        let (path, rationale) = companion.unwrap();
        assert!(path.ends_with("worker_test.rs"));
        assert!(rationale.contains("worker.rs"));
    }
}
