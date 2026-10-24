//! Canonical Markdown document architecture with validated YAML front matter.
//!
//! Provides a shared, authoritative parser and serializer for all M31A-authored
//! definitions and configuration documents:
//! - Prompts (`prompts/`)
//! - Configuration & Settings (`config.md`, `settings.md`)
//! - Profiles (`assets/profiles/`)
//! - Policies (`policy.md`, `organization.md`)
//! - Skills (`assets/skills/*/skill.md`)
//! - Verification Adapters (`assets/verification_adapters/`)
//! - User-defined slash commands (`prompts/commands/`)
//! - Native workflow manifests (`*.md`)

pub mod envelope;
pub mod error;
pub mod frontmatter;
pub mod serializer;

pub use envelope::{CURRENT_SCHEMA_VERSION, DocumentEnvelope, DocumentKind, MarkdownDocument};
pub use error::DocumentError;
pub use frontmatter::{
    MAX_DOCUMENT_SIZE_BYTES, MAX_FRONT_MATTER_SIZE_BYTES, parse_markdown_document,
    parse_markdown_value, split_frontmatter,
};
pub use serializer::{atomic_write_document, serialize_markdown_document};

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SampleData {
        id: String,
        version: u32,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        enabled: bool,
    }

    #[test]
    fn test_valid_markdown_with_typed_front_matter() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data:
  id: sample.test
  version: 2
  tags:
    - alpha
    - beta
  enabled: true
---

# Template Header
Hello {{ username }}!
"#;
        let doc = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap();
        assert_eq!(doc.kind, DocumentKind::Prompt);
        assert_eq!(doc.schema_version, 1);
        assert_eq!(doc.data.id, "sample.test");
        assert_eq!(doc.data.version, 2);
        assert_eq!(doc.data.tags, vec!["alpha", "beta"]);
        assert!(doc.data.enabled);
        assert_eq!(doc.body, "# Template Header\nHello {{ username }}!\n");
    }

    #[test]
    fn test_missing_opening_delimiter() {
        let doc_str = r#"kind: prompt
schema_version: 1
data:
  id: test
  version: 1
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::MissingOpeningDelimiter));
    }

    #[test]
    fn test_missing_closing_delimiter() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data:
  id: test
  version: 1
Body without delimiter
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::MissingClosingDelimiter));
    }

    #[test]
    fn test_empty_front_matter() {
        let doc_str = r#"---
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::EmptyFrontMatter));
    }

    #[test]
    fn test_empty_optional_body() {
        let doc_str = r#"---
kind: config
schema_version: 1
data:
  id: cfg
  version: 1
---
"#;
        let doc = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Config).unwrap();
        assert_eq!(doc.body, "");
    }

    #[test]
    fn test_incorrect_document_kind() {
        let doc_str = r#"---
kind: profile
schema_version: 1
data:
  id: test
  version: 1
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::KindMismatch { .. }));
    }

    #[test]
    fn test_unsupported_schema_version() {
        let doc_str = r#"---
kind: prompt
schema_version: 99
data:
  id: test
  version: 1
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(
            err,
            DocumentError::UnsupportedSchemaVersion { .. }
        ));
    }

    #[test]
    fn test_unknown_fields_rejected() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data:
  id: test
  version: 1
  malicious_unrecognized_field: true
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::ValidationError(_)));
    }

    #[test]
    fn test_duplicate_yaml_keys_rejected() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data:
  id: test
  version: 1
  id: duplicated_id
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::DuplicateKey { key } if key == "id"));
    }

    #[test]
    fn test_invalid_yaml_syntax() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data: [unclosed_list
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::YamlError(_)));
    }

    #[test]
    fn test_minijinja_interpolation_preserved_literally() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data:
  id: prompt.test
  version: 1
---
{% if condition %}
  Hello {{ target | filter }}
{% endif %}
"#;
        let doc = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap();
        assert_eq!(
            doc.body,
            "{% if condition %}\n  Hello {{ target | filter }}\n{% endif %}\n"
        );
    }

    #[test]
    fn test_crlf_newline_handling() {
        let doc_str = "---\r\nkind: prompt\r\nschema_version: 1\r\ndata:\r\n  id: test\r\n  version: 1\r\n---\r\nLine 1\r\nLine 2";
        let doc = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap();
        assert_eq!(doc.data.id, "test");
        assert_eq!(doc.body, "Line 1\r\nLine 2");
    }

    #[test]
    fn test_round_trip_serialization() {
        let original = MarkdownDocument::new(
            DocumentKind::Skill,
            1,
            SampleData {
                id: "test.skill".to_string(),
                version: 3,
                tags: vec!["audit".to_string()],
                enabled: true,
            },
            "# Instructions\nPerform step 1.\n",
        );

        let serialized = serialize_markdown_document(&original).unwrap();
        let parsed =
            parse_markdown_document::<SampleData>(&serialized, DocumentKind::Skill).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn test_yaml_anchor_limit_security() {
        let mut yaml =
            String::from("---\nkind: prompt\nschema_version: 1\ndata:\n  id: test\n  version: 1\n");
        for i in 0..40 {
            yaml.push_str(&format!("  anchor_{}: &a{} val\n", i, i));
        }
        yaml.push_str("---\nBody\n");
        let err = parse_markdown_document::<SampleData>(&yaml, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::SecurityViolation { .. }));
    }

    #[test]
    fn test_yaml_custom_tag_rejected() {
        let doc_str = r#"---
kind: prompt
schema_version: 1
data: !custom_tag
  id: test
  version: 1
---
Body
"#;
        let err = parse_markdown_document::<SampleData>(doc_str, DocumentKind::Prompt).unwrap_err();
        assert!(matches!(err, DocumentError::SecurityViolation { .. }));
    }
}
