//! Deterministic one-way REQUIREMENTS.md projection generator (PLN-03, D-16).

use crate::planning::projections::frontmatter::ProjectionFrontmatter;
use crate::planning::projections::renderer::ProjectionRenderer;
use crate::planning::requirements::EngineeringRequirement;

/// REQUIREMENTS.md projection generator.
pub struct RequirementsProjection {
    pub frontmatter: ProjectionFrontmatter,
    pub requirements: Vec<EngineeringRequirement>,
}

impl RequirementsProjection {
    pub fn new(
        frontmatter: ProjectionFrontmatter,
        requirements: Vec<EngineeringRequirement>,
    ) -> Self {
        Self {
            frontmatter,
            requirements,
        }
    }
}

impl ProjectionRenderer for RequirementsProjection {
    fn render(&self) -> String {
        let mut out = self.frontmatter.render_yaml();
        out.push('\n');
        out.push_str("# Engineering Requirements\n\n");

        if self.requirements.is_empty() {
            out.push_str("No engineering requirements registered.\n");
            return out;
        }

        out.push_str("| ID | Description | Epistemic Status | Trust Level | Source |\n");
        out.push_str("|---|---|---|---|---|\n");

        for req in &self.requirements {
            let loc = req.provenance.location.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| `{}` | {} | `{}` | `{:?}` | {} |\n",
                req.key, req.description, req.current_status, req.provenance.trust_level, loc,
            ));
        }

        out.push('\n');
        for req in &self.requirements {
            if !req.satisfaction_criteria.is_empty() {
                out.push_str(&format!("### Criteria for `{}`\n", req.key));
                for c in &req.satisfaction_criteria {
                    out.push_str(&format!("- {}\n", c));
                }
                out.push('\n');
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;
    use crate::planning::requirements::{
        EpistemicStatus, Provenance, ProvenanceSourceType, TrustLevel,
    };

    #[test]
    fn test_requirements_projection_rendering() {
        let mid = MissionId::new();
        let fm = ProjectionFrontmatter::new("requirements", mid, 2);

        let prov = Provenance::new(
            ProvenanceSourceType::RepositoryFile,
            TrustLevel::VerifiedRepository,
            "intake_engine",
        )
        .with_location("Cargo.toml");

        let req = EngineeringRequirement::new(
            "PLN-01",
            "Planning subsystem domain decomposition",
            EpistemicStatus::VerifiedFact,
            prov,
            vec!["Validate candidate plans".to_string()],
        );

        let proj = RequirementsProjection::new(fm, vec![req]);
        let rendered = proj.render();

        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("projection_type: \"requirements\""));
        assert!(rendered.contains("# Engineering Requirements"));
        assert!(rendered.contains("`PLN-01`"));
        assert!(rendered.contains("Planning subsystem domain decomposition"));
        assert!(rendered.contains("`verified_fact`"));
        assert!(rendered.contains("Validate candidate plans"));
    }
}
