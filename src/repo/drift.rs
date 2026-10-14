use crate::ids::{MissionId, TaskId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Maximum directory nesting traversed during baseline capture (F-08).
/// Deeper nesting is reported as an incomplete-capture error rather than
/// traversed unboundedly.
pub const MAX_BASELINE_CAPTURE_DEPTH: usize = 128;

/// Directories never part of the workspace content baseline (build outputs,
/// dependency stores, VCS/runtime control state). Intentional policy
/// exclusions — recorded in the capture report, never confused with
/// inspection failures.
pub const BASELINE_EXCLUDED_DIRS: &[&str] = &[
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

/// Detailed outcome of a workspace baseline capture (F-08).
///
/// An incomplete baseline (`complete == false`) MUST NEVER be interpreted
/// as evidence that no drift occurred: every downstream consumer must
/// degrade explicitly (repair/reject) instead of reporting a clean bill.
#[derive(Debug, Clone)]
pub struct BaselineCaptureReport {
    /// Captured file hashes (only fully-inspected entries).
    pub baseline: RepositoryBaseline,
    /// True only when every encountered entry was either hashed or
    /// intentionally excluded by policy with zero inspection failures.
    pub complete: bool,
    /// Intentionally excluded entries (policy): `reason: relative_path`.
    /// Capped for size; see `excluded_total`.
    pub excluded: Vec<String>,
    /// Total intentional exclusions (including beyond the `excluded` sample).
    pub excluded_total: usize,
    /// Entries that could NOT be inspected (failures). Non-empty implies
    /// `complete == false`. Capped for size; see `skipped_total`.
    pub skipped_errors: Vec<String>,
    /// Total inspection failures (including beyond the sample).
    pub skipped_total: usize,
}

impl BaselineCaptureReport {
    fn record_excluded(&mut self, reason: &str, rel: &str) {
        self.excluded_total += 1;
        if self.excluded.len() < 100 {
            self.excluded.push(format!("{reason}: {rel}"));
        }
    }

    fn record_skipped(&mut self, detail: String) {
        self.skipped_total += 1;
        self.complete = false;
        if self.skipped_errors.len() < 100 {
            self.skipped_errors.push(detail);
        }
    }
}

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
    ///
    /// Strict entry point (F-08/F-14): succeeds ONLY when the capture is
    /// complete and trustworthy. Any inspection failure (unreadable
    /// directory or file, unresolvable symlink, vanishing path, excessive
    /// depth) returns `Err` instead of a baseline that silently omits
    /// content. Callers needing partial data must use
    /// [`Self::capture_with_report`] and check `complete` explicitly.
    pub fn capture(
        workspace_root: &Path,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        git_commit: Option<String>,
    ) -> Result<Self, std::io::Error> {
        let report = Self::capture_with_report(workspace_root, mission_id, task_id, git_commit)?;
        if !report.complete {
            return Err(std::io::Error::other(format!(
                "workspace baseline incomplete: {} entries could not be inspected ({}); refusing to present a partial baseline as complete",
                report.skipped_total,
                report.skipped_errors.join("; ")
            )));
        }
        Ok(report.baseline)
    }

    /// Capture with a full completeness report (F-08).
    ///
    /// Symlink and filesystem-entry policy:
    ///
    /// - Directory symlinks are NEVER traversed (no recursive symlink
    ///   traversal by default; symlink loops and cycles are impossible).
    /// - A symlink whose canonical target escapes the workspace root is
    ///   excluded by policy (external content belongs to its own scope).
    /// - A symlink into protected control state (`.git`, `.m31a`) is
    ///   excluded by policy — a harmless-looking internal link can never
    ///   smuggle protected content into (or out of) the baseline.
    /// - File symlinks confined to unprotected workspace content are hashed
    ///   under the link's workspace-relative path.
    /// - Directory symlinks are never traversed; the raw link target is
    ///   fingerprinted instead, so retargeting is detected without recursion.
    /// - Dangling symlinks fingerprint their raw link target (the link
    ///   itself is workspace state); only a link vanishing mid-capture is
    ///   an inspection failure.
    /// - Non-regular files (fifos, sockets, devices) are excluded by policy
    ///   (hashing them could block forever); regular files that cannot be
    ///   read, directories that cannot be listed, and paths that vanish
    ///   mid-capture are inspection failures (incomplete).
    pub fn capture_with_report(
        workspace_root: &Path,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        git_commit: Option<String>,
    ) -> Result<BaselineCaptureReport, std::io::Error> {
        // The containment boundary must be established, never assumed: an
        // uncanonicalizable root fails closed instead of weakening every
        // symlink check below.
        let canonical_base = workspace_root.canonicalize().map_err(|e| {
            std::io::Error::other(format!(
                "cannot establish workspace boundary for '{}': {e}",
                workspace_root.display()
            ))
        })?;
        let mut report = BaselineCaptureReport {
            baseline: Self::new(mission_id, task_id, git_commit, BTreeMap::new()),
            complete: true,
            excluded: Vec::new(),
            excluded_total: 0,
            skipped_total: 0,
            skipped_errors: Vec::new(),
        };
        let mut visited = std::collections::HashSet::new();
        visited.insert(canonical_base.clone());

        Self::collect_hashes(
            workspace_root,
            &canonical_base,
            workspace_root,
            0,
            &mut visited,
            &mut report,
        )?;

        Ok(report)
    }

    fn collect_hashes(
        base: &Path,
        canonical_base: &Path,
        dir: &Path,
        depth: usize,
        visited: &mut std::collections::HashSet<std::path::PathBuf>,
        report: &mut BaselineCaptureReport,
    ) -> Result<(), std::io::Error> {
        if depth > MAX_BASELINE_CAPTURE_DEPTH {
            let rel = dir
                .strip_prefix(base)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| dir.display().to_string());
            report.record_skipped(format!(
                "maximum capture depth ({MAX_BASELINE_CAPTURE_DEPTH}) exceeded at '{rel}': traversal not bounded-safe"
            ));
            return Ok(());
        }
        let read_dir = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                let rel = dir
                    .strip_prefix(base)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| dir.display().to_string());
                // F-08: a directory that cannot be listed is an inspection
                // failure, never a silent skip. Siblings are still
                // preserved; the report is marked incomplete.
                report.record_skipped(format!("cannot list directory '{rel}': {e}"));
                return Ok(());
            }
        };

        for entry in read_dir {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    report.record_skipped(format!(
                        "cannot read directory entry in '{}': {e}",
                        dir.display()
                    ));
                    continue;
                }
            };
            let path = entry.path();
            let rel_path = path
                .strip_prefix(base)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.display().to_string());
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            if BASELINE_EXCLUDED_DIRS.iter().any(|ex| ex == &name_str) {
                report.record_excluded("policy-excluded directory", &rel_path);
                continue;
            }
            // Defense in depth: any protected component anywhere in the
            // workspace-relative path is excluded even if the leaf name is
            // innocuous.
            if crate::kernel::invariants::contains_protected_component(Path::new(&rel_path)) {
                report.record_excluded("protected path component", &rel_path);
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    report.record_skipped(format!("cannot inspect file type of '{rel_path}': {e}"));
                    continue;
                }
            };

            if file_type.is_symlink() {
                // Symlink policy (F-08): resolve once, never traverse.
                match path.canonicalize() {
                    Ok(canon_target) => {
                        // Escape check: target must be confined within the workspace root.
                        if !canon_target.starts_with(canonical_base) {
                            tracing::debug!(
                                path = %path.display(),
                                target = %canon_target.display(),
                                "Skipping symlink escaping workspace baseline boundary"
                            );
                            report.record_excluded("symlink escapes workspace", &rel_path);
                            continue;
                        }

                        // A harmless-looking internal symlink must never expose
                        // protected `.git` / `.m31a` content through the baseline.
                        if crate::kernel::invariants::contains_protected_component(
                            canon_target
                                .strip_prefix(canonical_base)
                                .unwrap_or(&canon_target),
                        ) {
                            tracing::debug!(
                                path = %path.display(),
                                target = %canon_target.display(),
                                "Skipping symlink into protected control state"
                            );
                            report.record_excluded("symlink targets protected state", &rel_path);
                            continue;
                        }

                        if canon_target.is_file() {
                            match fs::read(&canon_target) {
                                Ok(bytes) => {
                                    let mut hasher = Sha256::new();
                                    hasher.update(&bytes);
                                    let hash = format!("{:x}", hasher.finalize());
                                    report.baseline.file_hashes.insert(rel_path, hash);
                                }
                                Err(e) => {
                                    report.record_skipped(format!(
                                        "cannot read symlink target of '{rel_path}': {e}"
                                    ));
                                }
                            }
                        } else if canon_target.is_dir() {
                            // Directory symlinks are never traversed: content
                            // under an alias is out of baseline scope, and
                            // refusing recursion eliminates symlink loops and
                            // cycles. The link target itself IS fingerprinted
                            // (raw target string) so retargeting is detected
                            // as drift without ever entering the target.
                            match fs::read_link(&path) {
                                Ok(target) => {
                                    let mut hasher = Sha256::new();
                                    hasher.update(b"symlink:");
                                    hasher.update(target.to_string_lossy().as_bytes());
                                    let hash = format!("{:x}", hasher.finalize());
                                    report.baseline.file_hashes.insert(rel_path, hash);
                                }
                                Err(e) => {
                                    report.record_skipped(format!(
                                        "cannot read directory symlink '{rel_path}': {e}"
                                    ));
                                }
                            }
                        } else {
                            report.record_excluded("symlink to non-regular file", &rel_path);
                        }
                    }
                    Err(_) => {
                        // Unresolvable (dangling) symlink: no target content
                        // exists to hash, but the link itself is workspace
                        // state. Fingerprint the raw link target so
                        // retargeting or removal is detected as drift. Only
                        // a link that vanishes mid-capture is an inspection
                        // failure.
                        match fs::read_link(&path) {
                            Ok(target) => {
                                let mut hasher = Sha256::new();
                                hasher.update(b"symlink:");
                                hasher.update(target.to_string_lossy().as_bytes());
                                let hash = format!("{:x}", hasher.finalize());
                                report.baseline.file_hashes.insert(rel_path, hash);
                            }
                            Err(e) => {
                                report.record_skipped(format!(
                                    "symlink '{rel_path}' disappeared during capture: {e}"
                                ));
                            }
                        }
                        continue;
                    }
                }
            } else if file_type.is_dir() {
                let canon_dir = match path.canonicalize() {
                    Ok(c) => c,
                    Err(e) => {
                        report
                            .record_skipped(format!("cannot resolve directory '{rel_path}': {e}"));
                        continue;
                    }
                };
                if !canon_dir.starts_with(canonical_base) {
                    report.record_excluded("directory escapes workspace", &rel_path);
                    continue;
                }
                if visited.insert(canon_dir) {
                    Self::collect_hashes(base, canonical_base, &path, depth + 1, visited, report)?;
                }
            } else if file_type.is_file() {
                match fs::read(&path) {
                    Ok(bytes) => {
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        let hash = format!("{:x}", hasher.finalize());
                        report.baseline.file_hashes.insert(rel_path, hash);
                    }
                    Err(e) => {
                        report.record_skipped(format!("cannot read file '{rel_path}': {e}"));
                    }
                }
            } else {
                // Sockets, fifos, devices and other non-regular files are
                // excluded by policy (reading them could block forever).
                report.record_excluded("non-regular file", &rel_path);
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
