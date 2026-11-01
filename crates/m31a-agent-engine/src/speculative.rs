//! Speculative Multi-Branch Parallel Reality Forks (`m31a-agent-engine::speculative`).
//!
//! Spawns parallel candidate branches in isolated sandboxed workspaces (using Git worktrees with
//! shared build-cache symlinks or fast Copy-on-Write fallback), evaluates build validity,
//! test integrity, performance benchmarks, and blast-radius safety, and promotes the
//! highest-scoring candidate to the main working tree.

use anyhow::{Result, anyhow};
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_verify::{AutonomousVerifier, CodebaseSafetyGate};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Configuration for an individual speculative candidate branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeBranchConfig {
    pub branch_id: String,
    pub name: String,
    pub hypothesis_prompt: String,
    pub model_override: Option<String>,
    #[serde(default)]
    pub patch_content: Option<String>,
}

/// Comprehensive outcome metrics for a speculative candidate branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEvaluation {
    pub branch_id: String,
    pub name: String,
    pub hypothesis_prompt: String,
    pub build_passed: bool,
    pub tests_passed: bool,
    pub execution_duration_ms: u64,
    pub test_duration_ms: u64,
    pub safety_verdict: String,
    pub blast_radius_score: f64,
    pub modified_files: Vec<String>,
    pub composite_score: f64,
    pub patch_content: String,
    pub summary: String,
}

/// Full report of a speculative competition across multiple reality branches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeExecutionReport {
    pub mission: String,
    pub total_candidates: usize,
    pub evaluations: Vec<CandidateEvaluation>,
    pub winning_branch_id: Option<String>,
    pub winning_branch_name: Option<String>,
    pub promotion_status: String,
    pub decision_rationale: String,
}

/// RAII Guard managing sandbox lifecycle across either Git worktrees or temporary directories.
pub struct SpeculativeSandboxGuard {
    pub path: PathBuf,
    pub branch_id: String,
    pub is_worktree: bool,
    source_root: PathBuf,
}

impl Drop for SpeculativeSandboxGuard {
    fn drop(&mut self) {
        if self.is_worktree {
            let settings = WorktreeSettings::new(&self.source_root);
            let mgr = WorktreeManager::new(settings);
            if let Err(e) = mgr.cleanup_worktree(&self.branch_id) {
                warn!(
                    branch_id = %self.branch_id,
                    err = %e,
                    "Failed to clean up speculative worktree"
                );
            }
        } else {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub struct SpeculativeArena;

impl SpeculativeArena {
    /// Generates speculative branch configurations from an array of human-readable approaches.
    pub fn generate_hypotheses(mission: &str, approaches: &[&str]) -> Vec<SpeculativeBranchConfig> {
        approaches
            .iter()
            .enumerate()
            .map(|(i, app)| {
                let branch_id = format!("hypo-{}-{}", i + 1, uuid::Uuid::new_v4().simple());
                SpeculativeBranchConfig {
                    branch_id,
                    name: format!("Approach {}", (b'A' + i as u8) as char),
                    hypothesis_prompt: format!("Mission: {}. Approach: {}", mission, app),
                    model_override: None,
                    patch_content: None,
                }
            })
            .collect()
    }

    /// Prepares an isolated sandbox workspace, preferentially using Git worktrees with
    /// shared target caches, or falling back to fast filtered directory copies.
    pub fn prepare_sandbox(source_root: &Path, branch_id: &str) -> Result<SpeculativeSandboxGuard> {
        // 1. Attempt Git Worktree isolation if repository is discovered
        if source_root.ancestors().any(|a| a.join(".git").exists()) {
            let settings = WorktreeSettings::new(source_root);
            let mgr = WorktreeManager::new(settings);
            if let Ok(managed) = mgr.create_worktree(branch_id, None) {
                // Link shared target build cache if exists to avoid cold full compiles
                let target_src = source_root.join("target");
                let target_dst = managed.worktree_path.join("target");
                if target_src.is_dir() && !target_dst.exists() {
                    #[cfg(unix)]
                    let _ = std::os::unix::fs::symlink(&target_src, &target_dst);
                }

                return Ok(SpeculativeSandboxGuard {
                    path: managed.worktree_path,
                    branch_id: branch_id.to_string(),
                    is_worktree: true,
                    source_root: source_root.to_path_buf(),
                });
            }
        }

        // 2. Fallback: Fast filtered directory copy to ephemeral tempdir
        let temp_dir = std::env::temp_dir().join(format!(
            "m31a-speculative-{}-{}",
            branch_id,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&temp_dir)?;
        Self::copy_dir_recursive(source_root, &temp_dir, 0, 5)?;

        Ok(SpeculativeSandboxGuard {
            path: temp_dir,
            branch_id: branch_id.to_string(),
            is_worktree: false,
            source_root: source_root.to_path_buf(),
        })
    }

    fn copy_dir_recursive(src: &Path, dst: &Path, depth: usize, max_depth: usize) -> Result<()> {
        if depth > max_depth {
            return Ok(());
        }
        if let Ok(entries) = fs::read_dir(src) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();

                if name.starts_with('.')
                    || matches!(
                        name.as_ref(),
                        "target" | "node_modules" | "dist" | ".git" | "vendor"
                    )
                {
                    continue;
                }

                let target = dst.join(&file_name);
                if path.is_dir() {
                    let _ = fs::create_dir_all(&target);
                    let _ = Self::copy_dir_recursive(&path, &target, depth + 1, max_depth);
                } else if path.is_file() {
                    let _ = fs::copy(&path, &target);
                }
            }
        }
        Ok(())
    }

    /// Evaluates candidate branches concurrently and selects the optimal solution.
    pub async fn compete(
        workspace_root: &Path,
        mission: &str,
        candidates: Vec<SpeculativeBranchConfig>,
        timeout_per_candidate: Duration,
        auto_promote_winner: bool,
    ) -> Result<SpeculativeExecutionReport> {
        info!(
            mission = %mission,
            candidates_count = candidates.len(),
            "Starting Speculative Parallel Reality Arena"
        );

        if candidates.is_empty() {
            return Err(anyhow!(
                "No candidate branches provided to SpeculativeArena"
            ));
        }

        let mut join_set = tokio::task::JoinSet::new();

        for candidate in candidates {
            let ws_root = workspace_root.to_path_buf();
            join_set.spawn(async move {
                Self::evaluate_candidate(&ws_root, candidate, timeout_per_candidate).await
            });
        }

        let mut evaluations = Vec::new();
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(eval)) => evaluations.push(eval),
                Ok(Err(e)) => warn!("Candidate evaluation error: {e}"),
                Err(join_err) => warn!("Candidate task panicked: {join_err}"),
            }
        }

