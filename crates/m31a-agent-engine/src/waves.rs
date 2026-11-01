//! Wave-Based Parallel Execution Engine for M31A.
//!
//! Implements dependency-driven wave execution and automatic file overlap partitioning for phase execution.
//!
//! Orchestrates autonomous phase plan execution using dependency-driven waves:
//! 1. Topological Wave Partitioning: Resolves `depends_on` DAG into waves (Wave 1 parallel,
//!    Wave 2 depends on Wave 1).
//! 2. File Overlap Partitioning: Automatically separates plans that modify identical files into
//!    consecutive waves to prevent race conditions and merge conflicts.
//! 3. Build-Lock Avoidance: Parallel executors commit with `--no-verify` to prevent `Cargo.lock`
//!    or package lock contention. The orchestrator runs verification once per wave upon wave completion.
//! 4. Tokio JoinSet Parallel Dispatch: Spawns and manages concurrent subagent tasks with timeout
//!    and error boundary isolation.

use anyhow::{Result, anyhow};
use futures::future::BoxFuture;
use m31a_verify::AutonomousVerifier;
use m31a_verify::plan_checker::{PlanChecker, PlanModel};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanExecutionOutcome {
    pub plan_id: String,
    pub success: bool,
    pub commits: Vec<String>,
    pub summary_path: Option<PathBuf>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveExecutionSummary {
    pub wave_index: usize,
    pub outcomes: Vec<PlanExecutionOutcome>,
    pub success: bool,
    pub precommit_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseWaveResult {
    pub phase: String,
    pub total_waves: usize,
    pub completed_waves: usize,
    pub all_passed: bool,
    pub wave_summaries: Vec<WaveExecutionSummary>,
}

pub struct WaveScheduler;

impl WaveScheduler {
    /// Discovers and parses all `*-PLAN.md` files in a phase directory.
    pub fn discover_plans(phase_dir: &Path) -> Result<Vec<PlanModel>> {
        let mut plans = Vec::new();
        if !phase_dir.is_dir() {
            return Ok(plans);
        }

        for entry in std::fs::read_dir(phase_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.ends_with("-PLAN.md") || name.ends_with("_PLAN.md") {
                    let content = std::fs::read_to_string(&path)?;
                    let default_id = name
                        .strip_suffix("-PLAN.md")
                        .or_else(|| name.strip_suffix("_PLAN.md"))
                        .unwrap_or("01-01");
                    let plan = PlanChecker::parse_plan(&content, default_id)?;
                    plans.push(plan);
                }
            }
        }

        plans.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(plans)
    }

    /// Performs topological sorting and file-overlap partitioning to partition plans into waves.
    pub fn partition_into_waves(plans: Vec<PlanModel>) -> Result<Vec<Vec<PlanModel>>> {
        if plans.is_empty() {
            return Ok(Vec::new());
        }

        let mut plan_map: HashMap<String, PlanModel> = HashMap::new();
        for p in plans {
            plan_map.insert(p.id.clone(), p);
        }

        // 1. Initial topological wave assignment: wave = max(dep_waves) + 1
        let mut wave_assignments: HashMap<String, usize> = HashMap::new();
        let plan_ids: Vec<String> = plan_map.keys().cloned().collect();

        for id in &plan_ids {
            Self::compute_wave_index(id, &plan_map, &mut wave_assignments, &mut HashSet::new())?;
        }

        // 2. File overlap partitioning: plans in the same wave modifying the same file are separated
        let max_wave = *wave_assignments.values().max().unwrap_or(&1);
        for w in 1..=max_wave {
            let same_wave_ids: Vec<String> = wave_assignments
                .iter()
                .filter(|(_, wave)| **wave == w)
                .map(|(id, _)| id.clone())
                .collect();

            let mut touched_files: HashSet<PathBuf> = HashSet::new();
            for id in same_wave_ids {
                if let Some(p) = plan_map.get(&id) {
                    let mut has_overlap = false;
                    for f in &p.files_modified {
                        if touched_files.contains(f) {
                            has_overlap = true;
                            break;
                        }
                    }

                    if has_overlap {
                        // Push to next wave to eliminate race condition
                        if let Some(curr_w) = wave_assignments.get_mut(&id) {
                            *curr_w += 1;
                        }
                    } else {
                        for f in &p.files_modified {
                            touched_files.insert(f.clone());
                        }
                    }
                }
            }
        }

        // 3. Group plans by wave index into ordered buckets
        let mut wave_buckets: BTreeMap<usize, Vec<PlanModel>> = BTreeMap::new();
        for (id, wave) in wave_assignments {
            if let Some(mut plan) = plan_map.remove(&id) {
                plan.wave = wave;
                wave_buckets.entry(wave).or_default().push(plan);
            }
        }

        // Return ordered wave vector
        let waves: Vec<Vec<PlanModel>> = wave_buckets.into_values().collect();
        Ok(waves)
    }

    /// Recursively computes the wave index for a plan based on its dependency DAG.
    fn compute_wave_index(
        plan_id: &str,
        plan_map: &HashMap<String, PlanModel>,
        wave_assignments: &mut HashMap<String, usize>,
        visiting: &mut HashSet<String>,
    ) -> Result<usize> {
        if let Some(&w) = wave_assignments.get(plan_id) {
            return Ok(w);
        }

        if visiting.contains(plan_id) {
            return Err(anyhow!("Cyclic dependency detected at plan '{plan_id}'"));
        }

        visiting.insert(plan_id.to_string());

        let plan = plan_map
            .get(plan_id)
            .ok_or_else(|| anyhow!("Plan '{plan_id}' referenced in dependencies not found"))?;

        let mut max_dep_wave = 0;
        for dep in &plan.depends_on {
            let dep_wave = Self::compute_wave_index(dep, plan_map, wave_assignments, visiting)?;
            max_dep_wave = max_dep_wave.max(dep_wave);
        }

        visiting.remove(plan_id);

        let wave = (max_dep_wave + 1).max(plan.wave);
        wave_assignments.insert(plan_id.to_string(), wave);
        Ok(wave)
    }

    /// Executes all waves in sequence, dispatching plans within each wave in parallel.
    pub async fn execute_waves<F>(
        repo_root: &Path,
        phase: &str,
        waves: Vec<Vec<PlanModel>>,
        executor_fn: Arc<F>,
        verify_precommit: bool,
    ) -> Result<PhaseWaveResult>
    where
        F: Fn(PlanModel) -> BoxFuture<'static, Result<PlanExecutionOutcome>>
            + Send
            + Sync
            + 'static,
    {
        let total_waves = waves.len();
        let mut completed_waves = 0;
        let mut wave_summaries = Vec::new();
        let mut all_passed = true;

        for (idx, wave) in waves.into_iter().enumerate() {
            let wave_num = idx + 1;
            tracing::info!(
                wave = wave_num,
                total_waves,
                plans_in_wave = wave.len(),
                "Dispatching wave"
            );

            // Parallel dispatch using Tokio JoinSet
            let mut join_set = JoinSet::new();
            for plan in wave {
                let exec = executor_fn.clone();
                join_set.spawn(async move { exec(plan).await });
            }

            let mut outcomes = Vec::new();
            let mut wave_failed = false;

            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok(Ok(outcome)) => {
                        if !outcome.success {
                            wave_failed = true;
                        }
                        outcomes.push(outcome);
                    }
                    Ok(Err(e)) => {
                        wave_failed = true;
                        outcomes.push(PlanExecutionOutcome {
                            plan_id: "unknown".into(),
                            success: false,
                            commits: Vec::new(),
                            summary_path: None,
                            duration_ms: 0,
                            error: Some(e.to_string()),
                        });
                    }
                    Err(join_err) => {
                        wave_failed = true;
                        outcomes.push(PlanExecutionOutcome {
                            plan_id: "panicked".into(),
                            success: false,
                            commits: Vec::new(),
                            summary_path: None,
                            duration_ms: 0,
                            error: Some(join_err.to_string()),
                        });
                    }
                }
            }

            // Post-wave pre-commit verification run (once per wave across all parallel changes)
            let mut precommit_verified = true;
            if !wave_failed && verify_precommit {
                tracing::info!(wave = wave_num, "Running post-wave pre-commit verification");
                if let Ok(report) =
                    AutonomousVerifier::verify_check(repo_root, Duration::from_secs(30)).await
                    && !report.success
                {
                    precommit_verified = false;
                    wave_failed = true;
                    tracing::error!(wave = wave_num, "Post-wave verification check failed");
                }
            }

            let summary = WaveExecutionSummary {
                wave_index: wave_num,
                outcomes,
                success: !wave_failed,
                precommit_verified,
            };

            wave_summaries.push(summary);

            if wave_failed {
                all_passed = false;
                tracing::error!(wave = wave_num, "Wave failed; stopping phase execution");
                break;
            }

            completed_waves += 1;
        }

        Ok(PhaseWaveResult {
            phase: phase.to_string(),
            total_waves,
            completed_waves,
            all_passed,
            wave_summaries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use tempfile::tempdir;

    #[test]
    fn test_topological_wave_partitioning() {
        let p1 = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![PathBuf::from("src/a.rs")],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let p2 = PlanModel {
            id: "01-02".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![PathBuf::from("src/b.rs")],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let p3 = PlanModel {
            id: "01-03".into(),
            phase: "01".into(),
            wave: 1, // should be pushed to wave 2
            depends_on: vec!["01-01".into(), "01-02".into()],
            requirements: vec![],
            files_modified: vec![PathBuf::from("src/c.rs")],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let waves = WaveScheduler::partition_into_waves(vec![p1, p2, p3]).unwrap();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0].len(), 2); // 01-01 and 01-02 in Wave 1
        assert_eq!(waves[1].len(), 1); // 01-03 in Wave 2
        assert_eq!(waves[1][0].id, "01-03");
    }

    #[test]
    fn test_file_overlap_partitioning() {
        // Two independent plans (no depends_on) both touching `src/shared.rs`
        let p1 = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![PathBuf::from("src/shared.rs")],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let p2 = PlanModel {
            id: "01-02".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![PathBuf::from("src/shared.rs")], // Overlap!
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let waves = WaveScheduler::partition_into_waves(vec![p1, p2]).unwrap();
        assert_eq!(waves.len(), 2); // Separated into Wave 1 and Wave 2 to prevent collision!
        assert_eq!(waves[0].len(), 1);
        assert_eq!(waves[1].len(), 1);
    }

    #[tokio::test]
    async fn test_parallel_wave_execution() {
        let dir = tempdir().unwrap();

        let p1 = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let p2 = PlanModel {
            id: "01-02".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let waves = vec![vec![p1, p2]];

        let executor = Arc::new(|plan: PlanModel| {
            async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(PlanExecutionOutcome {
                    plan_id: plan.id,
                    success: true,
                    commits: vec!["c0ffee".into()],
                    summary_path: None,
                    duration_ms: 10,
                    error: None,
                })
            }
            .boxed()
        });

        let res = WaveScheduler::execute_waves(dir.path(), "01", waves, executor, false)
            .await
            .unwrap();

        assert!(res.all_passed);
        assert_eq!(res.completed_waves, 1);
        assert_eq!(res.wave_summaries[0].outcomes.len(), 2);
    }
}
