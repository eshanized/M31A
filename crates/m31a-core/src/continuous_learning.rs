//! Continuous Quality Improvement & Trajectory Learning Engine (`m31a-core::continuous_learning`).
//!
//! Synthesizes verified preference trajectories for Direct Preference Optimization (DPO),
//! Reinforcement Learning from Human Feedback (RLHF), and dynamic few-shot exemplar learning:
//! - Records paired `(prompt, rejected_patch, accepted_patch)` trajectories.
//! - Validates that rejected patches are grounded in compiler/test failure logs.
//! - Validates that accepted patches are supported by verified evidence.
//! - Generates task-specific few-shot exemplars to prevent repeating past failures.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

static CONTINUOUS_LEARNING_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Advisory cross-process file lock guard for continuous learning persistence.
struct FileLock {
    #[cfg(unix)]
    fd: std::os::unix::io::RawFd,
    _file: fs::File,
}

impl FileLock {
    fn acquire(lock_path: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .with_context(|| format!("Failed to open lock file at {:?}", lock_path))?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let ret = unsafe { libc::flock(fd, libc::LOCK_EX) };
            if ret != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to acquire file lock: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(Self { fd, _file: file })
        }

        #[cfg(not(unix))]
        {
            Ok(Self { _file: file })
        }
    }
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.fd, libc::LOCK_UN);
        }
    }
}

/// Structured record of verified test / compiler execution results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationRecord {
    pub command: String,
    pub exit_code: i32,
    pub passed_tests: u32,
    pub failed_tests: u32,
    pub workspace_digest: String,
    pub output_digest: String,
}

/// A DPO preference trajectory pair representing negative vs. positive engineering choices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DpoPreferencePair {
    pub prompt: String,
    pub rejected_patch: String,
    pub accepted_patch: String,
    pub rejection_reason: String,
    pub verification: VerificationRecord,
    #[serde(default)]
    pub verification_evidence: String,
    pub timestamp_epoch_secs: u64,
}

/// A distilled few-shot exemplar dynamically retrievable during similar tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FewShotExemplar {
    pub task_category: String,
    pub problem_description: String,
    pub failed_attempt_pattern: String,
    pub successful_solution: String,
    pub verification_notes: String,
}

/// Persistent engine for continuous learning from verified engineering turns.
#[derive(Debug, Clone, Default)]
pub struct ContinuousLearningEngine {
    storage_dir: Option<PathBuf>,
}

impl ContinuousLearningEngine {
    pub fn new(storage_dir: Option<PathBuf>) -> Self {
        if let Some(ref dir) = storage_dir {
            let _ = fs::create_dir_all(dir);
        }
        Self { storage_dir }
    }

