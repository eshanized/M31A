use crate::ids::{MissionId, TaskId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Cryptographic repository baseline snapshot at mission or task boundary (REP-05, D-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryBaseline {
    pub id: Uuid,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub git_commit: Option<String>,
    pub file_hashes: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
}

impl RepositoryBaseline {
    pub fn new(
        mission_id: MissionId,
        task_id: Option<TaskId>,
        git_commit: Option<String>,
        file_hashes: BTreeMap<String, String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            mission_id,
            task_id,
            git_commit,
            file_hashes,
            created_at: Utc::now(),
        }
    }

    /// Capture file-level SHA-256 tree snapshot across the workspace root.
    pub fn capture(
        workspace_root: &Path,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        git_commit: Option<String>,
    ) -> Result<Self, std::io::Error> {
        let mut file_hashes = BTreeMap::new();
        let excluded_dirs = [
            "target",
            "node_modules",
            ".git",
            ".m31a",
            ".m31",
            ".gemini",
            "dist",
            "build",
            ".next",
            "venv",
            ".venv",
            "__pycache__",
        ];

        let canonical_base = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        let mut visited = std::collections::HashSet::new();
        visited.insert(canonical_base.clone());

        Self::collect_hashes(
            workspace_root,
            &canonical_base,
            workspace_root,
            &excluded_dirs,
            &mut visited,
            &mut file_hashes,
        )?;

        Ok(Self::new(mission_id, task_id, git_commit, file_hashes))
    }

    fn collect_hashes(
        base: &Path,
        canonical_base: &Path,
        dir: &Path,
        excluded: &[&str],
        visited: &mut std::collections::HashSet<std::path::PathBuf>,
        acc: &mut BTreeMap<String, String>,
    ) -> Result<(), std::io::Error> {
        let read_dir = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };

        for entry in read_dir {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            if excluded.iter().any(|ex| ex == &name_str) {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_symlink() {
                // Symlink handling: never follow symlinks outside the workspace (F-08)
                // and detect directory cycles.
                let canon_target = match path.canonicalize() {
                    Ok(c) => c,
                    Err(_) => {
                        // Dangling or inaccessible symlink: skip safely without following
                        continue;
                    }
                };

                // Escape check: target must be confined within the workspace root
                if !canon_target.starts_with(canonical_base) {
                    tracing::debug!(
                        path = %path.display(),
                        target = %canon_target.display(),
                        "Skipping symlink escaping workspace baseline boundary"
                    );
                    continue;
                }

                if canon_target.is_dir() {
                    // Prevent recursion loops / cycles
                    if visited.insert(canon_target.clone()) {
                        Self::collect_hashes(
                            base,
                            canonical_base,
                            &canon_target,
                            excluded,
                            visited,
                            acc,
                        )?;
                    }
                } else if canon_target.is_file() {
                    if let Ok(bytes) = fs::read(&canon_target) {
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        let hash = format!("{:x}", hasher.finalize());

                        if let Ok(rel) = path.strip_prefix(base) {
                            let rel_path = rel.to_string_lossy().replace('\\', "/");
                            acc.insert(rel_path, hash);
                        }
                    }
                }
            } else if file_type.is_dir() {
                let canon_dir = path.canonicalize().unwrap_or_else(|_| path.clone());
                if !canon_dir.starts_with(canonical_base) {
                    continue;
                }
                if visited.insert(canon_dir) {
                    Self::collect_hashes(base, canonical_base, &path, excluded, visited, acc)?;
                }
            } else if file_type.is_file() {
                if let Ok(bytes) = fs::read(&path) {
                    let mut hasher = Sha256::new();
                    hasher.update(&bytes);
                    let hash = format!("{:x}", hasher.finalize());

                    if let Ok(rel) = path.strip_prefix(base) {
                        let rel_path = rel.to_string_lossy().replace('\\', "/");
                        acc.insert(rel_path, hash);
                    }
                }
            }
        }

        Ok(())
    }

    /// Persist this baseline to SQLite Migration 005 `repository_baselines`.
    pub async fn save_to_db(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let fingerprint_json = serde_json::to_string(&self.file_hashes).map_err(|e| {
            sqlx::Error::Protocol(format!("fingerprint map serialization failed: {e}"))
        })?;

        sqlx::query(
            r#"
            INSERT INTO repository_baselines (id, run_id, mission_id, task_id, git_commit, fingerprint_map, created_at)
            SELECT ?, (SELECT id FROM execution_runs WHERE id = ?), ?, ?, ?, ?, ?
            "#,
        )
        .bind(self.id.as_bytes().as_slice())
        .bind(self.mission_id.as_bytes().as_slice())
        .bind(self.mission_id.as_bytes().as_slice())
        .bind(self.task_id.as_ref().map(|t| t.as_bytes().as_slice().to_vec()))
        .bind(&self.git_commit)
        .bind(&fingerprint_json)
        .bind(self.created_at.to_rfc3339())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Load the latest baseline for a mission.
    pub async fn load_latest_for_mission(
        pool: &SqlitePool,
        mission_id: MissionId,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, COALESCE(run_id, mission_id) as mission_id, task_id, git_commit, fingerprint_map, created_at
            FROM repository_baselines
            WHERE (run_id = ? OR (run_id IS NULL AND mission_id = ?))
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(pool)
        .await?;

        if let Some(r) = row {
            let id_bytes: Vec<u8> = r.get("id");
            let id = Uuid::from_slice(&id_bytes).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

            let m_bytes: Vec<u8> = r.get("mission_id");
            let m_arr: [u8; 16] = m_bytes
                .as_slice()
                .try_into()
                .map_err(|_| sqlx::Error::Protocol("invalid mission_id bytes".to_string()))?;
            let mid = MissionId::from_bytes(m_arr);

            let tid: Option<TaskId> = match r.get::<Option<Vec<u8>>, _>("task_id") {
                Some(b) => {
                    let t_arr: [u8; 16] = b
                        .as_slice()
                        .try_into()
                        .map_err(|_| sqlx::Error::Protocol("invalid task_id bytes".to_string()))?;
                    Some(TaskId::from_bytes(t_arr))
                }
                None => None,
            };

            let fp_str: String = r.get("fingerprint_map");
            let file_hashes: BTreeMap<String, String> = serde_json::from_str(&fp_str)
                .map_err(|e| sqlx::Error::Protocol(format!("corrupt fingerprint json: {e}")))?;

            let created_str: String = r.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

            Ok(Some(Self {
                id,
                mission_id: mid,
                task_id: tid,
                git_commit: r.get("git_commit"),
                file_hashes,
                created_at,
            }))
        } else {
            Ok(None)
        }
    }
}

/// Structured drift reconciliation report (D-11, REP-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftReport {
    pub has_drift: bool,
    pub unexpected_additions: Vec<String>,
    pub unexpected_modifications: Vec<String>,
    pub unexpected_deletions: Vec<String>,
    pub authorized_mutation_count: usize,
}

/// Reconcile current workspace file hashes against baseline plus task-authorized mutation scope.
///
/// Any file in `authorized_mutations` is excluded from drift reporting.
/// Any unauthorized addition, modification, or removal is flagged as unexpected drift.
pub fn detect_drift(
    baseline: &RepositoryBaseline,
    current_hashes: &BTreeMap<String, String>,
    authorized_mutations: &BTreeSet<String>,
) -> DriftReport {
    let mut unexpected_additions = Vec::new();
    let mut unexpected_modifications = Vec::new();
    let mut unexpected_deletions = Vec::new();

    // Check additions and modifications
    for (path, current_hash) in current_hashes {
        if authorized_mutations.contains(path) {
            continue;
        }

        match baseline.file_hashes.get(path) {
            Some(base_hash) => {
                if base_hash != current_hash {
                    unexpected_modifications.push(path.clone());
                }
            }
            None => {
                unexpected_additions.push(path.clone());
            }
        }
    }

    // Check deletions
    for path in baseline.file_hashes.keys() {
        if authorized_mutations.contains(path) {
            continue;
        }

        if !current_hashes.contains_key(path) {
            unexpected_deletions.push(path.clone());
        }
    }

    let has_drift = !unexpected_additions.is_empty()
        || !unexpected_modifications.is_empty()
        || !unexpected_deletions.is_empty();

    DriftReport {
        has_drift,
        unexpected_additions,
        unexpected_modifications,
        unexpected_deletions,
        authorized_mutation_count: authorized_mutations.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_capture_and_drift_detection() {
        let dir = tempdir().unwrap();
        let ws = dir.path();

        let f1 = ws.join("src").join("main.rs");
        fs::create_dir_all(f1.parent().unwrap()).unwrap();
        fs::write(&f1, "fn main() {}").unwrap();

        let f2 = ws.join("src").join("lib.rs");
        fs::write(&f2, "pub fn lib() {}").unwrap();

        let mid = MissionId::new();
        let baseline = RepositoryBaseline::capture(ws, mid, None, Some("commit_abc".to_string()))
            .expect("baseline capture succeeds");

        assert_eq!(baseline.file_hashes.len(), 2);
        assert!(baseline.file_hashes.contains_key("src/main.rs"));
        assert!(baseline.file_hashes.contains_key("src/lib.rs"));

        // Case 1: No changes -> no drift
        let mut current_hashes = baseline.file_hashes.clone();
        let authorized = BTreeSet::new();
        let report = detect_drift(&baseline, &current_hashes, &authorized);
        assert!(!report.has_drift);

        // Case 2: Authorized mutation -> no drift
        current_hashes.insert("src/main.rs".to_string(), "new_hash_123".to_string());
        let mut authorized = BTreeSet::new();
        authorized.insert("src/main.rs".to_string());
        let report = detect_drift(&baseline, &current_hashes, &authorized);
        assert!(!report.has_drift);

        // Case 3: Unauthorized modification -> drift flagged
        current_hashes.insert("src/lib.rs".to_string(), "tampered_hash".to_string());
        let report = detect_drift(&baseline, &current_hashes, &authorized);
        assert!(report.has_drift);
        assert_eq!(
            report.unexpected_modifications,
            vec!["src/lib.rs".to_string()]
        );

        // Case 4: Unauthorized addition
        current_hashes.insert("src/extra.rs".to_string(), "extra_hash".to_string());
        let report = detect_drift(&baseline, &current_hashes, &authorized);
        assert!(report.has_drift);
        assert!(
            report
                .unexpected_additions
                .contains(&"src/extra.rs".to_string())
        );

        // Case 5: Unauthorized deletion
        current_hashes.remove("src/lib.rs");
        let report = detect_drift(&baseline, &current_hashes, &authorized);
        assert!(report.has_drift);
        assert!(
            report
                .unexpected_deletions
                .contains(&"src/lib.rs".to_string())
        );
    }

    #[tokio::test]
    async fn test_baseline_sqlite_persistence() {
        use crate::persistence::sqlite::schema::run_migrations;
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();

        let mid = MissionId::new();

        // Seed mission parent record to satisfy foreign key
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO missions (id, objective, status, created_at, updated_at)
            VALUES (?, 'Verify drift baseline', 'running', ?, ?)
            "#,
        )
        .bind(mid.as_bytes().as_slice())
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let mut hashes = BTreeMap::new();
        hashes.insert("file1.rs".to_string(), "h1".to_string());
        let baseline = RepositoryBaseline::new(mid, None, Some("git_rev_1".to_string()), hashes);

        baseline.save_to_db(&pool).await.unwrap();

        let loaded = RepositoryBaseline::load_latest_for_mission(&pool, mid)
            .await
            .unwrap()
            .expect("baseline loaded");

        assert_eq!(loaded.id, baseline.id);
        assert_eq!(loaded.mission_id, mid);
        assert_eq!(loaded.git_commit.as_deref(), Some("git_rev_1"));
        assert_eq!(loaded.file_hashes.get("file1.rs").unwrap(), "h1");
    }

    #[test]
    fn test_baseline_capture_symlink_confinement_and_cycle_prevention() {
        use tempfile::tempdir;

        let outer = tempdir().unwrap();
        let ws = outer.path().join("workspace");
        std::fs::create_dir_all(&ws).unwrap();

        // 1. Outside workspace file and directory
        let outside_file = outer.path().join("outside_secret.txt");
        std::fs::write(&outside_file, b"secret_outside_data").unwrap();
        let outside_dir = outer.path().join("outside_dir");
        std::fs::create_dir_all(&outside_dir).unwrap();
        std::fs::write(outside_dir.join("inner.txt"), b"outside_inner_data").unwrap();

        // 2. Normal legitimate workspace file
        let inside_file = ws.join("normal.txt");
        std::fs::write(&inside_file, b"normal_data").unwrap();

        // 3. Symlink to outside file
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside_file, ws.join("link_to_outside_file")).unwrap();
            std::os::unix::fs::symlink(&outside_dir, ws.join("link_to_outside_dir")).unwrap();

            // 4. Recursive symlink loop: link -> .
            std::os::unix::fs::symlink(&ws, ws.join("loop_self")).unwrap();

            // 5. Parent directory loop: link -> ..
            let sub = ws.join("subdir");
            std::fs::create_dir_all(&sub).unwrap();
            std::os::unix::fs::symlink(&ws, sub.join("loop_parent")).unwrap();

            // 6. Dangling symlink
            std::os::unix::fs::symlink(outer.path().join("nonexistent"), ws.join("dangling"))
                .unwrap();

            // 7. Legitimate internal symlink to normal file
            std::os::unix::fs::symlink(&inside_file, ws.join("link_to_normal")).unwrap();
        }

        let baseline = RepositoryBaseline::capture(&ws, MissionId::new(), None, None).unwrap();

        // Must capture normal.txt
        assert!(
            baseline.file_hashes.contains_key("normal.txt"),
            "normal.txt must be captured"
        );

        // Under Unix, verify confinement invariants
        #[cfg(unix)]
        {
            // Internal symlink to normal file can be captured
            assert!(baseline.file_hashes.contains_key("link_to_normal"));

            // Outside file symlink must NEVER be captured
            assert!(
                !baseline.file_hashes.contains_key("link_to_outside_file"),
                "Outside file must not be captured in workspace baseline"
            );
            // Outside directory contents must NEVER be captured
            assert!(
                !baseline
                    .file_hashes
                    .contains_key("link_to_outside_dir/inner.txt"),
                "Outside directory must not be traversed"
            );

            // None of the captured paths may be absolute or start with outside path
            for path in baseline.file_hashes.keys() {
                assert!(
                    !path.starts_with('/') && !path.contains(".."),
                    "Captured path '{path}' must be a clean relative workspace path"
                );
            }
        }
    }
}
