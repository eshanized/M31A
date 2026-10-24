//! Authority front-matter parser with strict envelope and security validation.

use std::collections::{HashMap, HashSet};

use crate::document::envelope::{CURRENT_SCHEMA_VERSION, DocumentKind, MarkdownDocument};
use crate::document::error::DocumentError;

/// Maximum permissible byte size for any document (5 MB).
pub const MAX_DOCUMENT_SIZE_BYTES: usize = 5 * 1024 * 1024;

/// Maximum permissible byte size for front-matter block (1 MB).
pub const MAX_FRONT_MATTER_SIZE_BYTES: usize = 1024 * 1024;

/// Maximum allowed YAML anchors and aliases to prevent exponential expansion attacks.
pub const MAX_YAML_ALIASES: usize = 32;

/// Maximum nesting depth for YAML mappings and sequences.
pub const MAX_YAML_DEPTH: usize = 32;

/// Splits raw Markdown document into its leading front-matter block and Markdown body.
pub fn split_frontmatter(content: &str) -> Result<(&str, &str), DocumentError> {
    if content.len() > MAX_DOCUMENT_SIZE_BYTES {
        return Err(DocumentError::DocumentSizeExceeded {
            actual: content.len(),
            max: MAX_DOCUMENT_SIZE_BYTES,
        });
    }

    // Strip optional UTF-8 BOM
    let trimmed_start = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    // Document must start with opening '---' delimiter
    if !trimmed_start.starts_with("---") {
        return Err(DocumentError::MissingOpeningDelimiter);
    }

    // Ensure the opening delimiter is on its own line
    let rest = &trimmed_start[3..];
    let (after_opening, opening_len) = if let Some(stripped) = rest.strip_prefix("\r\n") {
        (stripped, 5) // "---" + "\r\n"
    } else if let Some(stripped) = rest.strip_prefix('\n') {
        (stripped, 4) // "---" + "\n"
    } else {
        // Might have trailing spaces on first line before newline
        let mut idx = 0;
        let bytes = rest.as_bytes();
        while idx < bytes.len() && (bytes[idx] == b' ' || bytes[idx] == b'\t') {
            idx += 1;
        }
        if idx < bytes.len()
            && bytes[idx] == b'\r'
            && idx + 1 < bytes.len()
            && bytes[idx + 1] == b'\n'
        {
            (&rest[idx + 2..], 3 + idx + 2)
        } else if idx < bytes.len() && bytes[idx] == b'\n' {
            (&rest[idx + 1..], 3 + idx + 1)
        } else {
            return Err(DocumentError::MissingOpeningDelimiter);
        }
    };

    let _ = opening_len;

    // Search for closing '---' or '...' delimiter line
    let mut current_offset = 0;
    let mut closing_found: Option<(usize, usize)> = None; // (start_of_closing, end_of_closing_line)

    for line in after_opening.split_inclusive('\n') {
        let line_trimmed = line.trim_end_matches(&['\r', '\n'][..]).trim_end();
        if line_trimmed == "---" || line_trimmed == "..." {
            closing_found = Some((current_offset, current_offset + line.len()));
            break;
        }
        current_offset += line.len();
    }

    let (closing_start, closing_end) =
        closing_found.ok_or(DocumentError::MissingClosingDelimiter)?;

    let front_matter = &after_opening[..closing_start];
    if front_matter.len() > MAX_FRONT_MATTER_SIZE_BYTES {
        return Err(DocumentError::FrontMatterSizeExceeded {
            actual: front_matter.len(),
            max: MAX_FRONT_MATTER_SIZE_BYTES,
        });
    }

    if front_matter.trim().is_empty() {
        return Err(DocumentError::EmptyFrontMatter);
    }

    let body = &after_opening[closing_end..];
    // Strip a single leading newline separator if present
    let body = if let Some(stripped) = body.strip_prefix("\r\n") {
        stripped
    } else if let Some(stripped) = body.strip_prefix('\n') {
        stripped
    } else {
        body
    };

    Ok((front_matter, body))
}

