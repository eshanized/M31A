//! Dual-Mode Skill Execution Compiler (SKL-01, SKL-03, D-10).
//!
//! Compiles declarative skills into either:
//! 1. Sub-DAG task fragments admitted through scheduler dependency validation
//! 2. In-task procedural prompt guidance within single-step agent contexts

use std::collections::HashMap;

use crate::ids::TaskId;
use crate::skill::manifest::{SkillExecutionMode, SkillManifest};

/// A discrete task node generated during Sub-DAG compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSubTask {
    pub task_id: TaskId,
    pub name: String,
    pub instruction: String,
    pub agent_role: String,
    pub allowed_tools: Vec<String>,
    pub dependencies: Vec<TaskId>,
}

/// A validated DAG fragment resulting from Sub-DAG skill compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSubDag {
    pub parent_task_id: TaskId,
    pub skill_id: String,
    pub tasks: Vec<CompiledSubTask>,
    pub edges: Vec<(TaskId, TaskId)>,
}

/// Compiler transforming declarative skills into executable runtime proposals.
pub struct SkillCompiler;

impl SkillCompiler {
    /// Compile a multi-step skill into a candidate Sub-DAG fragment.
    pub fn compile_sub_dag(
        manifest: &SkillManifest,
        parent_task_id: TaskId,
    ) -> Result<CompiledSubDag, String> {
        if manifest.execution.mode != SkillExecutionMode::SubDag {
            return Err(format!(
                "Skill '{}' execution mode is not sub_dag",
                manifest.id
            ));
        }

        let mut name_to_id: HashMap<String, TaskId> = HashMap::new();
        let mut subtasks = Vec::new();
        let mut edges = Vec::new();

        // 1. Assign TaskIds to all steps
        for step in &manifest.procedure.steps {
            name_to_id.insert(step.name.clone(), TaskId::new());
        }

        // 2. Build tasks and resolve dependencies
        let mut previous_id: Option<TaskId> = None;
        for step in &manifest.procedure.steps {
            let task_id = *name_to_id.get(&step.name).unwrap();
            let mut dep_ids = Vec::new();

            if !step.dependencies.is_empty() {
                for dep_name in &step.dependencies {
                    let dep_id = name_to_id.get(dep_name).ok_or_else(|| {
                        format!(
                            "Unknown step dependency '{}' in step '{}'",
                            dep_name, step.name
                        )
                    })?;
                    dep_ids.push(*dep_id);
                    edges.push((*dep_id, task_id));
                }
            } else if let Some(prev) = previous_id {
                // Default to sequential dependency if none declared
                dep_ids.push(prev);
                edges.push((prev, task_id));
            }

            let role = step
                .agent_role
                .clone()
                .unwrap_or_else(|| "coder".to_string());

            subtasks.push(CompiledSubTask {
                task_id,
                name: step.name.clone(),
                instruction: step.instruction.clone(),
                agent_role: role,
                allowed_tools: step.allowed_tools.clone(),
                dependencies: dep_ids,
            });

            previous_id = Some(task_id);
        }

        Ok(CompiledSubDag {
            parent_task_id,
            skill_id: manifest.id.clone(),
            tasks: subtasks,
            edges,
        })
    }

    /// Compile a focused skill into structured in-task procedural XML guidance using a PromptCatalog.
    pub fn compile_in_task_guidance_with_catalog(
        manifest: &SkillManifest,
        catalog: &impl crate::prompt::catalog::PromptCatalog,
    ) -> Result<String, crate::prompt::error::PromptError> {
        let contract = catalog.get("skill.in_task_guidance", 1)?;

        let mut steps_block = String::new();
        if !manifest.procedure.steps.is_empty() {
            steps_block.push_str("  <steps>\n");
            for (idx, step) in manifest.procedure.steps.iter().enumerate() {
                steps_block.push_str(&format!(
                    "    <step seq=\"{}\" name=\"{}\">\n",
                    idx + 1,
                    step.name
                ));
                steps_block.push_str(&format!(
                    "      <instruction>{}</instruction>\n",
                    step.instruction.trim()
                ));
                if !step.allowed_tools.is_empty() {
                    steps_block.push_str(&format!(
                        "      <allowed_tools>{}</allowed_tools>\n",
                        step.allowed_tools.join(", ")
                    ));
                }
                steps_block.push_str("    </step>\n");
            }
            steps_block.push_str("  </steps>\n");
        }

        let mut verification_block = String::new();
        for cmd in &manifest.verification.commands {
            verification_block.push_str(&format!("    <command>{}</command>\n", cmd));
        }
        for ev in &manifest.verification.evidence_required {
            verification_block.push_str(&format!(
                "    <evidence_required>{}</evidence_required>\n",
                ev
            ));
        }

        let mut params = std::collections::BTreeMap::new();
        params.insert("skill_id".to_string(), manifest.id.clone());
        params.insert("skill_name".to_string(), manifest.name.clone());
        params.insert(
            "instructions".to_string(),
            manifest.procedure.instructions.trim().to_string(),
        );
        params.insert(
            "verification_tier".to_string(),
            manifest.verification.tier.to_string(),
        );
        params.insert("steps_block".to_string(), steps_block);
        params.insert("verification_block".to_string(), verification_block);

        let rendered = crate::prompt::renderer::render_prompt(contract, &params, false)?;
        Ok(rendered.rendered_text)
    }