    /// Creates, validates, sanitizes, and persists a DPO preference trajectory pair with file locking.
    /// Rejects ungrounded trajectories that lack diagnostic failure logs or passing verification records.
    /// Redacts all detected API keys, credentials, and private keys from saved records.
    pub fn record_trajectory(
        &self,
        prompt: &str,
        rejected_patch: &str,
        accepted_patch: &str,
        rejection_reason: &str,
        verification: VerificationRecord,
    ) -> Result<DpoPreferencePair> {
        let p_trimmed = prompt.trim();
        let rej_trimmed = rejected_patch.trim();
        let acc_trimmed = accepted_patch.trim();
        let reason_trimmed = rejection_reason.trim();

        if p_trimmed.is_empty() {
            bail!("Continuous learning prompt cannot be empty");
        }
        if rej_trimmed.is_empty() {
            bail!("Rejected patch cannot be empty");
        }
        if acc_trimmed.is_empty() {
            bail!("Accepted patch cannot be empty");
        }
        if reason_trimmed.is_empty() {
            bail!(
                "Rejected patch must be grounded in diagnostic failure logs or compiler/test error output"
            );
        }

        // Enforce genuine passing verification requirements
        if verification.command.trim().is_empty() {
            bail!("Verification command cannot be empty");
        }
        if verification.exit_code != 0 {
            bail!(
                "Verification failed: exit code must be 0, got {}",
                verification.exit_code
            );
        }
        if verification.failed_tests != 0 {
            bail!(
                "Verification failed: failed tests must be 0, got {}",
                verification.failed_tests
            );
        }

        // Apply strict secret redaction across all stored fields
        let clean_prompt = crate::sanitizer::redact_secrets(p_trimmed);
        let clean_rejected_patch = crate::sanitizer::redact_secrets(rej_trimmed);
        let clean_accepted_patch = crate::sanitizer::redact_secrets(acc_trimmed);
        let clean_rejection_reason = crate::sanitizer::redact_secrets(reason_trimmed);

        let clean_verification = VerificationRecord {
            command: crate::sanitizer::redact_secrets(verification.command.trim()),
            exit_code: verification.exit_code,
            passed_tests: verification.passed_tests,
            failed_tests: verification.failed_tests,
            workspace_digest: crate::sanitizer::redact_secrets(
                verification.workspace_digest.trim(),
            ),
            output_digest: crate::sanitizer::redact_secrets(verification.output_digest.trim()),
        };

        let evidence_summary = format!(
            "Command: `{}` (exit code {}, passed: {}, failed: {})",
            clean_verification.command,
            clean_verification.exit_code,
            clean_verification.passed_tests,
            clean_verification.failed_tests
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let pair = DpoPreferencePair {
            prompt: clean_prompt,
            rejected_patch: clean_rejected_patch,
            accepted_patch: clean_accepted_patch,
            rejection_reason: clean_rejection_reason,
            verification: clean_verification,
            verification_evidence: evidence_summary,
            timestamp_epoch_secs: now,
        };

        if let Some(ref dir) = self.storage_dir {
            let _guard = CONTINUOUS_LEARNING_LOCK.lock().unwrap();
            let lock_path = dir.join(".dpo_trajectories.lock");
            let _file_lock = FileLock::acquire(&lock_path)?;

            let file_path = dir.join("dpo_trajectories.jsonl");
            let serialized = serde_json::to_string(&pair)
                .context("Failed to serialize DPO preference pair to JSON")?;

            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .with_context(|| format!("Failed to open trajectories file at {:?}", file_path))?;

            writeln!(file, "{}", serialized)
                .with_context(|| format!("Failed to write trajectory to {:?}", file_path))?;
            file.sync_all()
                .with_context(|| format!("Failed to sync trajectory file at {:?}", file_path))?;
        }

        Ok(pair)
    }

    /// Distills a DPO trajectory pair into a few-shot exemplar for contextual guidance.
    pub fn distill_exemplar(
        &self,
        pair: &DpoPreferencePair,
        task_category: &str,
    ) -> FewShotExemplar {
        FewShotExemplar {
            task_category: task_category.to_string(),
            problem_description: pair.prompt.chars().take(200).collect(),
            failed_attempt_pattern: pair.rejection_reason.clone(),
            successful_solution: pair.accepted_patch.clone(),
            verification_notes: pair.verification_evidence.clone(),
        }
    }

    /// Exports an array of DPO pairs into standard JSONL format for model fine-tuning.
    pub fn export_dpo_jsonl(pairs: &[DpoPreferencePair]) -> String {
        pairs
            .iter()
            .filter_map(|p| serde_json::to_string(p).ok())
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// Retrieves relevant exemplars based on keyword overlap in problem descriptions.
    pub fn retrieve_exemplars(
        &self,
        query: &str,
        exemplars: &[FewShotExemplar],
        limit: usize,
    ) -> Vec<FewShotExemplar> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        let mut scored: Vec<(&FewShotExemplar, usize)> = exemplars
            .iter()
            .map(|ex| {
                let text =
                    format!("{} {}", ex.task_category, ex.problem_description).to_lowercase();
                let score = query_words.iter().filter(|&&w| text.contains(w)).count();
                (ex, score)
            })
            .collect();

        scored.sort_by_key(|a| std::cmp::Reverse(a.1));
        scored
            .into_iter()
            .take(limit)
            .map(|(ex, _)| ex.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_trajectory_and_jsonl_export() {
        let temp = tempfile::tempdir().unwrap();
        let engine = ContinuousLearningEngine::new(Some(temp.path().to_path_buf()));

        let verification = VerificationRecord {
            command: "cargo test -p auth".to_string(),
            exit_code: 0,
            passed_tests: 10,
            failed_tests: 0,
            workspace_digest: "sha256:workspace123".to_string(),
            output_digest: "sha256:output123".to_string(),
        };

        let pair = engine
            .record_trajectory(
                "Fix type error in auth",
                "- let x: u32 = str_val;",
                "+ let x: u32 = str_val.parse()?;\n+ assert!(x > 0);",
                "Mismatched types: expected u32, found String",
                verification,
            )
            .unwrap();

        assert_eq!(pair.prompt, "Fix type error in auth");
        assert!(pair.accepted_patch.contains(".parse()?"));
        assert_eq!(pair.verification.passed_tests, 10);
        assert_eq!(pair.verification.exit_code, 0);

        let jsonl = ContinuousLearningEngine::export_dpo_jsonl(std::slice::from_ref(&pair));
        assert!(jsonl.contains("Fix type error in auth"));
        assert!(jsonl.contains("Mismatched types"));
        assert!(jsonl.contains("workspace_digest"));

        let exemplar = engine.distill_exemplar(&pair, "TypeCorrection");
        assert_eq!(exemplar.task_category, "TypeCorrection");
        assert!(exemplar.verification_notes.contains("cargo test -p auth"));

        let exemplars = vec![exemplar];
        let retrieved = engine.retrieve_exemplars("auth type error", &exemplars, 5);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].task_category, "TypeCorrection");
    }

    #[test]
    fn test_record_trajectory_validation_and_secret_redaction() {
        let temp = tempfile::tempdir().unwrap();
        let engine = ContinuousLearningEngine::new(Some(temp.path().to_path_buf()));

        let valid_rec = VerificationRecord {
            command: "cargo test".to_string(),
            exit_code: 0,
            passed_tests: 5,
            failed_tests: 0,
            workspace_digest: "digest1".to_string(),
            output_digest: "digest2".to_string(),
        };

        // 1. Ungrounded rejected patch (missing rejection diagnostic)
        let err1 = engine.record_trajectory(
            "Fix auth bug",
            "- let a = 1;",
            "+ let a = 2;",
            "",
            valid_rec.clone(),
        );
        assert!(err1.is_err());
        assert!(
            err1.unwrap_err()
                .to_string()
                .contains("diagnostic failure logs")
        );

        // 2. Non-zero exit code fails verification
        let mut non_zero_rec = valid_rec.clone();
        non_zero_rec.exit_code = 1;
        let err2 = engine.record_trajectory(
            "Fix auth bug",
            "- let a = 1;",
            "+ let a = 2;",
            "panic at runtime",
            non_zero_rec,
        );
        assert!(err2.is_err());
        assert!(
            err2.unwrap_err()
                .to_string()
                .contains("exit code must be 0")
        );

        // 3. Failed tests > 0 fails verification
        let mut failed_tests_rec = valid_rec.clone();
        failed_tests_rec.failed_tests = 3;
        let err3 = engine.record_trajectory(
            "Fix auth bug",
            "- let a = 1;",
            "+ let a = 2;",
            "test failure",
            failed_tests_rec,
        );
        assert!(err3.is_err());
        assert!(
            err3.unwrap_err()
                .to_string()
                .contains("failed tests must be 0")
        );

        // 4. Empty command fails verification
        let mut empty_cmd_rec = valid_rec.clone();
        empty_cmd_rec.command = "   ".to_string();
        let err4 = engine.record_trajectory(
            "Fix auth bug",
            "- let a = 1;",
            "+ let a = 2;",
            "command failure",
            empty_cmd_rec,
        );
        assert!(err4.is_err());
        assert!(
            err4.unwrap_err()
                .to_string()
                .contains("Verification command cannot be empty")
        );

        // 5. Secret redaction on successful recording
        let secret_token = "Bearer sk-123456789012345678901234567890";
        let pair = engine
            .record_trajectory(
                &format!("Fix leak of {}", secret_token),
                "- let token = \"secret\";",
                "+ let token = \"[REDACTED]\";",
                "Leaked token in log",
                VerificationRecord {
                    command: format!("cargo test --token {}", secret_token),
                    exit_code: 0,
                    passed_tests: 5,
                    failed_tests: 0,
                    workspace_digest: "digest".to_string(),
                    output_digest: format!("Token {}", secret_token),
                },
            )
            .unwrap();

        assert!(!pair.prompt.contains("sk-123456789012345678901234567890"));
        assert!(pair.prompt.contains("[REDACTED_SECRET]"));
        assert!(
            !pair
                .verification
                .command
                .contains("sk-123456789012345678901234567890")
        );
        assert!(pair.verification.command.contains("[REDACTED_SECRET]"));
        assert!(
            !pair
                .verification
                .output_digest
                .contains("sk-123456789012345678901234567890")
        );
        assert!(
            pair.verification
                .output_digest
                .contains("[REDACTED_SECRET]")
        );
    }
}
