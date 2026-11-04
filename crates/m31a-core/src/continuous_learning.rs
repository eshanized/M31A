//! Continuous Quality Improvement & Trajectory Learning Engine (`m31a-core::continuous_learning`).
//!
//! Synthesizes verified preference trajectories for Direct Preference Optimization (DPO),
//! Reinforcement Learning from Human Feedback (RLHF), and dynamic few-shot exemplar learning:
//! - Records paired `(prompt, rejected_patch, accepted_patch)` trajectories.
//! - Validates that rejected patches are grounded in compiler/test failure logs.
//! - Validates that accepted patches are supported by verified evidence.
//! - Generates task-specific few-shot exemplars to prevent repeating past failures.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A DPO preference trajectory pair representing negative vs. positive engineering choices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DpoPreferencePair {
    pub prompt: String,
    pub rejected_patch: String,
    pub accepted_patch: String,
    pub rejection_reason: String,
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

    /// Creates, validates, sanitizes, and optionally persists a DPO preference trajectory pair.
    /// Rejects ungrounded trajectories that lack diagnostic failure logs or verified evidence.
    /// Redacts all detected API keys, credentials, and private keys from saved records.
    pub fn record_trajectory(
        &self,
        prompt: &str,
        rejected_patch: &str,
        accepted_patch: &str,
        rejection_reason: &str,
        verification_evidence: &str,
    ) -> Result<DpoPreferencePair> {
        let p_trimmed = prompt.trim();
        let rej_trimmed = rejected_patch.trim();
        let acc_trimmed = accepted_patch.trim();
        let reason_trimmed = rejection_reason.trim();
        let ev_trimmed = verification_evidence.trim();

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
        if ev_trimmed.is_empty() {
            bail!("Accepted patch must be grounded in verified test/compiler evidence");
        }

        let ev_lower = ev_trimmed.to_ascii_lowercase();
        if ev_lower == "none"
            || ev_lower == "n/a"
            || ev_lower == "unknown"
            || ev_lower == "unverified"
        {
            bail!("Verification evidence cannot be placeholder/stub value '{ev_trimmed}'");
        }

        // Apply strict secret redaction across all stored fields
        let clean_prompt = crate::sanitizer::redact_secrets(p_trimmed);
        let clean_rejected_patch = crate::sanitizer::redact_secrets(rej_trimmed);
        let clean_accepted_patch = crate::sanitizer::redact_secrets(acc_trimmed);
        let clean_rejection_reason = crate::sanitizer::redact_secrets(reason_trimmed);
        let clean_verification_evidence = crate::sanitizer::redact_secrets(ev_trimmed);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let pair = DpoPreferencePair {
            prompt: clean_prompt,
            rejected_patch: clean_rejected_patch,
            accepted_patch: clean_accepted_patch,
            rejection_reason: clean_rejection_reason,
            verification_evidence: clean_verification_evidence,
            timestamp_epoch_secs: now,
        };

        if let Some(ref dir) = self.storage_dir {
            let file_path = dir.join("dpo_trajectories.jsonl");
            if let Ok(serialized) = serde_json::to_string(&pair) {
                use std::io::Write;
                if let Ok(mut file) = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)
                {
                    let _ = writeln!(file, "{}", serialized);
                }
            }
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

        let pair = engine
            .record_trajectory(
                "Fix type error in auth",
                "- let x: u32 = str_val;",
                "+ let x: u32 = str_val.parse()?;\n+ assert!(x > 0);",
                "Mismatched types: expected u32, found String",
                "cargo test -p auth PASSED 10/10",
            )
            .unwrap();

        assert_eq!(pair.prompt, "Fix type error in auth");
        assert!(pair.accepted_patch.contains(".parse()?"));

        let jsonl = ContinuousLearningEngine::export_dpo_jsonl(std::slice::from_ref(&pair));
        assert!(jsonl.contains("Fix type error in auth"));
        assert!(jsonl.contains("Mismatched types"));

        let exemplar = engine.distill_exemplar(&pair, "TypeCorrection");
        assert_eq!(exemplar.task_category, "TypeCorrection");

        let exemplars = vec![exemplar];
        let retrieved = engine.retrieve_exemplars("auth type error", &exemplars, 5);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].task_category, "TypeCorrection");
    }

    #[test]
    fn test_record_trajectory_validation_and_secret_redaction() {
        let temp = tempfile::tempdir().unwrap();
        let engine = ContinuousLearningEngine::new(Some(temp.path().to_path_buf()));

        // 1. Ungrounded rejected patch (missing rejection diagnostic)
        let err1 = engine.record_trajectory(
            "Fix auth bug",
            "- let a = 1;",
            "+ let a = 2;",
            "",
            "cargo test passed",
        );
        assert!(err1.is_err());
        assert!(
            err1.unwrap_err()
                .to_string()
                .contains("diagnostic failure logs")
        );

        // 2. Ungrounded accepted patch (stub/placeholder evidence)
        let err2 = engine.record_trajectory(
            "Fix auth bug",
            "- let a = 1;",
            "+ let a = 2;",
            "panic at runtime",
            "none",
        );
        assert!(err2.is_err());
        assert!(err2.unwrap_err().to_string().contains("placeholder/stub"));

        // 3. Secret redaction on successful recording
        let secret_token = "Bearer sk-123456789012345678901234567890";
        let pair = engine
            .record_trajectory(
                &format!("Fix leak of {}", secret_token),
                "- let token = \"secret\";",
                "+ let token = \"[REDACTED]\";",
                "Leaked token in log",
                "cargo test verification passed 5/5",
            )
            .unwrap();

        assert!(!pair.prompt.contains("sk-123456789012345678901234567890"));
        assert!(pair.prompt.contains("[REDACTED_SECRET]"));
    }
}