    /// Compile a focused skill into structured in-task procedural XML guidance using default built-in catalog.
    pub fn compile_in_task_guidance(manifest: &SkillManifest) -> String {
        let catalog = crate::prompt::catalog::InMemoryPromptCatalog::with_builtins();
        Self::compile_in_task_guidance_with_catalog(manifest, &catalog).unwrap_or_else(|e| {
            tracing::warn!(
                "failed to compile skill in-task guidance from catalog: {}",
                e
            );
            format!(
                "<skill_procedure id=\"{}\" name=\"{}\">\n  <instructions>{}\n  </instructions>\n</skill_procedure>",
                manifest.id, manifest.name, manifest.procedure.instructions.trim()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::manifest::{
        SkillExecutionConfig, SkillProcedure, SkillRiskLevel, SkillRiskProfile,
        SkillStepDefinition, SkillVerificationSpec,
    };
    use semver::Version;

    #[test]
    fn test_compile_sub_dag_generates_tasks_and_edges() {
        let manifest = SkillManifest {
            schema_version: 1,
            id: "multi-step".to_string(),
            name: "Multi Step".to_string(),
            version: Version::new(1, 0, 0),
            description: "test".to_string(),
            required_capabilities: vec![],
            input_schema: serde_json::json!({}),
            execution: SkillExecutionConfig {
                mode: SkillExecutionMode::SubDag,
            },
            procedure: SkillProcedure {
                instructions: "execute sequentially".to_string(),
                steps: vec![
                    SkillStepDefinition {
                        name: "step_a".to_string(),
                        agent_role: Some("coder".to_string()),
                        instruction: "Do A".to_string(),
                        allowed_tools: vec![],
                        dependencies: vec![],
                    },
                    SkillStepDefinition {
                        name: "step_b".to_string(),
                        agent_role: Some("reviewer".to_string()),
                        instruction: "Do B".to_string(),
                        allowed_tools: vec![],
                        dependencies: vec!["step_a".to_string()],
                    },
                ],
            },
            verification: SkillVerificationSpec {
                tier: 3,
                commands: vec![],
                evidence_required: vec![],
            },
            risk_profile: SkillRiskProfile {
                level: SkillRiskLevel::Low,
                requires_approval: false,
            },
        };

        let parent_id = TaskId::new();
        let subdag = SkillCompiler::compile_sub_dag(&manifest, parent_id).unwrap();

        assert_eq!(subdag.tasks.len(), 2);
        assert_eq!(subdag.edges.len(), 1);
        assert_eq!(subdag.tasks[1].dependencies[0], subdag.tasks[0].task_id);
    }

    #[test]
    fn test_compile_in_task_guidance() {
        let manifest = SkillManifest {
            schema_version: 1,
            id: "guided-skill".to_string(),
            name: "Guided Skill".to_string(),
            version: Version::new(1, 0, 0),
            description: "test".to_string(),
            required_capabilities: vec![],
            input_schema: serde_json::json!({}),
            execution: SkillExecutionConfig {
                mode: SkillExecutionMode::InTask,
            },
            procedure: SkillProcedure {
                instructions: "Follow these rules carefully".to_string(),
                steps: vec![SkillStepDefinition {
                    name: "step_1".to_string(),
                    agent_role: None,
                    instruction: "Execute step 1".to_string(),
                    allowed_tools: vec!["tool_a".to_string()],
                    dependencies: vec![],
                }],
            },
            verification: SkillVerificationSpec {
                tier: 2,
                commands: vec!["cargo check".to_string()],
                evidence_required: vec!["check.log".to_string()],
            },
            risk_profile: SkillRiskProfile {
                level: SkillRiskLevel::Low,
                requires_approval: false,
            },
        };

        let guidance = SkillCompiler::compile_in_task_guidance(&manifest);
        assert!(guidance.contains("<skill_procedure id=\"guided-skill\""));
        assert!(guidance.contains("<instruction>Execute step 1</instruction>"));
        assert!(guidance.contains("<command>cargo check</command>"));
    }
}