/// Detects duplicate keys in YAML mappings at identical nesting levels.
pub fn check_duplicate_yaml_keys(yaml_str: &str) -> Result<(), DocumentError> {
    // Track keys per nesting level: scope stack storing seen keys
    let mut scope_keys: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut prev_indent = 0;

    for line in yaml_str.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            continue;
        }

        // Compute leading whitespace indentation
        let indent = line.len() - line.trim_start().len();

        let is_sequence_item = trimmed_line.starts_with("- ");
        if is_sequence_item {
            // A new list item resets the mapping scope at or deeper than this item's indentation
            scope_keys.retain(|&k, _| k < indent);
        } else if indent < prev_indent {
            // If indent decreased, clear scopes deeper than current indent
            scope_keys.retain(|&k, _| k <= indent);
        }
        prev_indent = indent;

        // Check if line defines a mapping key: `key:` or `- key:`
        let content = if let Some(after_dash) = trimmed_line.strip_prefix("- ") {
            after_dash.trim()
        } else {
            trimmed_line
        };

        if let Some(colon_pos) = content.find(':') {
            let candidate_key = content[..colon_pos].trim();
            // Verify candidate_key is a valid key (not inside quotes or a URL)
            let is_key = !candidate_key.is_empty()
                && !candidate_key.starts_with('#')
                && !candidate_key.contains(' ')
                && (content.len() == colon_pos + 1
                    || content[colon_pos + 1..].starts_with(' ')
                    || content[colon_pos + 1..].starts_with('\t'));

            if is_key {
                let clean_key = candidate_key.trim_matches(&['"', '\''][..]).to_string();
                let key_col = line.find(candidate_key).unwrap_or(indent);
                let keys = scope_keys.entry(key_col).or_default();
                if !keys.insert(clean_key.clone()) {
                    return Err(DocumentError::DuplicateKey { key: clean_key });
                }
            }
        }
    }

    Ok(())
}

/// Inspects front-matter YAML for security risks (anchor denial-of-service, custom tags, excessive depth).
pub fn check_yaml_security(yaml_str: &str) -> Result<(), DocumentError> {
    let mut anchor_count = 0;
    let mut alias_count = 0;

    for token in yaml_str.split_whitespace() {
        if token.starts_with('&') && token.len() > 1 {
            anchor_count += 1;
        } else if token.starts_with('*') && token.len() > 1 {
            alias_count += 1;
        } else if token.starts_with('!') && token.len() > 1 {
            // Reject custom tags like !ruby/object or !cmd
            if !token.starts_with("!!") {
                return Err(DocumentError::SecurityViolation {
                    reason: format!("Disallowed custom YAML tag '{token}'"),
                });
            }
        }
    }

    if anchor_count + alias_count > MAX_YAML_ALIASES {
        return Err(DocumentError::SecurityViolation {
            reason: format!(
                "Excessive YAML anchors/aliases ({}) exceeds safety limit ({})",
                anchor_count + alias_count,
                MAX_YAML_ALIASES
            ),
        });
    }

    // Check nesting depth
    let mut max_indent = 0;
    for line in yaml_str.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent > max_indent {
            max_indent = indent;
        }
    }

    // 2 spaces per level standard -> depth = max_indent / 2
    if max_indent / 2 > MAX_YAML_DEPTH {
        return Err(DocumentError::SecurityViolation {
            reason: format!(
                "YAML nesting depth ({}) exceeds safety limit ({})",
                max_indent / 2,
                MAX_YAML_DEPTH
            ),
        });
    }

    Ok(())
}

/// Parses a Markdown document string with YAML front matter into a typed [`MarkdownDocument<T>`].
pub fn parse_markdown_document<T: serde::de::DeserializeOwned>(
    content: &str,
    expected_kind: DocumentKind,
) -> Result<MarkdownDocument<T>, DocumentError> {
    let (front_matter, body) = split_frontmatter(content)?;

    // Run security and structural checks on front-matter YAML
    check_yaml_security(front_matter)?;
    check_duplicate_yaml_keys(front_matter)?;

    // Intermediate untyped envelope to validate kind and schema version strictly
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawEnvelope {
        kind: DocumentKind,
        schema_version: u32,
        data: serde_yaml::Value,
    }

    let raw: RawEnvelope =
        serde_yaml::from_str(front_matter).map_err(|e| DocumentError::YamlError(e.to_string()))?;

    if raw.kind != expected_kind {
        return Err(DocumentError::KindMismatch {
            expected: expected_kind.as_str().to_string(),
            found: raw.kind.as_str().to_string(),
        });
    }

    if raw.schema_version == 0 || raw.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(DocumentError::UnsupportedSchemaVersion {
            found: raw.schema_version,
            max_supported: CURRENT_SCHEMA_VERSION,
        });
    }

    // Strictly deserialize the domain-specific data object
    let data: T =
        T::deserialize(raw.data).map_err(|e| DocumentError::ValidationError(e.to_string()))?;

    Ok(MarkdownDocument::new(
        raw.kind,
        raw.schema_version,
        data,
        body,
    ))
}

/// Parses a raw Markdown document into a [`MarkdownDocument<serde_yaml::Value>`] without a specific domain schema.
pub fn parse_markdown_value(
    content: &str,
    expected_kind: DocumentKind,
) -> Result<MarkdownDocument<serde_yaml::Value>, DocumentError> {
    parse_markdown_document::<serde_yaml::Value>(content, expected_kind)
}
