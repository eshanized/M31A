//! Planning artifact projections (PLN-03, D-13..D-16).
//!
//! Generates deterministic one-way markdown projections for MISSION.md, REQUIREMENTS.md,
//! PLAN.md, and STATE.md, housed under `.m31a/missions/<id>/projections/`.

pub mod frontmatter;
pub mod mission;
pub mod plan;
pub mod renderer;
pub mod requirements;
pub mod state;

pub use frontmatter::ProjectionFrontmatter;
pub use mission::MissionProjection;
pub use plan::PlanProjection;
pub use renderer::ProjectionRenderer;
pub use requirements::RequirementsProjection;
pub use state::StateProjection;

use crate::ids::MissionId;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves the canonical projections directory for a mission (D-14).
pub fn mission_projections_dir(storage_root: &Path, mission_id: MissionId) -> PathBuf {
    let base = if storage_root.ends_with(".m31a") {
        storage_root.to_path_buf()
    } else {
        storage_root.join(".m31a")
    };
    base.join("missions")
        .join(mission_id.to_string())
        .join("projections")
}

/// Evaluates if a projection's state sequence is stale compared to authoritative state (D-15).
pub fn is_projection_stale(projection_sequence: u64, authoritative_sequence: u64) -> bool {
    projection_sequence < authoritative_sequence
}

/// Inspects a projection file on disk and detects if its frontmatter sequence is stale (D-15).
pub fn check_file_staleness(file_path: &Path, authoritative_sequence: u64) -> Result<bool, String> {
    let content = fs::read_to_string(file_path).map_err(|e| {
        format!(
            "Failed to read projection file {}: {}",
            file_path.display(),
            e
        )
    })?;

    let fm = ProjectionFrontmatter::parse_from_markdown(&content)?;
    Ok(is_projection_stale(
        fm.source_state_sequence,
        authoritative_sequence,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_mission_projections_dir_layout() {
        let dir = tempdir().unwrap();
        let mid = MissionId::new();
        let expected = dir
            .path()
            .join(".m31a")
            .join("missions")
            .join(mid.to_string())
            .join("projections");

        let resolved = mission_projections_dir(dir.path(), mid);
        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_mission_projections_dir_already_in_m31a() {
        let dir = tempdir().unwrap();
        let m31a_dir = dir.path().join(".m31a");
        let mid = MissionId::new();
        let expected = m31a_dir
            .join("missions")
            .join(mid.to_string())
            .join("projections");

        let resolved = mission_projections_dir(&m31a_dir, mid);
        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_staleness_detection() {
        assert!(is_projection_stale(1, 2));
        assert!(!is_projection_stale(2, 2));
        assert!(!is_projection_stale(3, 2));

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("PLAN.md");

        let mid = MissionId::new();
        let fm = ProjectionFrontmatter::new("plan", mid, 5);
        let content = format!("{}\n# Body", fm.render_yaml());
        fs::write(&file_path, content).unwrap();

        // Sequence 5 vs Authoritative 5 -> Not stale
        assert_eq!(check_file_staleness(&file_path, 5), Ok(false));

        // Sequence 5 vs Authoritative 6 -> Stale
        assert_eq!(check_file_staleness(&file_path, 6), Ok(true));
    }
}
