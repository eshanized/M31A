//! Deterministic one-way MISSION.md projection generator (PLN-03, D-16).

use crate::planning::projections::frontmatter::ProjectionFrontmatter;
use crate::planning::projections::renderer::ProjectionRenderer;
use crate::state::intake::AutonomyMode;

/// MISSION.md projection generator.
pub struct MissionProjection {
    pub frontmatter: ProjectionFrontmatter,
    pub objective: String,
    pub autonomy_mode: Option<AutonomyMode>,
    pub constraints: Vec<String>,
    pub success_criteria: Vec<String>,
}

impl MissionProjection {
    pub fn new(frontmatter: ProjectionFrontmatter, objective: impl Into<String>) -> Self {
        Self {
            frontmatter,
            objective: objective.into(),
            autonomy_mode: None,
            constraints: Vec::new(),
            success_criteria: Vec::new(),
        }
    }

    pub fn with_autonomy_mode(mut self, mode: AutonomyMode) -> Self {
        self.autonomy_mode = Some(mode);
        self
    }

    pub fn with_constraints(mut self, constraints: Vec<String>) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_success_criteria(mut self, criteria: Vec<String>) -> Self {
        self.success_criteria = criteria;
        self
    }
}

impl ProjectionRenderer for MissionProjection {
    fn render(&self) -> String {
        let mut out = self.frontmatter.render_yaml();
        out.push('\n');
        out.push_str(&format!("# Mission: {}\n\n", self.objective));

        if let Some(mode) = self.autonomy_mode {
            out.push_str(&format!("- **Autonomy Mode**: `{:?}`\n", mode));
        }

        if !self.constraints.is_empty() {
            out.push_str("\n## Repository Constraints\n\n");
            for c in &self.constraints {
                out.push_str(&format!("- {}\n", c));
            }
        }

        if !self.success_criteria.is_empty() {
            out.push_str("\n## Success Criteria\n\n");
            for sc in &self.success_criteria {
                out.push_str(&format!("- {}\n", sc));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;

    #[test]
    fn test_mission_projection_rendering() {
        let mid = MissionId::new();
        let fm = ProjectionFrontmatter::new("mission", mid, 1);
        let proj = MissionProjection::new(fm, "Build M31 Autonomous Core")
            .with_autonomy_mode(AutonomyMode::Safe)
            .with_constraints(vec!["Rust 2024 single-crate".to_string()])
            .with_success_criteria(vec!["Pass all tests".to_string()]);

        let rendered = proj.render();
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("projection_type: \"mission\""));
        assert!(rendered.contains("# Mission: Build M31 Autonomous Core"));
        assert!(rendered.contains("**Autonomy Mode**: `Safe`"));

        assert!(rendered.contains("Rust 2024 single-crate"));
        assert!(rendered.contains("Pass all tests"));
    }
}
