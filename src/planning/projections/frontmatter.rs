//! Standardized YAML frontmatter envelope for planning projections (PLN-03, D-16).

use crate::ids::MissionId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Frontmatter metadata accompanying all one-way planning markdown projections (D-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionFrontmatter {
    pub m31a_schema_version: String,
    pub projection_type: String,
    pub mission_id: MissionId,
    pub source_state_sequence: u64,
    pub generated_at: DateTime<Utc>,
    pub projection_schema_version: String,
}

impl ProjectionFrontmatter {
    pub fn new(
        projection_type: impl Into<String>,
        mission_id: MissionId,
        source_state_sequence: u64,
    ) -> Self {
        Self {
            m31a_schema_version: "1.0".to_string(),
            projection_type: projection_type.into(),
            mission_id,
            source_state_sequence,
            generated_at: Utc::now(),
            projection_schema_version: "1.0".to_string(),
        }
    }

    /// Renders the frontmatter enclosed within deterministic YAML fences `---`.
    pub fn render_yaml(&self) -> String {
        format!(
            "---\nm31a_schema_version: \"{}\"\nprojection_type: \"{}\"\nmission_id: \"{}\"\nsource_state_sequence: {}\ngenerated_at: \"{}\"\nprojection_schema_version: \"{}\"\n---\n",
            self.m31a_schema_version,
            self.projection_type,
            self.mission_id,
            self.source_state_sequence,
            self.generated_at.to_rfc3339(),
            self.projection_schema_version,
        )
    }

    /// Parses frontmatter from a markdown string beginning with `---`.
    pub fn parse_from_markdown(content: &str) -> Result<Self, String> {
        let trimmed = content.trim_start();
        if !trimmed.starts_with("---") {
            return Err("Missing opening '---' frontmatter delimiter".to_string());
        }

        let rest = &trimmed[3..];
        let end_idx = rest
            .find("\n---")
            .ok_or_else(|| "Missing closing '---' frontmatter delimiter".to_string())?;
        let block = &rest[..end_idx];

        let mut m31a_schema_version = None;
        let mut projection_type = None;
        let mut mission_id = None;
        let mut source_state_sequence = None;
        let mut generated_at = None;
        let mut projection_schema_version = None;

        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim();
                let val = v.trim().trim_matches('"');
                match key {
                    "m31a_schema_version" => m31a_schema_version = Some(val.to_string()),
                    "projection_type" => projection_type = Some(val.to_string()),
                    "mission_id" => {
                        let id: MissionId = val
                            .parse()
                            .map_err(|e| format!("Invalid mission_id: {}", e))?;
                        mission_id = Some(id);
                    }
                    "source_state_sequence" => {
                        let seq: u64 = val
                            .parse()
                            .map_err(|e| format!("Invalid sequence: {}", e))?;
                        source_state_sequence = Some(seq);
                    }
                    "generated_at" => {
                        let dt = DateTime::parse_from_rfc3339(val)
                            .map_err(|e| format!("Invalid timestamp: {}", e))?
                            .with_timezone(&Utc);
                        generated_at = Some(dt);
                    }
                    "projection_schema_version" => {
                        projection_schema_version = Some(val.to_string())
                    }
                    _ => {}
                }
            }
        }

        Ok(Self {
            m31a_schema_version: m31a_schema_version.ok_or("Missing m31a_schema_version")?,
            projection_type: projection_type.ok_or("Missing projection_type")?,
            mission_id: mission_id.ok_or("Missing mission_id")?,
            source_state_sequence: source_state_sequence.ok_or("Missing source_state_sequence")?,
            generated_at: generated_at.ok_or("Missing generated_at")?,
            projection_schema_version: projection_schema_version
                .ok_or("Missing projection_schema_version")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontmatter_render_and_parse_roundtrip() {
        let mid = MissionId::new();
        let fm = ProjectionFrontmatter::new("mission", mid, 42);

        let rendered = fm.render_yaml();
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("\n---\n"));

        let doc = format!("{}\n# Mission Body\nHello world", rendered);
        let parsed =
            ProjectionFrontmatter::parse_from_markdown(&doc).expect("parse markdown frontmatter");

        assert_eq!(fm.m31a_schema_version, parsed.m31a_schema_version);
        assert_eq!(fm.projection_type, parsed.projection_type);
        assert_eq!(fm.mission_id, parsed.mission_id);
        assert_eq!(fm.source_state_sequence, parsed.source_state_sequence);
        assert_eq!(
            fm.projection_schema_version,
            parsed.projection_schema_version
        );
    }
}
