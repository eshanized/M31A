//! Bounded parallel research orchestrator and workflow definition compiler.

use super::dimension_registry::ResearchDimensionRegistry;
use super::errors::GenesisError;
use super::intake::GenesisOptions;
use super::project::ProjectCharter;
use super::research_decision::ResearchDecision;
use super::synthesis::{ResearchSummary, ResearchSynthesizer};
use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use crate::state_machine::agent::AgentRole;
use crate::workflow::compiler::CompiledWorkflow;
use crate::workflow::definition::{
    InputBinding, OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition,
    WorkflowStepDefinition,
};
use crate::workflow::engine::{WorkflowEngine, WorkflowStartRequest};
use crate::workflow::provenance::WorkflowProvenance;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Orchestrates multi-dimensional research through the WorkflowEngine.
pub struct ResearchOrchestrator;

impl ResearchOrchestrator {
    /// Compiles a ResearchDecision into a formal WorkflowDefinition with bounded waves.
    ///
    /// Generic over registered dimensions: every step is
    /// built from the dimension's registry definition. An unregistered
    /// dimension id in the decision fails explicitly — orchestration never
    /// assumes per-dimension metadata.
    pub fn build_workflow_definition(
        charter: &ProjectCharter,
        decision: &ResearchDecision,
        options: &GenesisOptions,
    ) -> Result<WorkflowDefinition, GenesisError> {
        if !decision.execute_research || decision.selected_dimensions.is_empty() {
            return Err(GenesisError::ResearchDecision(
                "cannot build research workflow: research execution was not triggered".to_string(),
            ));
        }

        let mut steps = Vec::new();
        let mut dimension_step_keys = Vec::new();

        // 1. Compile individual dimension researcher steps from registry
        // definitions. Step keys follow the stable
        // `genesis_research_{id}` convention.
        let registry = ResearchDimensionRegistry::global().read().map_err(|_| {
            GenesisError::ResearchDecision("dimension registry lock poisoned".to_string())
        })?;
        for dim in &decision.selected_dimensions {
            let def = registry.resolve(dim).ok_or_else(|| {
                GenesisError::ResearchDecision(format!(
                    "unknown research dimension '{}': no registered dimension definition",
                    dim.as_str()
                ))
            })?;
            let step_key = format!("genesis_research_{}", def.id.as_str());
            dimension_step_keys.push(step_key.clone());
            let artifact_path = PathBuf::from(&options.projection_dir)
                .join("research")
                .join(&def.artifact_filename);

            let step = WorkflowStepDefinition {
                key: step_key,
                name: format!("Research: {}", def.id.as_str()),
                role: def.agent_role.clone(),
                prompt_template: def.prompt_contract.clone(),
                // First-class execution binding: the dimension's prompt
                // contract flows as a typed reference into the candidate
                // task, not as description text.
                prompt_ref: crate::prompt::PromptReference::parse_lenient(&def.prompt_contract)
                    .map(|r| r.with_explicit_purpose(crate::prompt::PromptPurpose::Genesis))
                    .ok(),
                required_inputs: Vec::new(),
                expected_outputs: vec![OutputBinding {
                    artifact_name: def.artifact_filename.clone(),
                    relative_path: artifact_path,
                    schema_type: "markdown".to_string(),
                }],
                required_capabilities: vec![
                    CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("repo.read", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("artifacts.read", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("docs.search", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("web.search", CapabilityAccessMode::Read),
                ],
                quality_gate: QualityGate {
                    schema: None,
                    required_artifacts: Vec::new(),
                    require_human_approval: false,
                    max_ambiguity_percent: None,
                },
                depends_on: Vec::new(), // Parallel within research wave
                timeout_secs: 600,
                allows_parallelism: true,
                recovery_strategy: Some(RecoveryStrategy::Retry { max_retries: 2 }),
            };

            steps.push(step);
        }

        // 2. Compile synthesis step depending on all dimension steps.
        // Input bindings resolve filenames through the same definitions so
        // custom dimensions flow through unchanged.
        let mut synthesis_inputs = Vec::with_capacity(decision.selected_dimensions.len());
        let synthesis_expected_outputs = vec![OutputBinding {
            artifact_name: "SUMMARY.md".to_string(),
            relative_path: PathBuf::from(&options.projection_dir)
                .join("research")
                .join("SUMMARY.md"),
            schema_type: "markdown".to_string(),
        }];
        let synthesis_required_artifacts = vec!["SUMMARY.md".to_string()];

        for dim in &decision.selected_dimensions {
            let def = registry.resolve(dim).ok_or_else(|| {
                GenesisError::ResearchDecision(format!(
                    "unknown research dimension '{}': no registered dimension definition",
                    dim.as_str()
                ))
            })?;
            synthesis_inputs.push(InputBinding {
                parameter_name: format!("{}_findings", def.id.as_str()),
                source_step_key: format!("genesis_research_{}", def.id.as_str()),
                artifact_name: def.artifact_filename.clone(),
                is_optional: false,
            });
        }

        let synthesis_step = WorkflowStepDefinition {
            key: "genesis_research_synthesis".to_string(),
            name: "Research Synthesis".to_string(),
            role: AgentRole::synthesizer(),
            prompt_template: "genesis.research_synthesis".to_string(),
            prompt_ref: Some(crate::prompt::PromptReference::with_purpose(
                "genesis.research_synthesis",
                1,
                crate::prompt::PromptPurpose::Genesis,
            )),
            required_inputs: synthesis_inputs,
            expected_outputs: synthesis_expected_outputs,
            required_capabilities: vec![
                CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
                CapabilityRequirement::new("fs.write", CapabilityAccessMode::Write),
                CapabilityRequirement::new("artifacts.read", CapabilityAccessMode::Read),
                CapabilityRequirement::new("artifacts.write", CapabilityAccessMode::Write),
                CapabilityRequirement::new("spec.propose", CapabilityAccessMode::ReadWrite),
            ],
            quality_gate: QualityGate {
                schema: None,
                required_artifacts: synthesis_required_artifacts,
                require_human_approval: false,
                max_ambiguity_percent: None,
            },
            depends_on: dimension_step_keys,
            timeout_secs: 600,
            allows_parallelism: false,
            recovery_strategy: Some(RecoveryStrategy::Retry { max_retries: 1 }),
        };

        steps.push(synthesis_step);

        let workflow_def = WorkflowDefinition {
            id: format!("genesis-research-{}", uuid::Uuid::now_v7()),
            name: format!("Project Genesis Domain Research: {}", charter.project_name),
            description: "Bounded parallel domain research and multi-dimensional synthesis"
                .to_string(),
            version: 1,
            steps,
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        workflow_def.validate()?;

        Ok(workflow_def)
    }

    /// Compiles research into a runtime-ready `CompiledWorkflow`.
    pub fn compile_research_workflow(
        charter: &ProjectCharter,
        decision: &ResearchDecision,
        options: &GenesisOptions,
    ) -> Result<CompiledWorkflow, GenesisError> {
        let workflow_def = Self::build_workflow_definition(charter, decision, options)?;

        let topological_order = workflow_def.steps.iter().map(|s| s.key.clone()).collect();
        let provenance = WorkflowProvenance::new(
            &workflow_def.id,
            workflow_def.version,
            "deterministic_genesis_research_hash",
            1,
            BTreeMap::new(),
            None,
            chrono::Utc::now(),
        );

        Ok(CompiledWorkflow {
            definition: workflow_def,
            provenance,
            topological_order,
        })
    }

    /// Parse structured research finding from raw markdown text.
    pub fn parse_finding(
        dim: &super::research_decision::ResearchDimension,
        dimension_id: &str,
        content: &str,
    ) -> Option<crate::workflow::genesis::provenance::ResearchFinding> {
        use crate::workflow::genesis::provenance::ResearchFinding;
        let first_para = content
            .lines()
            .find(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| l.trim().to_string())?;

        let mut finding = ResearchFinding::new(
            dim.clone(),
            format!("{} Findings", dimension_id),
            first_para,
        );

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let bullet = trimmed[2..].trim();
                let lower = bullet.to_lowercase();
                if lower.starts_with("risk:") {
                    finding.risks.push(bullet[5..].trim().to_string());
                } else if lower.starts_with("tradeoff:") {
                    finding.tradeoffs.push(bullet[9..].trim().to_string());
                } else if lower.starts_with("recommendation:") {
                    finding
                        .recommendations
                        .push(bullet[15..].trim().to_string());
                }
            }
        }

        Some(finding)
    }

    /// Materialize research artifacts to disk by an authorized writer.
    ///
    /// Preserves epistemic integrity: only dimensions with genuine findings
    /// are materialized into `.planning/research/<artifact_filename>`.
    /// When no finding exists, no file is fabricated.
    pub fn materialize_research_artifacts(
        workspace_root: &Path,
        projection_dir: &str,
        findings: &[crate::workflow::genesis::provenance::ResearchFinding],
        summary: Option<&ResearchSummary>,
    ) -> Result<Vec<PathBuf>, GenesisError> {
        let research_dir = workspace_root.join(projection_dir).join("research");
        std::fs::create_dir_all(&research_dir)?;
        let mut paths = Vec::new();

        let registry = ResearchDimensionRegistry::global().read().map_err(|_| {
            GenesisError::ResearchDecision("dimension registry lock poisoned".to_string())
        })?;

        for finding in findings {
            if let Some(def) = registry.resolve(&finding.dimension) {
                let file_path = research_dir.join(&def.artifact_filename);
                let mut content = format!("# Research Dimension: {}\n\n", def.id.as_str());
                content.push_str(&finding.summary);
                content.push_str("\n\n");

                if !finding.risks.is_empty() {
                    content.push_str("## Risks\n");
                    for r in &finding.risks {
                        content.push_str(&format!("- Risk: {}\n", r));
                    }
                    content.push('\n');
                }

                if !finding.tradeoffs.is_empty() {
                    content.push_str("## Tradeoffs\n");
                    for t in &finding.tradeoffs {
                        content.push_str(&format!("- Tradeoff: {}\n", t));
                    }
                    content.push('\n');
                }

                if !finding.recommendations.is_empty() {
                    content.push_str("## Recommendations\n");
                    for rec in &finding.recommendations {
                        content.push_str(&format!("- Recommendation: {}\n", rec));
                    }
                    content.push('\n');
                }

                std::fs::write(&file_path, content)?;
                paths.push(file_path);
            }
        }

        if let Some(sum) = summary {
            let summary_path = research_dir.join("SUMMARY.md");
            std::fs::write(&summary_path, sum.to_markdown())?;
            paths.push(summary_path);
        }

        Ok(paths)
    }

    /// Collect actual research findings from the executed workflow on disk.
    ///
    /// Only findings backed by dimension artifacts on disk are returned. A
    /// missing dimension file means no evidence was produced — the runtime
    /// MUST NOT fabricate a finding in its place. An unregistered dimension
    /// id fails explicitly: the collector cannot know its artifact filename
    /// without a definition, and guessing would misattribute evidence.
    pub fn collect_findings(
        workspace_root: &Path,
        projection_dir: &str,
        dimensions: &[super::research_decision::ResearchDimension],
    ) -> Result<Vec<crate::workflow::genesis::provenance::ResearchFinding>, GenesisError> {
        let mut findings = Vec::new();
        let research_dir = workspace_root.join(projection_dir).join("research");
        let registry = ResearchDimensionRegistry::global().read().map_err(|_| {
            GenesisError::ResearchDecision("dimension registry lock poisoned".to_string())
        })?;

        for dim in dimensions {
            let def = registry.resolve(dim).ok_or_else(|| {
                GenesisError::ResearchDecision(format!(
                    "unknown research dimension '{}': no registered dimension definition",
                    dim.as_str()
                ))
            })?;
            let file_path = research_dir.join(&def.artifact_filename);
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                if let Some(finding) = Self::parse_finding(dim, def.id.as_str(), &content) {
                    findings.push(finding);
                }
            } else {
                // No artifact on disk means no evidence: record the absence
                // explicitly instead of fabricating a finding. Callers treat
                // a missing dimension as "not researched".
                tracing::warn!(
                    "research dimension '{}' produced no artifact at {}; recording absence, not a finding",
                    def.id.as_str(),
                    file_path.display()
                );
            }
        }

        Ok(findings)
    }

    /// Execute the research workflow using the provided WorkflowEngine runtime.
    pub async fn execute(
        engine: &WorkflowEngine,
        charter: &ProjectCharter,
        decision: &ResearchDecision,
        options: &GenesisOptions,
        workspace_root: &Path,
    ) -> Result<
        (
            Option<ResearchSummary>,
            Vec<crate::workflow::genesis::provenance::ResearchFinding>,
        ),
        GenesisError,
    > {
        if !decision.execute_research || decision.selected_dimensions.is_empty() {
            return Ok((None, Vec::new()));
        }

        let compiled = Self::compile_research_workflow(charter, decision, options)?;
        let start_req = WorkflowStartRequest::new(workspace_root)
            .with_parameter("project_charter", charter.to_markdown());
        let handle = engine.start_workflow(&compiled, start_req).await?;

        if handle.status == crate::workflow::state::WorkflowRunState::Failed {
            let details =
                if let Ok(step_runs) = engine.repository().list_step_runs(handle.run_id).await {
                    step_runs
                        .into_iter()
                        .filter_map(|s| s.halt_reason.map(|hr| format!("{}: {}", s.step_key, hr)))
                        .collect::<Vec<_>>()
                        .join("; ")
                } else {
                    String::new()
                };
            return Err(GenesisError::ResearchExecution(format!(
                "research workflow model execution failed: run_id={}{}",
                handle.run_id,
                if details.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", details)
                }
            )));
        }

        // Collect actual research findings from the executed workflow
        let findings = Self::collect_findings(
            workspace_root,
            &options.projection_dir,
            &decision.selected_dimensions,
        )?;

        // Synthesize summary using actual findings
        let summary = ResearchSynthesizer::synthesize(charter, &findings)?;

        // Materialize findings and summary to disk by authorized writer
        Self::materialize_research_artifacts(
            workspace_root,
            &options.projection_dir,
            &findings,
            Some(&summary),
        )?;

        Ok((Some(summary), findings))
    }
}