        // Rank candidates by composite score descending
        evaluations.sort_by(|a, b| {
            b.composite_score
                .partial_cmp(&a.composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let winning_candidate = evaluations.first().cloned();
        let mut promotion_status = "No promotion requested".to_string();

        if let Some(ref winner) = winning_candidate {
            if winner.build_passed && winner.composite_score > 40.0 {
                if auto_promote_winner && !winner.patch_content.is_empty() {
                    // Apply winning patch to active workspace
                    match m31a_tools::patch::parse_patch(&winner.patch_content) {
                        Ok(parsed) => {
                            match m31a_tools::patch::apply_patch_to_workspace(
                                workspace_root,
                                &parsed,
                            ) {
                                Ok(res) => {
                                    let total_healed =
                                        res.files_modified.len() + res.files_created.len();
                                    promotion_status = format!(
                                        "Promoted winning candidate '{}' (applied patch to {} files)",
                                        winner.name, total_healed
                                    );
                                    info!(status = %promotion_status, "Speculative winner promoted to workspace");
                                }
                                Err(e) => {
                                    promotion_status =
                                        format!("Failed to apply winning patch: {e}");
                                    warn!(err = %e, "Patch promotion failed");
                                }
                            }
                        }
                        Err(e) => {
                            promotion_status = format!("Failed to parse winning patch: {e}");
                        }
                    }
                } else {
                    promotion_status = format!(
                        "Winning candidate identified: '{}' (score: {:.1})",
                        winner.name, winner.composite_score
                    );
                }
            } else {
                promotion_status =
                    "No candidate achieved passing verification threshold".to_string();
            }
        }

        let rationale = if let Some(ref winner) = winning_candidate {
            format!(
                "Candidate '{}' selected with composite score {:.1} (Build: {}, Tests: {}, Safety: {}, Blast Radius: {:.1})",
                winner.name,
                winner.composite_score,
                winner.build_passed,
                winner.tests_passed,
                winner.safety_verdict,
                winner.blast_radius_score
            )
        } else {
            "No candidates succeeded".to_string()
        };

        Ok(SpeculativeExecutionReport {
            mission: mission.to_string(),
            total_candidates: evaluations.len(),
            evaluations: evaluations.clone(),
            winning_branch_id: winning_candidate.as_ref().map(|w| w.branch_id.clone()),
            winning_branch_name: winning_candidate.as_ref().map(|w| w.name.clone()),
            promotion_status,
            decision_rationale: rationale,
        })
    }

    /// Evaluates a single candidate branch in an isolated sandbox workspace.
    async fn evaluate_candidate(
        workspace_root: &Path,
        candidate: SpeculativeBranchConfig,
        timeout: Duration,
    ) -> Result<CandidateEvaluation> {
        let start = Instant::now();
        let sandbox = Self::prepare_sandbox(workspace_root, &candidate.branch_id)?;

        let mut modified_files = Vec::new();
        let patch_content = candidate.patch_content.clone().unwrap_or_default();

        // If candidate supplies a patch, apply it into the isolated sandbox
        if !patch_content.is_empty()
            && let Ok(parsed) = m31a_tools::patch::parse_patch(&patch_content)
            && let Ok(res) = m31a_tools::patch::apply_patch_to_workspace(&sandbox.path, &parsed)
        {
            modified_files.extend(
                res.files_modified
                    .into_iter()
                    .chain(res.files_created)
                    .map(|p| p.to_string_lossy().to_string()),
            );
        }

        // Run fast compiler / typecheck
        let check_res = AutonomousVerifier::verify_check(&sandbox.path, timeout).await?;
        let build_passed = check_res.success;

        // Run tests if build succeeds
        let test_start = Instant::now();
        let test_res = if build_passed {
            AutonomousVerifier::verify_tests(&sandbox.path, timeout)
                .await
                .ok()
        } else {
            None
        };
        let test_duration_ms = test_start.elapsed().as_millis() as u64;
        let tests_passed = test_res.as_ref().map(|t| t.success).unwrap_or(false);

        // Run Safety Audit on modified files
        let mod_paths: Vec<PathBuf> = modified_files.iter().map(PathBuf::from).collect();
        let safety_audit =
            CodebaseSafetyGate::audit_workspace_changes(&sandbox.path, &mod_paths, &[])?;

        // Compute Composite Score (0.0 to 100.0)
        let mut score: f64 = 0.0;
        if build_passed {
            score += 40.0;
        }
        if tests_passed {
            score += 35.0;
        }
        // Reward low blast radius
        let safety_bonus =
            (100.0 - safety_audit.blast_radius.blast_radius_score).clamp(0.0, 100.0) * 0.15;
        score += safety_bonus;

        // Fast execution bonus (up to 10 points)
        let exec_ms = start.elapsed().as_millis() as u64;
        if exec_ms < 5000 {
            score += 10.0;
        } else if exec_ms < 15000 {
            score += 5.0;
        }

        let summary = format!(
            "Candidate '{}' score: {:.1} (Build: {}, Tests: {}, Safety: {:?})",
            candidate.name, score, build_passed, tests_passed, safety_audit.verdict
        );

        // Explicit drop will clean up worktree or temp directory
        drop(sandbox);

        Ok(CandidateEvaluation {
            branch_id: candidate.branch_id,
            name: candidate.name,
            hypothesis_prompt: candidate.hypothesis_prompt,
            build_passed,
            tests_passed,
            execution_duration_ms: exec_ms,
            test_duration_ms,
            safety_verdict: format!("{:?}", safety_audit.verdict),
            blast_radius_score: safety_audit.blast_radius.blast_radius_score,
            modified_files,
            composite_score: score,
            patch_content,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_speculative_arena_competition() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"[package]
name = "speculative_test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("lib.rs"),
            r#"
            pub fn add(a: i32, b: i32) -> i32 {
                a + b
            }
            "#,
        )
        .unwrap();

        let candidates = vec![
            SpeculativeBranchConfig {
                branch_id: "branch-a".to_string(),
                name: "Approach A".to_string(),
                hypothesis_prompt: "Standard addition".to_string(),
                model_override: None,
                patch_content: None,
            },
            SpeculativeBranchConfig {
                branch_id: "branch-b".to_string(),
                name: "Approach B".to_string(),
                hypothesis_prompt: "Bitwise addition".to_string(),
                model_override: None,
                patch_content: None,
            },
        ];

        let report = SpeculativeArena::compete(
            temp_dir.path(),
            "Optimize math helper",
            candidates,
            Duration::from_secs(10),
            false,
        )
        .await
        .unwrap();

        assert_eq!(report.total_candidates, 2);
        assert!(report.winning_branch_id.is_some());
        assert!(!report.decision_rationale.is_empty());
    }

    #[test]
    fn test_generate_hypotheses_helper() {
        let hypotheses = SpeculativeArena::generate_hypotheses(
            "Fix parser performance",
            &["Use SIMD parser", "Use lazy string references"],
        );
        assert_eq!(hypotheses.len(), 2);
        assert_eq!(hypotheses[0].name, "Approach A");
        assert_eq!(hypotheses[1].name, "Approach B");
        assert!(hypotheses[0].branch_id.starts_with("hypo-1-"));
    }
}
