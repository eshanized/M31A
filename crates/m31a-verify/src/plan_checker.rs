//! 8-Dimension Pre-Execution Plan Checker for M31A.
//!
//! 8-Dimension Pre-Execution Plan Checker verifying feasibility, coverage, and safety before execution:
//!
//! Enforces an explicit FORCE adversarial stance ("assume plans fail until proven otherwise")
//! to verify that phase plans WILL achieve their goals BEFORE burning execution tokens.
//!
//! Evaluates plans across 8 rigorous dimensions:
//! 1. Requirement Coverage: 100% claim of phase requirement IDs (missing -> BLOCKER).
//! 2. Task Completeness: Files + Action + Verify + Done present for auto tasks (missing -> BLOCKER).
//! 3. Dependency Correctness: Acyclic DAG and consistent wave assignments (cycle -> BLOCKER).
//!    3b. Temporal Coupling: Same-wave plans sharing mutable resources without declared dependency.
//! 4. Key Links Planned: Concrete wiring planned between created artifacts (missing wiring -> WARNING).
//! 5. Scope Sanity: Task count (2-3 target, 4 warning, 5+ blocker) and file count bounds.
//! 6. Verification Derivation: User-observable must-have truths rather than implementation stubs.
//! 7. Context Compliance: 100% of `<decisions>` from `CONTEXT.md` mapped to tasks (missing -> BLOCKER).
//! 8. Nyquist Compliance: Automated verify command per task, test scaffolding scheduled, latency bounded.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Blocker,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanIssue {
    pub dimension: String,
    pub severity: IssueSeverity,
    pub plan_id: String,
    pub task_index: Option<usize>,
    pub description: String,
    pub fix_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskModel {
    pub name: String,
    pub task_type: String,
    pub files: Vec<PathBuf>,
    pub action: String,
    pub verify: Option<String>,
    pub done: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModel {
    pub id: String,
    pub phase: String,
    pub wave: usize,
    pub depends_on: Vec<String>,
    pub requirements: Vec<String>,
    pub files_modified: Vec<PathBuf>,
    pub must_haves_truths: Vec<String>,
    pub must_haves_artifacts: Vec<PathBuf>,
    pub must_haves_key_links: Vec<String>,
    pub tasks: Vec<TaskModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCheckReport {
    pub passed: bool,
    pub total_plans: usize,
    pub blockers: Vec<PlanIssue>,
    pub warnings: Vec<PlanIssue>,
    pub infos: Vec<PlanIssue>,
    pub dimension_summaries: BTreeMap<String, String>,
}

pub struct PlanChecker;

impl PlanChecker {
    /// Parses a standard markdown `PLAN.md` document with YAML frontmatter and `<task>` elements.
    pub fn parse_plan(content: &str, default_id: &str) -> Result<PlanModel> {
        let mut id = default_id.to_string();
        let mut phase = "01".to_string();
        let mut wave = 1;
        let mut depends_on = Vec::new();
        let mut requirements = Vec::new();
        let mut files_modified = Vec::new();
        let mut must_haves_truths = Vec::new();
        let mut must_haves_artifacts = Vec::new();
        let mut must_haves_key_links = Vec::new();

        // 1. Parse Frontmatter
        let trimmed = content.trim_start();
        if trimmed.starts_with("---")
            && let Some(end_idx) = trimmed[3..].find("\n---")
        {
            let fm_raw = &trimmed[3..3 + end_idx];
            let mut current_section = "";

            for line in fm_raw.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if line.starts_with("plan:") {
                    if let Some((_, v)) = line.split_once(':') {
                        id = v.trim().trim_matches(['"', '\'']).to_string();
                    }
                } else if line.starts_with("phase:") {
                    if let Some((_, v)) = line.split_once(':') {
                        phase = v.trim().trim_matches(['"', '\'']).to_string();
                    }
                } else if line.starts_with("wave:") {
                    if let Some((_, v)) = line.split_once(':') {
                        wave = v.trim().parse::<usize>().unwrap_or(1);
                    }
                } else if line.starts_with("depends_on:") {
                    current_section = "depends_on";
                    Self::parse_inline_array(line, &mut depends_on);
                } else if line.starts_with("requirements:") {
                    current_section = "requirements";
                    Self::parse_inline_array(line, &mut requirements);
                } else if line.starts_with("files_modified:") {
                    current_section = "files_modified";
                    let mut string_paths = Vec::new();
                    Self::parse_inline_array(line, &mut string_paths);
                    for p in string_paths {
                        files_modified.push(PathBuf::from(p));
                    }
                } else if line.starts_with("truths:") {
                    current_section = "truths";
                } else if line.starts_with("artifacts:") {
                    current_section = "artifacts";
                } else if line.starts_with("key_links:") {
                    current_section = "key_links";
                } else if line.starts_with('-') {
                    let item = line
                        .trim_start_matches('-')
                        .trim()
                        .trim_matches(['"', '\''])
                        .to_string();
                    match current_section {
                        "depends_on" => depends_on.push(item),
                        "requirements" => requirements.push(item),
                        "files_modified" => files_modified.push(PathBuf::from(item)),
                        "truths" => must_haves_truths.push(item),
                        "artifacts" => must_haves_artifacts.push(PathBuf::from(item)),
                        "key_links" => must_haves_key_links.push(item),
                        _ => {}
                    }
                }
            }
        }

        // 2. Parse XML `<task>` tags in markdown body
        let tasks = Self::parse_tasks(content)?;

        Ok(PlanModel {
            id,
            phase,
            wave,
            depends_on,
            requirements,
            files_modified,
            must_haves_truths,
            must_haves_artifacts,
            must_haves_key_links,
            tasks,
        })
    }

    /// Evaluates a collection of phase plans across all 8 verification dimensions.
    pub fn check_plans(
        plans: &[PlanModel],
        phase_requirements: &[String],
        locked_context_decisions: &[String],
    ) -> PlanCheckReport {
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        let mut infos = Vec::new();
        let mut summaries = BTreeMap::new();

        // Dimension 1: Requirement Coverage
        let (dim1_issues, dim1_summary) =
            Self::check_dimension_1_requirements(plans, phase_requirements);
        Self::categorize_issues(dim1_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 1: Requirement Coverage".into(), dim1_summary);

        // Dimension 2: Task Completeness
        let (dim2_issues, dim2_summary) = Self::check_dimension_2_completeness(plans);
        Self::categorize_issues(dim2_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 2: Task Completeness".into(), dim2_summary);

        // Dimension 3: Dependency Correctness & 3b Coupling
        let (dim3_issues, dim3_summary) = Self::check_dimension_3_dependencies(plans);
        Self::categorize_issues(dim3_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 3: Dependency Correctness".into(), dim3_summary);

        // Dimension 4: Key Links Planned
        let (dim4_issues, dim4_summary) = Self::check_dimension_4_key_links(plans);
        Self::categorize_issues(dim4_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 4: Key Links Planned".into(), dim4_summary);

        // Dimension 5: Scope Sanity
        let (dim5_issues, dim5_summary) = Self::check_dimension_5_scope_sanity(plans);
        Self::categorize_issues(dim5_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 5: Scope Sanity".into(), dim5_summary);

        // Dimension 6: Verification Derivation
        let (dim6_issues, dim6_summary) = Self::check_dimension_6_verification_derivation(plans);
        Self::categorize_issues(dim6_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 6: Verification Derivation".into(), dim6_summary);

        // Dimension 7: Context Compliance
        let (dim7_issues, dim7_summary) =
            Self::check_dimension_7_context_compliance(plans, locked_context_decisions);
        Self::categorize_issues(dim7_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 7: Context Compliance".into(), dim7_summary);

        // Dimension 8: Nyquist Compliance
        let (dim8_issues, dim8_summary) = Self::check_dimension_8_nyquist(plans);
        Self::categorize_issues(dim8_issues, &mut blockers, &mut warnings, &mut infos);
        summaries.insert("Dimension 8: Nyquist Compliance".into(), dim8_summary);

        let passed = blockers.is_empty();

        PlanCheckReport {
            passed,
            total_plans: plans.len(),
            blockers,
            warnings,
            infos,
            dimension_summaries: summaries,
        }
    }

    // ─── Dimension Checkers ────────────────────────────────────────────────────

    fn check_dimension_1_requirements(
        plans: &[PlanModel],
        phase_requirements: &[String],
    ) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();
        let mut claimed = HashSet::new();

        for plan in plans {
            for req in &plan.requirements {
                claimed.insert(req.clone());
            }
        }

        let mut missing_reqs = Vec::new();
        for req in phase_requirements {
            if !claimed.contains(req) {
                missing_reqs.push(req.clone());
                issues.push(PlanIssue {
                    dimension: "requirement_coverage".into(),
                    severity: IssueSeverity::Blocker,
                    plan_id: plans
                        .first()
                        .map(|p| p.id.clone())
                        .unwrap_or_else(|| "none".into()),
                    task_index: None,
                    description: format!(
                        "Phase requirement '{req}' is not claimed by any plan's frontmatter"
                    ),
                    fix_hint: format!("Add '{req}' to requirements array of the implementing plan"),
                });
            }
        }

        let summary = if issues.is_empty() {
            format!(
                "✅ All {} phase requirements covered",
                phase_requirements.len()
            )
        } else {
            format!(
                "❌ {} requirements missing coverage: {:?}",
                missing_reqs.len(),
                missing_reqs
            )
        };

        (issues, summary)
    }

    fn check_dimension_2_completeness(plans: &[PlanModel]) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();
        let mut total_tasks = 0;

        for plan in plans {
            for (idx, task) in plan.tasks.iter().enumerate() {
                total_tasks += 1;
                if task.task_type == "auto" || task.task_type == "tdd" {
                    if task.verify.is_none() || task.verify.as_ref().unwrap().trim().is_empty() {
                        issues.push(PlanIssue {
                            dimension: "task_completeness".into(),
                            severity: IssueSeverity::Blocker,
                            plan_id: plan.id.clone(),
                            task_index: Some(idx + 1),
                            description: format!(
                                "Task {} '{}' missing <verify> element",
                                idx + 1,
                                task.name
                            ),
                            fix_hint: "Add specific verification command separating pass from fail"
                                .into(),
                        });
                    }

                    if task.done.is_none() || task.done.as_ref().unwrap().trim().is_empty() {
                        issues.push(PlanIssue {
                            dimension: "task_completeness".into(),
                            severity: IssueSeverity::Blocker,
                            plan_id: plan.id.clone(),
                            task_index: Some(idx + 1),
                            description: format!(
                                "Task {} '{}' missing <done> acceptance criteria",
                                idx + 1,
                                task.name
                            ),
                            fix_hint: "Add explicit observable acceptance criteria in <done>"
                                .into(),
                        });
                    }

                    if task.files.is_empty() {
                        issues.push(PlanIssue {
                            dimension: "task_completeness".into(),
                            severity: IssueSeverity::Warning,
                            plan_id: plan.id.clone(),
                            task_index: Some(idx + 1),
                            description: format!(
                                "Task {} '{}' has empty <files> element",
                                idx + 1,
                                task.name
                            ),
                            fix_hint: "List exact files created or modified by this task".into(),
                        });
                    }
                }
            }
        }

        let summary = if issues.is_empty() {
            format!("✅ All {total_tasks} tasks complete with Action, Files, Verify, and Done")
        } else {
            format!("❌ Found {} task completeness issues", issues.len())
        };

        (issues, summary)
    }

    fn check_dimension_3_dependencies(plans: &[PlanModel]) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();
        let plan_map: HashMap<String, &PlanModel> =
            plans.iter().map(|p| (p.id.clone(), p)).collect();

        // 1. Check for missing references and DAG cycles
        for plan in plans {
            for dep in &plan.depends_on {
                if !plan_map.contains_key(dep) {
                    issues.push(PlanIssue {
                        dimension: "dependency_correctness".into(),
                        severity: IssueSeverity::Blocker,
                        plan_id: plan.id.clone(),
                        task_index: None,
                        description: format!(
                            "Plan {} depends on non-existent plan '{}'",
                            plan.id, dep
                        ),
                        fix_hint: "Remove invalid dependency or fix plan ID spelling".into(),
                    });
                } else if dep == &plan.id {
                    issues.push(PlanIssue {
                        dimension: "dependency_correctness".into(),
                        severity: IssueSeverity::Blocker,
                        plan_id: plan.id.clone(),
                        task_index: None,
                        description: format!("Plan {} has self-dependency", plan.id),
                        fix_hint: "Remove self-dependency from depends_on".into(),
                    });
                }
            }
        }

        // Cycle detection via DFS
        for plan in plans {
            let mut visited = HashSet::new();
            let mut rec_stack = HashSet::new();
            if Self::has_cycle(&plan.id, &plan_map, &mut visited, &mut rec_stack) {
                issues.push(PlanIssue {
                    dimension: "dependency_correctness".into(),
                    severity: IssueSeverity::Blocker,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!("Circular dependency detected involving plan {}", plan.id),
                    fix_hint: "Break the cycle by reordering dependencies".into(),
                });
            }
        }

        // 2. Wave consistency: wave = max(dep_waves) + 1
        for plan in plans {
            let max_dep_wave = plan
                .depends_on
                .iter()
                .filter_map(|dep| plan_map.get(dep).map(|p| p.wave))
                .max()
                .unwrap_or(0);

            if plan.wave <= max_dep_wave {
                issues.push(PlanIssue {
                    dimension: "dependency_correctness".into(),
                    severity: IssueSeverity::Blocker,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!(
                        "Plan {} scheduled in wave {}, but depends on plan in wave {}",
                        plan.id, plan.wave, max_dep_wave
                    ),
                    fix_hint: format!(
                        "Assign plan {} to wave {} or higher",
                        plan.id,
                        max_dep_wave + 1
                    ),
                });
            }
        }

        // 3. Dimension 3b: Same-wave shared mutable file overlap
        for i in 0..plans.len() {
            for j in (i + 1)..plans.len() {
                let p1 = &plans[i];
                let p2 = &plans[j];
                if p1.wave == p2.wave {
                    let s1: HashSet<&PathBuf> = p1.files_modified.iter().collect();
                    let s2: HashSet<&PathBuf> = p2.files_modified.iter().collect();
                    let overlap: Vec<_> = s1.intersection(&s2).collect();
                    if !overlap.is_empty() {
                        issues.push(PlanIssue {
                            dimension: "dependency_correctness".into(),
                            severity: IssueSeverity::Warning,
                            plan_id: p1.id.clone(),
                            task_index: None,
                            description: format!(
                                "Same-wave coupling: plans {} and {} both modify {:?} in wave {}",
                                p1.id, p2.id, overlap, p1.wave
                            ),
                            fix_hint:
                                "Separate plans into consecutive waves or merge into single plan"
                                    .into(),
                        });
                    }
                }
            }
        }

        let summary = if issues.is_empty() {
            "✅ Dependencies acyclic with consistent wave scheduling".into()
        } else {
            format!("❌ Found {} dependency/coupling issues", issues.len())
        };

        (issues, summary)
    }

    fn check_dimension_4_key_links(plans: &[PlanModel]) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();
        for plan in plans {
            if !plan.must_haves_artifacts.is_empty() && plan.must_haves_key_links.is_empty() {
                issues.push(PlanIssue {
                    dimension: "key_links_planned".into(),
                    severity: IssueSeverity::Warning,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!(
                        "Plan {} creates {} artifacts but declares zero must_haves.key_links wiring",
                        plan.id,
                        plan.must_haves_artifacts.len()
                    ),
                    fix_hint: "Add key_links describing how created artifacts wire into the broader system".into(),
                });
            }
        }

        let summary = if issues.is_empty() {
            "✅ Key links and artifact wiring verified".into()
        } else {
            format!("⚠️ {} plans missing artifact wiring links", issues.len())
        };

        (issues, summary)
    }

    fn check_dimension_5_scope_sanity(plans: &[PlanModel]) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();

        for plan in plans {
            let task_count = plan.tasks.len();
            if task_count >= 5 {
                issues.push(PlanIssue {
                    dimension: "scope_sanity".into(),
                    severity: IssueSeverity::Blocker,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!(
                        "Plan {} has {task_count} tasks (max threshold 4)",
                        plan.id
                    ),
                    fix_hint: "Split plan into 2 smaller plans to prevent context rot".into(),
                });
            } else if task_count == 4 {
                issues.push(PlanIssue {
                    dimension: "scope_sanity".into(),
                    severity: IssueSeverity::Warning,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!("Plan {} has 4 tasks (target is 2-3)", plan.id),
                    fix_hint: "Consider decomposing into 2 smaller plans".into(),
                });
            }

            let file_count = plan.files_modified.len();
            if file_count >= 15 {
                issues.push(PlanIssue {
                    dimension: "scope_sanity".into(),
                    severity: IssueSeverity::Blocker,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!("Plan {} touches {file_count} files (limit 14)", plan.id),
                    fix_hint: "Split work across multiple plans".into(),
                });
            } else if file_count >= 10 {
                issues.push(PlanIssue {
                    dimension: "scope_sanity".into(),
                    severity: IssueSeverity::Warning,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!(
                        "Plan {} touches {file_count} files (target is 5-8)",
                        plan.id
                    ),
                    fix_hint: "Scope is wide; verify executor context budget".into(),
                });
            }
        }

        let summary = if issues.is_empty() {
            "✅ All plans within task and file context budgets".into()
        } else {
            format!("⚠️ Found {} scope sanity warnings/blockers", issues.len())
        };

        (issues, summary)
    }

    fn check_dimension_6_verification_derivation(plans: &[PlanModel]) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();
        let stub_pattern =
            Regex::new(r"(?i)\b(installed|added library|created file|package added)\b").unwrap();

        for plan in plans {
            if plan.must_haves_truths.is_empty() {
                issues.push(PlanIssue {
                    dimension: "verification_derivation".into(),
                    severity: IssueSeverity::Warning,
                    plan_id: plan.id.clone(),
                    task_index: None,
                    description: format!("Plan {} has no must_haves.truths declared", plan.id),
                    fix_hint: "Define user-observable truths that hold when this plan succeeds"
                        .into(),
                });
            } else {
                for truth in &plan.must_haves_truths {
                    if stub_pattern.is_match(truth) {
                        issues.push(PlanIssue {
                            dimension: "verification_derivation".into(),
                            severity: IssueSeverity::Warning,
                            plan_id: plan.id.clone(),
                            task_index: None,
                            description: format!(
                                "Truth '{}' is implementation-focused, not user-observable",
                                truth
                            ),
                            fix_hint:
                                "Reframe truth as observable behavior (e.g. 'User can log in')"
                                    .into(),
                        });
                    }
                }
            }
        }

        let summary = if issues.is_empty() {
            "✅ Must-have truths are user-observable and properly derived".into()
        } else {
            format!(
                "⚠️ {} truths need reframing as user-observable",
                issues.len()
            )
        };

        (issues, summary)
    }

    fn check_dimension_7_context_compliance(
        plans: &[PlanModel],
        locked_context_decisions: &[String],
    ) -> (Vec<PlanIssue>, String) {
        if locked_context_decisions.is_empty() {
            return (
                Vec::new(),
                "SKIPPED (no locked CONTEXT.md decisions provided)".into(),
            );
        }

        let mut issues = Vec::new();
        let mut combined_actions = String::new();

        for plan in plans {
            for task in &plan.tasks {
                combined_actions.push_str(&task.action);
                combined_actions.push(' ');
            }
        }

        let mut missing_decisions = Vec::new();
        for dec in locked_context_decisions {
            // Check if decision token (e.g., "D-01" or full text) appears in any task action
            let token = dec.split(':').next().unwrap_or(dec).trim();
            if !combined_actions.contains(token) {
                missing_decisions.push(token.to_string());
                issues.push(PlanIssue {
                    dimension: "context_compliance".into(),
                    severity: IssueSeverity::Blocker,
                    plan_id: plans.first().map(|p| p.id.clone()).unwrap_or_else(|| "none".into()),
                    task_index: None,
                    description: format!("Locked user decision '{token}' is not referenced or implemented in any task action"),
                    fix_hint: format!("Reference '{token}' in the relevant task <action> element"),
                });
            }
        }

        let summary = if issues.is_empty() {
            format!(
                "✅ 100% of {} locked user decisions covered",
                locked_context_decisions.len()
            )
        } else {
            format!(
                "❌ {} locked decisions missing from task actions: {:?}",
                missing_decisions.len(),
                missing_decisions
            )
        };

        (issues, summary)
    }

    fn check_dimension_8_nyquist(plans: &[PlanModel]) -> (Vec<PlanIssue>, String) {
        let mut issues = Vec::new();
        let watch_regex = Regex::new(r"(?i)\b(--watch|--watchAll)\b").unwrap();

        for plan in plans {
            let mut consecutive_unverified = 0;

            for (idx, task) in plan.tasks.iter().enumerate() {
                if let Some(verify_cmd) = &task.verify {
                    if watch_regex.is_match(verify_cmd) {
                        issues.push(PlanIssue {
                            dimension: "nyquist_compliance".into(),
                            severity: IssueSeverity::Blocker,
                            plan_id: plan.id.clone(),
                            task_index: Some(idx + 1),
                            description: format!(
                                "Verify command '{}' contains watch mode flag",
                                verify_cmd
                            ),
                            fix_hint: "Remove --watch/--watchAll flag to allow automated exit"
                                .into(),
                        });
                    }
                    consecutive_unverified = 0;
                } else if task.task_type == "auto" {
                    consecutive_unverified += 1;
                    if consecutive_unverified >= 3 {
                        issues.push(PlanIssue {
                            dimension: "nyquist_compliance".into(),
                            severity: IssueSeverity::Blocker,
                            plan_id: plan.id.clone(),
                            task_index: Some(idx + 1),
                            description: "Sampling continuity failure: 3 consecutive tasks without automated verification".into(),
                            fix_hint: "Add automated verify command to break consecutive unverified tasks".into(),
                        });
                    }
                }
            }
        }

        let summary = if issues.is_empty() {
            "✅ Nyquist sampling continuity and verify commands valid".into()
        } else {
            format!("❌ Found {} Nyquist compliance issues", issues.len())
        };

        (issues, summary)
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    fn categorize_issues(
        issues: Vec<PlanIssue>,
        blockers: &mut Vec<PlanIssue>,
        warnings: &mut Vec<PlanIssue>,
        infos: &mut Vec<PlanIssue>,
    ) {
        for issue in issues {
            match issue.severity {
                IssueSeverity::Blocker => blockers.push(issue),
                IssueSeverity::Warning => warnings.push(issue),
                IssueSeverity::Info => infos.push(issue),
            }
        }
    }

    fn parse_inline_array(line: &str, target: &mut Vec<String>) {
        if let Some((_, arr_str)) = line.split_once(':') {
            let arr_str = arr_str.trim();
            if arr_str.starts_with('[') && arr_str.ends_with(']') {
                let inner = &arr_str[1..arr_str.len() - 1];
                for item in inner.split(',') {
                    let cleaned = item.trim().trim_matches(['"', '\'']).to_string();
                    if !cleaned.is_empty() {
                        target.push(cleaned);
                    }
                }
            }
        }
    }

    fn parse_tasks(content: &str) -> Result<Vec<TaskModel>> {
        let mut tasks = Vec::new();
        let task_re = Regex::new(r#"(?s)<task\b([^>]*)>(.*?)</task>"#)?;
        let name_re = Regex::new(r#"(?s)<name>(.*?)</name>"#)?;
        let files_re = Regex::new(r#"(?s)<files>(.*?)</files>"#)?;
        let action_re = Regex::new(r#"(?s)<action>(.*?)</action>"#)?;
        let verify_re = Regex::new(r#"(?s)<verify>(.*?)</verify>"#)?;
        let done_re = Regex::new(r#"(?s)<done>(.*?)</done>"#)?;

        for cap in task_re.captures_iter(content) {
            let attrs = &cap[1];
            let body = &cap[2];

            let mut task_type = "auto".to_string();
            if let Some(t_idx) = attrs.find("type=\"")
                && let Some(end_q) = attrs[t_idx + 6..].find('"')
            {
                task_type = attrs[t_idx + 6..t_idx + 6 + end_q].to_string();
            }

            let name = name_re
                .captures(body)
                .map(|c| c[1].trim().to_string())
                .unwrap_or_else(|| "Unnamed task".to_string());

            let mut files = Vec::new();
            if let Some(f_cap) = files_re.captures(body) {
                for line in f_cap[1].lines() {
                    let p = line.trim().trim_matches(['"', '\'']);
                    if !p.is_empty() {
                        files.push(PathBuf::from(p));
                    }
                }
            }

            let action = action_re
                .captures(body)
                .map(|c| c[1].trim().to_string())
                .unwrap_or_default();

            let verify = verify_re.captures(body).map(|c| c[1].trim().to_string());
            let done = done_re.captures(body).map(|c| c[1].trim().to_string());

            tasks.push(TaskModel {
                name,
                task_type,
                files,
                action,
                verify,
                done,
            });
        }

        Ok(tasks)
    }

    fn has_cycle(
        curr: &str,
        graph: &HashMap<String, &PlanModel>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(curr.to_string());
        rec_stack.insert(curr.to_string());

        if let Some(plan) = graph.get(curr) {
            for dep in &plan.depends_on {
                if !visited.contains(dep) {
                    if Self::has_cycle(dep, graph, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(dep) {
                    return true;
                }
            }
        }

        rec_stack.remove(curr);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plan_markdown() {
        let content = r#"---
plan: "01-01"
phase: "01"
wave: 1
depends_on: []
requirements: ["AUTH-01", "AUTH-02"]
files_modified: ["src/auth.rs"]
must_haves:
  truths:
    - "User can login"
  artifacts:
    - "src/auth.rs"
  key_links:
    - "main.rs calls auth::login"
---

# Plan 01-01

<task type="auto">
  <name>Task 1: Add auth handler</name>
  <files>src/auth.rs</files>
  <action>Implement token verification with D-01</action>
  <verify>cargo test test_auth</verify>
  <done>Tokens are verified</done>
</task>
"#;
        let plan = PlanChecker::parse_plan(content, "default-01").unwrap();
        assert_eq!(plan.id, "01-01");
        assert_eq!(plan.wave, 1);
        assert_eq!(plan.requirements, vec!["AUTH-01", "AUTH-02"]);
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.tasks[0].name, "Task 1: Add auth handler");
        assert_eq!(plan.tasks[0].verify, Some("cargo test test_auth".into()));
    }

    #[test]
    fn test_requirement_coverage_blocker() {
        let plan = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec!["AUTH-01".into()],
            files_modified: vec![],
            must_haves_truths: vec!["User login works".into()],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![TaskModel {
                name: "Task 1".into(),
                task_type: "auto".into(),
                files: vec![PathBuf::from("src/auth.rs")],
                action: "Action D-01".into(),
                verify: Some("cargo test".into()),
                done: Some("Done".into()),
            }],
        };

        let reqs = vec!["AUTH-01".to_string(), "AUTH-02".to_string()];
        let report = PlanChecker::check_plans(&[plan], &reqs, &["D-01".into()]);
        assert!(!report.passed);
        assert_eq!(report.blockers.len(), 1);
        assert!(report.blockers[0].description.contains("AUTH-02"));
    }

    #[test]
    fn test_task_completeness_missing_verify_blocker() {
        let plan = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec!["AUTH-01".into()],
            files_modified: vec![],
            must_haves_truths: vec!["Truth".into()],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![TaskModel {
                name: "Task 1".into(),
                task_type: "auto".into(),
                files: vec![PathBuf::from("src/lib.rs")],
                action: "Do work D-01".into(),
                verify: None, // Missing verify!
                done: Some("Done".into()),
            }],
        };

        let report = PlanChecker::check_plans(&[plan], &["AUTH-01".into()], &["D-01".into()]);
        assert!(!report.passed);
        assert!(
            report
                .blockers
                .iter()
                .any(|b| b.dimension == "task_completeness")
        );
    }

    #[test]
    fn test_circular_dependency_blocker() {
        let p1 = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec!["01-02".into()],
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
            wave: 2,
            depends_on: vec!["01-01".into()],
            requirements: vec![],
            files_modified: vec![],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![],
        };

        let report = PlanChecker::check_plans(&[p1, p2], &[], &[]);
        assert!(!report.passed);
        assert!(
            report
                .blockers
                .iter()
                .any(|b| b.dimension == "dependency_correctness")
        );
    }
}
