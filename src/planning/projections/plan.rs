//! Deterministic one-way PLAN.md projection generator (PLN-03, D-16).

use crate::kernel::plan::CandidatePlan;
use crate::planning::projections::frontmatter::ProjectionFrontmatter;
use crate::planning::projections::renderer::ProjectionRenderer;

/// PLAN.md projection generator.
pub struct PlanProjection {
    pub frontmatter: ProjectionFrontmatter,
    pub plan: CandidatePlan,
}

impl PlanProjection {
    pub fn new(frontmatter: ProjectionFrontmatter, plan: CandidatePlan) -> Self {
        Self { frontmatter, plan }
    }
}

impl ProjectionRenderer for PlanProjection {
    fn render(&self) -> String {
        let mut out = self.frontmatter.render_yaml();
        out.push('\n');
        out.push_str(&format!("# Plan: {}\n\n", self.plan.plan_id));
        out.push_str(&format!("**Objective**: {}\n\n", self.plan.objective));
        out.push_str(&format!(
            "**Total Proposed Tasks**: {}\n\n",
            self.plan.task_count()
        ));

        out.push_str("## Proposed Tasks\n\n");
        for task in &self.plan.tasks {
            out.push_str(&format!("### Task `{}`: {}\n\n", task.id, task.objective));
            out.push_str(&format!("- **Assigned Role**: `{:?}`\n", task.role));
            out.push_str(&format!("- **Verification**: `{:?}`\n", task.verification));
            out.push_str(&format!(
                "- **Estimates**: max_steps={}, max_duration_secs={}s, max_tokens={}, max_cost_usd=${:.2}\n",
                task.estimates.max_steps,
                task.estimates.max_duration_secs,
                task.estimates.max_tokens,
                task.estimates.max_cost_usd
            ));

            if !task.depends_on.is_empty() {
                out.push_str("- **Depends On**: ");
                let deps: Vec<String> =
                    task.depends_on.iter().map(|d| format!("`{}`", d)).collect();
                out.push_str(&deps.join(", "));
                out.push('\n');
            }

            if !task.capabilities.is_empty() {
                out.push_str("- **Capabilities**: ");
                let caps: Vec<String> = task
                    .capabilities
                    .iter()
                    .map(|c| format!("`{}:{}`", c.id, c.mode))
                    .collect();
                out.push_str(&caps.join(", "));
                out.push('\n');
            }

            if let Some(ref desc) = task.description {
                out.push_str(&format!("\n{}\n", desc));
            }

            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;
    use crate::kernel::plan::{
        CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
        ResourceEstimate, VerificationStrategy,
    };
    use crate::state_machine::agent::AgentRole;

    #[test]
    fn test_plan_projection_rendering() {
        let mid = MissionId::new();
        let fm = ProjectionFrontmatter::new("plan", mid, 3);

        let task = CandidateTask {
            id: CandidateTaskKey::new("TASK-01"),
            objective: "Initial bootstrap".to_string(),
            description: Some("Implement kernel primitives".to_string()),
            depends_on: vec![],
            capabilities: vec![CapabilityRequirement::new(
                "fs.write",
                CapabilityAccessMode::Write,
            )],
            role: AgentRole::implementer(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::new(5, 60, 5000, 0.10),
            ..Default::default()
        };

        let plan = CandidatePlan::new("PLAN-42", "Bootstrap Mission Plan", vec![task]);
        let proj = PlanProjection::new(fm, plan);
        let rendered = proj.render();

        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("projection_type: \"plan\""));
        assert!(rendered.contains("# Plan: PLAN-42"));
        assert!(rendered.contains("Initial bootstrap"));
        assert!(rendered.contains("`TASK-01`"));
        assert!(rendered.contains("`fs.write:write`"));
    }
}
