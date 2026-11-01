//! Semver Drift & Breaking API Change Detector (`m31a-ast::semver`).
//!
//! Detects version incompatibilities, breaking signature mutations,
//! and removed public APIs between baseline and candidate revisions.
//!
//! Enforces:
//! 1. Removal Detection: Flags removed public functions, structs, enums, traits, or methods.
//! 2. Signature Drift: Detects modified argument types, counts, or return signatures.
//! 3. Visibility Reduction: Flags public symbols downgraded to private.
//! 4. Semver Classification: Categorizes diffs into Major (Breaking), Minor (Additive), or Patch (Internal).

use crate::types::{AstSymbol, AstSymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemverImpact {
    Major, // Breaking change
    Minor, // Backward-compatible addition
    Patch, // Non-API internal fix
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChange {
    pub symbol_name: String,
    pub kind: AstSymbolKind,
    pub description: String,
    pub old_signature: Option<String>,
    pub new_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemverAuditReport {
    pub required_bump: SemverImpact,
    pub is_breaking: bool,
    pub breaking_changes: Vec<BreakingChange>,
    pub added_symbols: Vec<String>,
    pub modified_symbols: Vec<String>,
    pub summary: String,
}

pub struct SemverDriftDetector;

impl SemverDriftDetector {
    /// Compares baseline public symbols against candidate public symbols.
    pub fn compare_apis(
        baseline_symbols: &[AstSymbol],
        candidate_symbols: &[AstSymbol],
    ) -> SemverAuditReport {
        let mut breaking = Vec::new();
        let mut added = Vec::new();
        let mut modified = Vec::new();

        let mut base_map: HashMap<String, &AstSymbol> = HashMap::new();
        for s in baseline_symbols.iter().filter(|s| s.is_public) {
            base_map.insert(s.name.clone(), s);
        }

        let mut cand_map: HashMap<String, &AstSymbol> = HashMap::new();
        for s in candidate_symbols.iter().filter(|s| s.is_public) {
            cand_map.insert(s.name.clone(), s);
        }

        // Check for removed public symbols
        for (name, base_sym) in &base_map {
            if !cand_map.contains_key(name) {
                breaking.push(BreakingChange {
                    symbol_name: name.clone(),
                    kind: base_sym.kind,
                    description: format!(
                        "Public {} '{}' was removed or made private.",
                        base_sym.kind.as_str(),
                        name
                    ),
                    old_signature: Some(base_sym.signature.clone()),
                    new_signature: None,
                });
            } else {
                let cand_sym = cand_map[name];
                // Check if signature changed
                if base_sym.signature != cand_sym.signature
                    && !base_sym.signature.is_empty()
                    && !cand_sym.signature.is_empty()
                {
                    modified.push(name.clone());
                    breaking.push(BreakingChange {
                        symbol_name: name.clone(),
                        kind: base_sym.kind,
                        description: format!(
                            "Signature of public {} '{}' was altered.",
                            base_sym.kind.as_str(),
                            name
                        ),
                        old_signature: Some(base_sym.signature.clone()),
                        new_signature: Some(cand_sym.signature.clone()),
                    });
                }
            }
        }

        // Check for added public symbols
        for name in cand_map.keys() {
            if !base_map.contains_key(name) {
                added.push(name.clone());
            }
        }

        let is_breaking = !breaking.is_empty();
        let required_bump = if is_breaking {
            SemverImpact::Major
        } else if !added.is_empty() {
            SemverImpact::Minor
        } else {
            SemverImpact::Patch
        };

        let summary = format!(
            "Semver Audit: {} breaking changes, {} added APIs. Recommended Semver Bump: {:?}",
            breaking.len(),
            added.len(),
            required_bump
        );

        SemverAuditReport {
            required_bump,
            is_breaking,
            breaking_changes: breaking,
            added_symbols: added,
            modified_symbols: modified,
            summary,
        }
    }
}

/// Severity of a manifest dependency finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencySeverity {
    Critical,
    Warning,
    Info,
}

/// An audit issue discovered within a dependency specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyIssue {
    pub package_name: String,
    pub declared_version: String,
    pub severity: DependencySeverity,
    pub rule: String,
    pub suggestion: String,
}

/// Comprehensive dependency audit report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAuditReport {
    pub total_dependencies: usize,
    pub issues: Vec<DependencyIssue>,
    pub has_critical: bool,
    pub summary: String,
}

