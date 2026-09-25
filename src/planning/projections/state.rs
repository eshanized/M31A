//! Deterministic one-way STATE.md projection generator (PLN-03, D-16).

use crate::planning::projections::frontmatter::ProjectionFrontmatter;
use crate::planning::projections::renderer::ProjectionRenderer;

/// STATE.md projection generator.
pub struct StateProjection {
    pub frontmatter: ProjectionFrontmatter,
    pub mission_state: String,
    pub current_cycle: u64,
    pub current_stage: Option<String>,
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub pending_escalations: usize,
}

impl StateProjection {
    pub fn new(
        frontmatter: ProjectionFrontmatter,
        mission_state: impl Into<String>,
        current_cycle: u64,
    ) -> Self {
        Self {
            frontmatter,
            mission_state: mission_state.into(),
            current_cycle,
            current_stage: None,
            active_tasks: 0,
            completed_tasks: 0,
            pending_escalations: 0,
        }
    }

    pub fn with_stage(mut self, stage: impl Into<String>) -> Self {
        self.current_stage = Some(stage.into());
        self
    }

    pub fn with_task_counts(mut self, active: usize, completed: usize) -> Self {
        self.active_tasks = active;
        self.completed_tasks = completed;
        self
    }

    pub fn with_escalations(mut self, escalations: usize) -> Self {
        self.pending_escalations = escalations;
        self
    }
}

impl ProjectionRenderer for StateProjection {
    fn render(&self) -> String {
        let mut out = self.frontmatter.render_yaml();
        out.push('\n');
        out.push_str("# Mission Runtime State\n\n");
        out.push_str(&format!("- **Mission State**: `{}`\n", self.mission_state));
        out.push_str(&format!("- **Execution Cycle**: {}\n", self.current_cycle));

        if let Some(ref stage) = self.current_stage {
            out.push_str(&format!("- **Current Controller Stage**: `{}`\n", stage));
        }

        out.push_str(&format!("- **Active Tasks**: {}\n", self.active_tasks));
        out.push_str(&format!(
            "- **Completed Tasks**: {}\n",
            self.completed_tasks
        ));
        out.push_str(&format!(
            "- **Pending Escalations**: {}\n",
            self.pending_escalations
        ));

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;

    #[test]
    fn test_state_projection_rendering() {
        let mid = MissionId::new();
        let fm = ProjectionFrontmatter::new("state", mid, 4);

        let proj = StateProjection::new(fm, "Running", 5)
            .with_stage("ExecuteBoundedWork")
            .with_task_counts(2, 8)
            .with_escalations(0);

        let rendered = proj.render();
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("projection_type: \"state\""));
        assert!(rendered.contains("# Mission Runtime State"));
        assert!(rendered.contains("**Mission State**: `Running`"));

        assert!(rendered.contains("**Execution Cycle**: 5"));
        assert!(rendered.contains("**Current Controller Stage**: `ExecuteBoundedWork`"));
        assert!(rendered.contains("**Active Tasks**: 2"));
        assert!(rendered.contains("**Completed Tasks**: 8"));
    }
}
