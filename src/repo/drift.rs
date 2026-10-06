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

        Self::collect_hashes(
            workspace_root,
            workspace_root,
            &excluded_dirs,
            &mut file_hashes,
        )?;

        Ok(Self::new(mission_id, task_id, git_commit, file_hashes))
    }

    fn collect_hashes(
        base: &Path,
        dir: &Path,
        excluded: &[&str],
        acc: &mut BTreeMap<String, String>,
    ) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            if excluded.iter().any(|ex| ex == &name_str) {
                continue;
            }

            if path.is_dir() {
                Self::collect_hashes(base, &path, excluded, acc)?;
            } else if path.is_file()
                && let Ok(bytes) = fs::read(&path)
            {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let hash = format!("{:x}", hasher.finalize());

                let rel_path = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");

                acc.insert(rel_path, hash);
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
            INSERT INTO repository_baselines (id, mission_id, task_id, git_commit, fingerprint_map, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.id.as_bytes().as_slice())
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
            SELECT id, mission_id, task_id, git_commit, fingerprint_map, created_at
            FROM repository_baselines
            WHERE mission_id = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
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
}