/// Auditor for package manifest dependencies (Cargo.toml, package.json).
pub struct ManifestDependencyAuditor;

impl ManifestDependencyAuditor {
    /// Audits a Cargo.toml file content for unpinned versions, wildcards, git heads, and anomalies.
    pub fn audit_cargo_toml(content: &str) -> Result<DependencyAuditReport, toml::de::Error> {
        let parsed: toml::Value = toml::from_str(content)?;
        let mut total_deps = 0;
        let mut issues = Vec::new();

        let table_keys = ["dependencies", "dev-dependencies", "build-dependencies"];
        for key in &table_keys {
            if let Some(toml::Value::Table(deps)) = parsed.get(*key) {
                for (name, val) in deps {
                    total_deps += 1;
                    match val {
                        toml::Value::String(ver_str) => {
                            Self::check_version_string(name, ver_str, &mut issues);
                        }
                        toml::Value::Table(spec) => {
                            if let Some(toml::Value::String(ver_str)) = spec.get("version") {
                                Self::check_version_string(name, ver_str, &mut issues);
                            }
                            if let Some(toml::Value::String(git)) = spec.get("git")
                                && spec.get("tag").is_none()
                                && spec.get("rev").is_none()
                            {
                                issues.push(DependencyIssue {
                                        package_name: name.clone(),
                                        declared_version: git.clone(),
                                        severity: DependencySeverity::Warning,
                                        rule: "unpinned-git-dependency".to_string(),
                                        suggestion: "Pin git dependency with an explicit 'rev' or 'tag' commit SHA.".to_string(),
                                    });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Also check workspace.dependencies if present
        if let Some(workspace_table) = parsed
            .get("workspace")
            .and_then(|w| w.get("dependencies"))
            .and_then(|d| d.as_table())
        {
            for (name, val) in workspace_table {
                total_deps += 1;
                match val {
                    toml::Value::String(ver_str) => {
                        Self::check_version_string(name, ver_str, &mut issues);
                    }
                    toml::Value::Table(spec) => {
                        if let Some(toml::Value::String(ver_str)) = spec.get("version") {
                            Self::check_version_string(name, ver_str, &mut issues);
                        }
                    }
                    _ => {}
                }
            }
        }

        let has_critical = issues
            .iter()
            .any(|i| i.severity == DependencySeverity::Critical);
        let summary = format!(
            "Cargo Dependency Audit: {} dependencies audited, {} issues found ({} critical).",
            total_deps,
            issues.len(),
            issues
                .iter()
                .filter(|i| i.severity == DependencySeverity::Critical)
                .count()
        );

        Ok(DependencyAuditReport {
            total_dependencies: total_deps,
            issues,
            has_critical,
            summary,
        })
    }

    /// Audits a package.json content for npm wildcards or unpinned dependencies.
    pub fn audit_package_json(content: &str) -> Result<DependencyAuditReport, serde_json::Error> {
        let parsed: serde_json::Value = serde_json::from_str(content)?;
        let mut total_deps = 0;
        let mut issues = Vec::new();

        for section_key in &["dependencies", "devDependencies", "peerDependencies"] {
            if let Some(obj) = parsed.get(*section_key).and_then(|v| v.as_object()) {
                for (name, ver_val) in obj {
                    total_deps += 1;
                    if let Some(ver_str) = ver_val.as_str() {
                        Self::check_version_string(name, ver_str, &mut issues);
                    }
                }
            }
        }

        let has_critical = issues
            .iter()
            .any(|i| i.severity == DependencySeverity::Critical);
        let summary = format!(
            "NPM Dependency Audit: {} dependencies audited, {} issues found ({} critical).",
            total_deps,
            issues.len(),
            issues
                .iter()
                .filter(|i| i.severity == DependencySeverity::Critical)
                .count()
        );

        Ok(DependencyAuditReport {
            total_dependencies: total_deps,
            issues,
            has_critical,
            summary,
        })
    }

    fn check_version_string(name: &str, ver: &str, issues: &mut Vec<DependencyIssue>) {
        let trimmed = ver.trim();
        if trimmed == "*" || trimmed == "latest" || trimmed == ">=0.0.0" {
            issues.push(DependencyIssue {
                package_name: name.to_string(),
                declared_version: ver.to_string(),
                severity: DependencySeverity::Critical,
                rule: "wildcard-dependency".to_string(),
                suggestion: format!(
                    "Replace wildcard '{}' with an explicit semantic version constraint.",
                    ver
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceRange;
    use std::path::PathBuf;

    fn make_sym(name: &str, sig: &str, is_pub: bool) -> AstSymbol {
        AstSymbol {
            name: name.to_string(),
            kind: AstSymbolKind::Function,
            file_path: PathBuf::from("src/lib.rs"),
            range: SourceRange::new(1, 10, 1, 1),
            signature: sig.to_string(),
            doc_comment: None,
            parent_scope: None,
            is_public: is_pub,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_semver_detects_removed_public_function() {
        let base = vec![make_sym("init_db", "pub fn init_db()", true)];
        let cand = vec![];

        let report = SemverDriftDetector::compare_apis(&base, &cand);
        assert!(report.is_breaking);
        assert_eq!(report.required_bump, SemverImpact::Major);
        assert_eq!(report.breaking_changes.len(), 1);
        assert!(report.breaking_changes[0].description.contains("removed"));
    }

    #[test]
    fn test_semver_detects_signature_change() {
        let base = vec![make_sym("send", "pub fn send(id: u64)", true)];
        let cand = vec![make_sym(
            "send",
            "pub fn send(id: u64, timeout: Duration)",
            true,
        )];

        let report = SemverDriftDetector::compare_apis(&base, &cand);
        assert!(report.is_breaking);
        assert_eq!(report.required_bump, SemverImpact::Major);
        assert_eq!(
            report.breaking_changes[0].old_signature,
            Some("pub fn send(id: u64)".to_string())
        );
    }

    #[test]
    fn test_semver_additive_is_minor() {
        let base = vec![make_sym("read", "pub fn read()", true)];
        let cand = vec![
            make_sym("read", "pub fn read()", true),
            make_sym("write", "pub fn write()", true),
        ];

        let report = SemverDriftDetector::compare_apis(&base, &cand);
        assert!(!report.is_breaking);
        assert_eq!(report.required_bump, SemverImpact::Minor);
        assert_eq!(report.added_symbols.len(), 1);
    }

    #[test]
    fn test_manifest_dependency_auditor_cargo() {
        let cargo_toml = r#"
[dependencies]
serde = "1.0"
bad_dep = "*"
git_dep = { git = "https://github.com/example/foo" }
"#;
        let report = ManifestDependencyAuditor::audit_cargo_toml(cargo_toml).unwrap();
        assert_eq!(report.total_dependencies, 3);
        assert!(report.has_critical);
        assert_eq!(report.issues.len(), 2);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.rule == "wildcard-dependency")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.rule == "unpinned-git-dependency")
        );
    }

    #[test]
    fn test_manifest_dependency_auditor_npm() {
        let pkg_json = r#"
{
  "dependencies": {
    "react": "^18.2.0",
    "lodash": "*"
  }
}
"#;
        let report = ManifestDependencyAuditor::audit_package_json(pkg_json).unwrap();
        assert_eq!(report.total_dependencies, 2);
        assert!(report.has_critical);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].package_name, "lodash");
    }
}
