//! Multi-Tier Skill Discovery (SKL-01, D-09).
//!
//! Scans across Builtin -> System -> User -> Workspace precedence tiers,
//! parses canonical skill.md files (with legacy SKILL.toml compatibility),
//! and calculates cryptographic SHA-256 content hashes.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::skill::manifest::SkillManifest;

/// Precedence tier for skill definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkillOriginTier {
    /// Embedded default skills packaged directly in the binary.
    Builtin = 0,
    /// System-wide administrator skills (/etc/m31a/skills).
    System = 1,
    /// User-specific global skills (~/.config/m31a/skills).
    User = 2,
    /// Workspace-specific project skills (<workspace>/.m31a/skills).
    Workspace = 3,
}

impl std::fmt::Display for SkillOriginTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillOriginTier::Builtin => write!(f, "builtin"),
            SkillOriginTier::System => write!(f, "system"),
            SkillOriginTier::User => write!(f, "user"),
            SkillOriginTier::Workspace => write!(f, "workspace"),
        }
    }
}

/// Discovered and validated skill package ready for registry insertion.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillPackage {
    pub manifest: SkillManifest,
    pub origin: SkillOriginTier,
    pub source_path: Option<PathBuf>,
    pub content_hash: String,
}

/// Computes SHA-256 hex digest for content integrity.
pub fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Embedded default builtin skill manifests.
///
/// Definitions live in `assets/skills/*/skill.md` (packaged declarative
/// assets) and load through the same parser as external skills — no
/// Rust-only registry. This function is a thin bundling boundary, not a
/// second definition site.
pub fn builtin_skills() -> Vec<SkillPackage> {
    const BUILTIN_ASSETS: &[&str] = &[
        include_str!("../../assets/skills/fix-test-failure/skill.md"),
        include_str!("../../assets/skills/refactor-module/skill.md"),
        include_str!("../../assets/skills/prepare-release/skill.md"),
    ];

    BUILTIN_ASSETS
        .iter()
        .map(|doc_str| {
            let manifest =
                SkillManifest::parse_str_auto(doc_str).expect("builtin skill must parse");
            let hash = compute_sha256(doc_str);
            SkillPackage {
                manifest,
                origin: SkillOriginTier::Builtin,
                source_path: None,
                content_hash: hash,
            }
        })
        .collect()
}

/// Scanner discovering skills from filesystem directories.
pub struct SkillDiscovery;

impl SkillDiscovery {
    /// Discover all skills from a specific directory tier.
    pub fn discover_directory(base_dir: &Path, tier: SkillOriginTier) -> Vec<SkillPackage> {
        let (packages, diagnostics) = Self::discover_directory_with_diagnostics(base_dir, tier);
        for (path, err) in diagnostics {
            tracing::warn!("Skill discovery warning at '{}': {err}", path.display());
        }
        packages
    }

    /// Discover skills from a directory tier, capturing any error diagnostics alongside valid packages.
    pub fn discover_directory_with_diagnostics(
        base_dir: &Path,
        tier: SkillOriginTier,
    ) -> (Vec<SkillPackage>, Vec<(PathBuf, String)>) {
        let mut packages = Vec::new();
        let mut diagnostics = Vec::new();
        if !base_dir.exists() || !base_dir.is_dir() {
            return (packages, diagnostics);
        }

        let entries = match std::fs::read_dir(base_dir) {
            Ok(e) => e,
            Err(e) => {
                diagnostics.push((base_dir.to_path_buf(), format!("read_dir failed: {e}")));
                return (packages, diagnostics);
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let md_path = path.join("skill.md");
                let toml_path = path.join("SKILL.toml");

                let chosen_path = if md_path.is_file() {
                    if toml_path.is_file() {
                        tracing::warn!(
                            "Both Markdown ('skill.md') and legacy TOML ('SKILL.toml') skill manifests exist in '{}'. Markdown took precedence.",
                            path.display()
                        );
                    }
                    Some(md_path)
                } else if toml_path.is_file() {
                    Some(toml_path)
                } else {
                    None
                };

                if let Some(manifest_path) = chosen_path {
                    match std::fs::read_to_string(&manifest_path) {
                        Ok(content) => match SkillManifest::parse_str_auto(&content) {
                            Ok(manifest) => {
                                let content_hash = compute_sha256(&content);
                                packages.push(SkillPackage {
                                    manifest,
                                    origin: tier,
                                    source_path: Some(manifest_path),
                                    content_hash,
                                });
                            }
                            Err(e) => {
                                diagnostics.push((manifest_path, format!("parse error: {e}")));
                            }
                        },
                        Err(e) => {
                            diagnostics.push((manifest_path, format!("IO read error: {e}")));
                        }
                    }
                }
            }
        }

        (packages, diagnostics)
    }

    /// Run discovery across all 4 tiers in ascending precedence order.
    pub fn discover_all(workspace_root: Option<&Path>) -> Vec<SkillPackage> {
        let mut all = Vec::new();

        // 1. Built-in tier (lowest precedence baseline)
        all.extend(builtin_skills());

        let platform_paths = crate::config::PlatformPaths::new();

        // 2. System tier
        let system_dir = platform_paths.system_config_dir().join("skills");
        all.extend(Self::discover_directory(
            &system_dir,
            SkillOriginTier::System,
        ));

        // 3. User tier
        let user_skills = platform_paths.config_dir().join("skills");
        all.extend(Self::discover_directory(
            &user_skills,
            SkillOriginTier::User,
        ));

        // 4. Workspace tier (highest precedence)
        if let Some(ws) = workspace_root {
            let ws_skills = ws.join(".m31a").join("skills");
            all.extend(Self::discover_directory(
                &ws_skills,
                SkillOriginTier::Workspace,
            ));
        }

        all
    }
}
