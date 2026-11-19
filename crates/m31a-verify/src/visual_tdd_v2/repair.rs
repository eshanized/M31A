//! Bounded Layout Repair and Self-Healing Engine.
//!
//! Provides bounded iterative layout repair for frontends encountering visual regressions.
//! Uses checkpointing, maximum retry enforcement, and strict approval gating
//! to prevent runaway mutation loops or broken styles.

use super::comparator::ComparisonOutcome;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Checkpoint preserving file state before layout repair attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairCheckpoint {
    pub file_path: PathBuf,
    pub original_content: String,
    pub created_at: String,
}

/// A proposed layout repair candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutRepairPatch {
    pub target_file: PathBuf,
    pub proposed_patch: String,
    pub rationale: String,
}

/// Result of an automated repair cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAttemptResult {
    pub attempt_number: usize,
    pub patch_applied: bool,
    pub visual_mismatch_before: f64,
    pub visual_mismatch_after: Option<f64>,
    pub outcome_description: String,
}

/// Overall layout repair report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutRepairReport {
    pub scenario_name: String,
    pub attempts: Vec<RepairAttemptResult>,
    pub max_attempts_reached: bool,
    pub success: bool,
    pub rolled_back: bool,
    pub final_guidance: String,
}

/// Configuration for the bounded layout repair loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairLoopConfig {
    pub max_attempts: usize,
    pub rollback_on_failure: bool,
    pub require_approval: bool,
}

impl Default for RepairLoopConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            rollback_on_failure: true,
            require_approval: true,
        }
    }
}

pub struct LayoutRepairCoordinator {
    config: RepairLoopConfig,
    checkpoints: Vec<RepairCheckpoint>,
}

impl LayoutRepairCoordinator {
    pub fn new(config: RepairLoopConfig) -> Self {
        Self {
            config,
            checkpoints: Vec::new(),
        }
    }

    /// Captures a pre-mutation checkpoint of a file.
    pub fn create_checkpoint(&mut self, file_path: &Path) -> Result<()> {
        if file_path.exists() {
            let content = std::fs::read_to_string(file_path)?;
            self.checkpoints.push(RepairCheckpoint {
                file_path: file_path.to_path_buf(),
                original_content: content,
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }
        Ok(())
    }

    /// Rolls back all modified files to their pre-repair checkpoint state.
    pub fn rollback_all(&self) -> Result<()> {
        for cp in self.checkpoints.iter().rev() {
            if let Some(parent) = cp.file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&cp.file_path, &cp.original_content)?;
        }
        Ok(())
    }

    /// Generates structured diagnostic prompt guidance for the agent/model to fix layout regressions.
    pub fn generate_repair_prompt(
        scenario_name: &str,
        viewport_id: &str,
        outcome: &ComparisonOutcome,
        dom_context: Option<&str>,
    ) -> String {
        let mut prompt = format!(
            "### Visual Layout Regression Detected in Scenario: '{}' [Viewport: {}]\n\n",
            scenario_name, viewport_id
        );

        prompt.push_str(&format!(
            "- **Mismatch Ratio:** {:.3}%\n- **Mismatched Pixels:** {}\n- **Max Color Delta ΔC:** {:.3}\n",
            outcome.mismatch_ratio * 100.0,
            outcome.mismatched_pixels,
            outcome.max_color_distance
        ));

        if !outcome.bounding_boxes.is_empty() {
            prompt.push_str("- **Difference Bounding Boxes:**\n");
            for (idx, b) in outcome.bounding_boxes.iter().enumerate() {
                prompt.push_str(&format!(
                    "  {}. Coordinates: [x: {}..{}, y: {}..{}] (dimensions: {}x{})\n",
                    idx + 1,
                    b.min_x,
                    b.max_x,
                    b.min_y,
                    b.max_y,
                    b.width(),
                    b.height()
                ));
            }
        }

        if let Some(ctx) = dom_context {
            prompt.push_str(&format!(
                "\n**DOM Elements in Affected Region:**\n```html\n{}\n```\n",
                ctx
            ));
        }

        prompt.push_str("\nPlease generate a minimal CSS/HTML patch that realigns elements without modifying unrelated layout rules.");
        prompt
    }

    /// Enforces the maximum attempt limit before executing a repair iteration.
    pub fn assert_can_attempt(&self, current_attempt: usize) -> Result<()> {
        if current_attempt >= self.config.max_attempts {
            bail!(
                "Bounded layout repair loop exceeded maximum allowed attempts ({}); aborting to prevent runaway mutations",
                self.config.max_attempts
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_checkpoint_and_rollback() {
        let dir = tempdir().unwrap();
        let test_file = dir.path().join("style.css");
        std::fs::write(&test_file, ".button { color: red; }").unwrap();

        let mut coordinator = LayoutRepairCoordinator::new(RepairLoopConfig::default());
        coordinator.create_checkpoint(&test_file).unwrap();

        // Mutate file
        std::fs::write(&test_file, ".button { color: broken; }").unwrap();
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            ".button { color: broken; }"
        );

        // Rollback
        coordinator.rollback_all().unwrap();
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            ".button { color: red; }"
        );
    }

    #[test]
    fn test_assert_max_attempts() {
        let coordinator = LayoutRepairCoordinator::new(RepairLoopConfig {
            max_attempts: 2,
            rollback_on_failure: true,
            require_approval: true,
        });

        assert!(coordinator.assert_can_attempt(0).is_ok());
        assert!(coordinator.assert_can_attempt(1).is_ok());
        assert!(coordinator.assert_can_attempt(2).is_err());
    }
}
