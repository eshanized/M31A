//! Structural & Schema Drift Detector (`m31a-ast::drift`).
//!
//! Detects structural and schema drift between documented planning territory and current workspace state:
//! - Tracks `last_mapped_commit` and `last_mapped_at` in `.planning/codebase/*.md` frontmatter.
//! - Enforces the **Inverse-Territory Rule**:
//!   - An addition is drift OUTSIDE mapped territory (the documentation cannot describe what did not exist).
//!   - A modification or deletion is drift INSIDE mapped territory (the documentation describes it and it changed).
//! - Identifies structural drift categories:
//!   - `new_dir`: File whose directory prefix does not appear in mapped territory.
//!   - `barrel`: Export index file (e.g., `packages/*/src/index.ts`, `crates/*/src/lib.rs`).
//!   - `migration`: Database migration files (`migrations/`, `prisma/migrations/`, etc.).
//!   - `route`: Route module (`routes/`, `api/`).
//!   - `modified`: Changed file in mapped territory.
//!   - `deleted`: Removed file from mapped territory.
//!   - `schema_drift`: Model definitions changed (e.g. `schema.prisma`, `schema.rs`, `diesel`, `sea-orm`) without corresponding migrations.
//! - Priority resolution: `SchemaDrift` > `Deleted` > `Modified` > `Migration` > `Route` > `Barrel` > `NewDir`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DriftCategory {
    NewDir = 0,
    Barrel = 1,
    Route = 2,
    Migration = 3,
    Modified = 4,
    Deleted = 5,
    SchemaDrift = 6,
}

impl DriftCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NewDir => "new_dir",
            Self::Barrel => "barrel",
            Self::Route => "route",
            Self::Migration => "migration",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::SchemaDrift => "schema_drift",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftElement {
    pub path: String,
    pub category: DriftCategory,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriftCounts {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub schema_drift: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResult {
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub elements: Vec<DriftElement>,
    pub action_required: bool,
    pub spawn_mapper: bool,
    pub threshold: usize,
    pub affected_paths: Vec<String>,
    pub schema_drift_detected: bool,
    pub counts: DriftCounts,
    pub message: String,
}

pub struct CodebaseDriftDetector;

impl CodebaseDriftDetector {
    /// Extracts `last_mapped_commit` from YAML frontmatter in a markdown document.
    pub fn read_last_mapped_commit(markdown: &str) -> Option<String> {
        let trimmed = markdown.trim_start();
        if !trimmed.starts_with("---") {
            return None;
        }

        let after_start = &trimmed[3..];
        let end_idx = after_start.find("\n---")?;
        let frontmatter = &after_start[..end_idx];

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("last_mapped_commit:") {
                let sha = rest.trim().trim_matches('"').trim_matches('\'').trim();
                if !sha.is_empty() {
                    return Some(sha.to_string());
                }
            }
        }

        None
    }

    /// Upserts `last_mapped_commit` and `last_mapped_at` into markdown frontmatter.
    pub fn stamp_last_mapped_commit(markdown: &str, commit_sha: &str) -> String {
        let now_iso = chrono::Utc::now().to_rfc3339();
        let trimmed = markdown.trim_start();

        if let Some(after_start) = trimmed.strip_prefix("---")
            && let Some(end_idx) = after_start.find("\n---")
        {
            let frontmatter = &after_start[..end_idx];
            let rest_doc = &after_start[end_idx + 4..];

            let mut lines: Vec<String> = frontmatter
                .lines()
                .filter(|l| {
                    !l.trim().starts_with("last_mapped_commit:")
                        && !l.trim().starts_with("last_mapped_at:")
                })
                .map(|l| l.to_string())
                .collect();

            lines.push(format!("last_mapped_commit: \"{}\"", commit_sha));
            lines.push(format!("last_mapped_at: \"{}\"", now_iso));

            return format!("---\n{}\n---{}", lines.join("\n"), rest_doc);
        }

        format!(
            "---\nlast_mapped_commit: \"{}\"\nlast_mapped_at: \"{}\"\n---\n\n{}",
            commit_sha, now_iso, markdown
        )
    }

    /// Determines if a file path is a database migration.
    pub fn is_migration_file(path: &str) -> bool {
        let norm = Self::normalize_path(path);
        norm.starts_with("supabase/migrations/")
            || norm.starts_with("prisma/migrations/")
            || norm.starts_with("drizzle/migrations/")
            || norm.starts_with("drizzle/meta/")
            || norm.starts_with("src/migrations/")
            || norm.starts_with("db/migrations/")
            || norm.starts_with("migrations/")
            || (norm.contains("/migrations/")
                && (norm.ends_with(".sql")
                    || norm.ends_with(".rs")
                    || norm.ends_with(".ts")
                    || norm.ends_with(".js")))
    }

    /// Determines if a file is an ORM / schema model file.
    pub fn is_schema_file(path: &str) -> bool {
        let norm = Self::normalize_path(path);
        norm.ends_with("schema.prisma")
            || norm.ends_with("schema.rs")
            || norm.ends_with("drizzle/schema.ts")
            || norm.ends_with("models.py")
            || norm.contains("models/")
            || norm.contains("entities/")
            || norm.ends_with("db/schema.ts")
    }

    /// Determines if a file path is a route or API module.
    pub fn is_route_file(path: &str) -> bool {
        let norm = Self::normalize_path(path);
        norm.contains("src/routes/")
            || norm.contains("src/api/")
            || norm.starts_with("routes/")
            || norm.starts_with("api/")
    }

    /// Determines if a file is a barrel export (e.g. `index.ts`, `lib.rs`).
    pub fn is_barrel_file(path: &str) -> bool {
        let norm = Self::normalize_path(path);
        (norm.starts_with("packages/") || norm.starts_with("apps/"))
            && norm.ends_with("/src/index.ts")
            || norm.ends_with("/src/index.tsx")
            || norm.ends_with("/src/index.js")
            || norm.ends_with("/src/lib.rs")
            || norm == "src/index.ts"
            || norm == "src/lib.rs"
    }

    /// Normalizes path to POSIX forward slashes.
    pub fn normalize_path(path: &str) -> String {
        path.replace('\\', "/").trim_start_matches("./").to_string()
    }

    /// Checks if a needle occurs in corpus at path component boundaries.
    pub fn occurs_at_boundary(corpus: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }

        let mut start = 0;
        while let Some(pos) = corpus[start..].find(needle) {
            let actual_pos = start + pos;
            let end = actual_pos + needle.len();

            let before_ok = if actual_pos == 0 {
                true
            } else {
                let prev = corpus.as_bytes()[actual_pos - 1] as char;
                !prev.is_alphanumeric() && prev != '_' && prev != '-' && prev != '.'
            };

            let after_ok = if end >= corpus.len() {
                true
            } else {
                let next = corpus.as_bytes()[end] as char;
                !next.is_alphanumeric() && next != '_' && next != '-'
            };

            if before_ok && after_ok {
                return true;
            }

            start = actual_pos + 1;
        }

        false
    }

    /// Checks if any directory prefix of `file` appears in the corpus.
    pub fn is_path_mapped(file: &str, corpus: &str) -> bool {
        let norm = Self::normalize_path(file);
        let parts: Vec<&str> = norm.split('/').collect();

        // Check directory prefixes
        for i in (1..parts.len()).rev() {
            let prefix = parts[..i].join("/");
            if !prefix.is_empty() && Self::occurs_at_boundary(corpus, &prefix) {
                return true;
            }
        }

        // Single component root level file
        if parts.len() == 1 && !parts[0].is_empty() {
            let with_slash = format!("{}/", parts[0]);
            let backtick = format!("`{}`", parts[0]);
            if Self::occurs_at_boundary(corpus, &with_slash) || corpus.contains(&backtick) {
                return true;
            }
        }

        false
    }

    /// Evaluates structural and schema drift against mapped territory.
    pub fn detect_drift(
        added_files: &[String],
        modified_files: &[String],
        deleted_files: &[String],
        corpus: &str,
        threshold: usize,
        auto_remap: bool,
    ) -> DriftResult {
        if corpus.trim().is_empty() {
            return DriftResult {
                skipped: true,
                skip_reason: Some("missing-structure-doc".into()),
                elements: Vec::new(),
                action_required: false,
                spawn_mapper: false,
                threshold,
                affected_paths: Vec::new(),
                schema_drift_detected: false,
                counts: DriftCounts::default(),
                message: "Drift check skipped: no structure documentation corpus provided".into(),
            };
        }

        let mut seen = HashMap::<String, (DriftCategory, String)>::new();

        // 1. Check added files (Additions are drift OUTSIDE mapped territory or special categories)
        for raw in added_files {
            let file = Self::normalize_path(raw);
            if file.is_empty() {
                continue;
            }

            let category = if Self::is_migration_file(&file) {
                Some((DriftCategory::Migration, "New migration file added".into()))
            } else if Self::is_route_file(&file) {
                Some((DriftCategory::Route, "New route module added".into()))
            } else if Self::is_barrel_file(&file) {
                Some((DriftCategory::Barrel, "New barrel export added".into()))
            } else if !Self::is_path_mapped(&file, corpus) {
                Some((
                    DriftCategory::NewDir,
                    "New unmapped directory path added".into(),
                ))
            } else {
                None // mapped territory normal addition
            };

            if let Some((cat, reason)) = category {
                seen.insert(file, (cat, reason));
            }
        }

        // 2. Check modified and deleted files (Edits/deletions are drift INSIDE mapped territory)
        for raw in modified_files {
            let file = Self::normalize_path(raw);
            if file.is_empty() {
                continue;
            }
            if Self::is_path_mapped(&file, corpus) {
                let existing_prio = seen.get(&file).map(|(c, _)| *c as usize).unwrap_or(0);
                if existing_prio < DriftCategory::Modified as usize {
                    seen.insert(
                        file,
                        (
                            DriftCategory::Modified,
                            "Modified file in mapped territory".into(),
                        ),
                    );
                }
            }
        }

        for raw in deleted_files {
            let file = Self::normalize_path(raw);
            if file.is_empty() {
                continue;
            }
            if Self::is_path_mapped(&file, corpus) {
                let existing_prio = seen.get(&file).map(|(c, _)| *c as usize).unwrap_or(0);
                if existing_prio < DriftCategory::Deleted as usize {
                    seen.insert(
                        file,
                        (
                            DriftCategory::Deleted,
                            "Deleted file in mapped territory".into(),
                        ),
                    );
                }
            }
        }

        // 3. Schema drift detection: ORM schema changed but no migration added
        let has_modified_schema = modified_files.iter().any(|f| Self::is_schema_file(f));
        let has_new_migration = added_files.iter().any(|f| Self::is_migration_file(f));
        let schema_drift_detected = has_modified_schema && !has_new_migration;

        if schema_drift_detected {
            for f in modified_files {
                if Self::is_schema_file(f) {
                    seen.insert(
                        Self::normalize_path(f),
                        (
                            DriftCategory::SchemaDrift,
                            "Schema model modified without corresponding database migration".into(),
                        ),
                    );
                }
            }
        }

        let mut elements: Vec<DriftElement> = seen
            .into_iter()
            .map(|(path, (category, reason))| DriftElement {
                path,
                category,
                reason,
            })
            .collect();

        // Sort elements deterministically
        elements.sort_by(|a, b| {
            b.category
                .cmp(&a.category)
                .then_with(|| a.path.cmp(&b.path))
        });

        let affected_paths: Vec<String> = elements.iter().map(|e| e.path.clone()).collect();
        let action_required = elements.len() >= threshold || schema_drift_detected;
        let spawn_mapper = auto_remap && action_required;

        let counts = DriftCounts {
            added: added_files.len(),
            modified: modified_files.len(),
            deleted: deleted_files.len(),
            schema_drift: if schema_drift_detected { 1 } else { 0 },
        };

        let message = if action_required {
            format!(
                "Drift threshold exceeded: {} drift element(s) detected (threshold: {}). Schema drift: {}. {}",
                elements.len(),
                threshold,
                schema_drift_detected,
                if spawn_mapper {
                    "Spawning codebase re-mapper."
                } else {
                    "Review or remap required."
                }
            )
        } else {
            format!(
                "Drift within acceptable threshold: {} element(s) (threshold: {}).",
                elements.len(),
                threshold
            )
        };

        DriftResult {
            skipped: false,
            skip_reason: None,
            elements,
            action_required,
            spawn_mapper,
            threshold,
            affected_paths,
            schema_drift_detected,
            counts,
            message,
        }
    }

    /// Evaluates drift using git between `last_mapped_commit` and `HEAD`.
    pub fn detect_git_drift(
        repo_root: &Path,
        last_mapped_commit: &str,
        corpus: &str,
        threshold: usize,
        auto_remap: bool,
    ) -> anyhow::Result<DriftResult> {
        let output = Command::new("git")
            .arg("diff")
            .arg("--name-status")
            .arg(format!("{}..HEAD", last_mapped_commit))
            .current_dir(repo_root)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(DriftResult {
                skipped: true,
                skip_reason: Some(format!("git-diff-failed: {}", stderr.trim())),
                elements: Vec::new(),
                action_required: false,
                spawn_mapper: false,
                threshold,
                affected_paths: Vec::new(),
                schema_drift_detected: false,
                counts: DriftCounts::default(),
                message: format!("Git diff failed: {}", stderr.trim()),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }
            let status = parts[0].trim();
            let path = parts[1].trim();

            if status.starts_with('A') {
                added.push(path.to_string());
            } else if status.starts_with('M') {
                modified.push(path.to_string());
            } else if status.starts_with('D') {
                deleted.push(path.to_string());
            } else if status.starts_with('R') && parts.len() >= 3 {
                deleted.push(path.to_string());
                added.push(parts[2].trim().to_string());
            }
        }

        Ok(Self::detect_drift(
            &added, &modified, &deleted, corpus, threshold, auto_remap,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontmatter_commit_extraction_and_stamping() {
        let md = r#"---
last_mapped_commit: "a1b2c3d4"
last_mapped_at: "2026-10-29T10:00:00Z"
---
# Structure
The codebase layout.
"#;
        assert_eq!(
            CodebaseDriftDetector::read_last_mapped_commit(md),
            Some("a1b2c3d4".to_string())
        );

        let stamped = CodebaseDriftDetector::stamp_last_mapped_commit(md, "e5f60718");
        assert_eq!(
            CodebaseDriftDetector::read_last_mapped_commit(&stamped),
            Some("e5f60718".to_string())
        );
        assert!(stamped.contains("last_mapped_commit: \"e5f60718\""));
        assert!(stamped.contains("# Structure"));
    }

    #[test]
    fn test_inverse_territory_drift_detection() {
        let corpus = r#"
# Codebase Structure
- `crates/m31a-core/`
- `crates/m31a-verify/`
- `crates/m31a-storage/`
"#;
        let added = vec![
            "crates/m31a-verify/src/helper.rs".to_string(), // In mapped territory -> not drift
            "services/m31a-newservice/src/main.rs".to_string(), // Outside mapped territory -> NewDir drift
            "crates/m31a-core/src/routes/api.ts".to_string(),   // Route module -> Route drift
        ];

        let modified = vec![
            "crates/m31a-core/src/lib.rs".to_string(), // In mapped territory -> Modified drift
            "external/unmapped/script.sh".to_string(), // Outside territory -> not drift
        ];

        let deleted = vec![
            "crates/m31a-storage/src/old.rs".to_string(), // In mapped territory -> Deleted drift
        ];

        let res =
            CodebaseDriftDetector::detect_drift(&added, &modified, &deleted, corpus, 3, false);

        assert!(!res.skipped);
        assert!(res.action_required);
        // Elements should include NewDir (m31a-newservice), Route (routes/api.ts), Modified (m31a-core), Deleted (m31a-storage)
        assert_eq!(res.elements.len(), 4);
    }

    #[test]
    fn test_schema_drift_detection() {
        let corpus = "# Architecture\n- `src/db/`\n";
        let added = vec!["src/db/queries.rs".to_string()];
        let modified = vec!["prisma/schema.prisma".to_string()];
        let deleted = vec![];

        let res =
            CodebaseDriftDetector::detect_drift(&added, &modified, &deleted, corpus, 5, false);

        assert!(res.schema_drift_detected);
        assert!(res.action_required);
        assert!(
            res.elements
                .iter()
                .any(|e| e.category == DriftCategory::SchemaDrift)
        );
    }
}
