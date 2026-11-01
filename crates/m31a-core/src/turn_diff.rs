//! Turn Diff Tracker subsystem
//!
//! Tracks turn mutations, computing net unified diffs and object IDs for changed files.
//! Tracks the net text diff for the current turn from committed tool/patch mutations,
//! generating git-compatible unified diffs, index hashes (SHA-1 object IDs),
//! and net line mutation counts (+X / -Y) without rereading the workspace filesystem.

use sha1::Digest;
use similar::TextDiff;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const ZERO_OID: &str = "0000000000000000000000000000000000000000";
const DEV_NULL: &str = "/dev/null";
const REGULAR_FILE_MODE: &str = "100644";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrackedPath {
    pub environment_id: String,
    pub path: PathBuf,
}

impl TrackedPath {
    pub fn new(environment_id: impl Into<String>, path: impl AsRef<Path>) -> Self {
        Self {
            environment_id: environment_id.into(),
            path: path.as_ref().to_path_buf(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackedContent {
    pub content: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiffCacheKey {
    left_path: TrackedPath,
    left_revision: Option<u64>,
    right_path: TrackedPath,
    right_revision: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Tracks turn file mutations and renders git-compatible unified diffs and stat lines.
pub struct TurnDiffTracker {
    valid: bool,
    display_roots_by_environment: HashMap<String, PathBuf>,
    baseline_by_path: HashMap<TrackedPath, TrackedContent>,
    current_by_path: HashMap<TrackedPath, TrackedContent>,
    origin_by_current_path: HashMap<TrackedPath, TrackedPath>,
    next_revision: u64,
    rendered_diffs: HashMap<DiffCacheKey, Option<String>>,
    unified_diff: Option<String>,
    stats: DiffStats,
}

impl Default for TurnDiffTracker {
    fn default() -> Self {
        Self {
            valid: true,
            display_roots_by_environment: HashMap::new(),
            baseline_by_path: HashMap::new(),
            current_by_path: HashMap::new(),
            origin_by_current_path: HashMap::new(),
            next_revision: 0,
            rendered_diffs: HashMap::new(),
            unified_diff: None,
            stats: DiffStats::default(),
        }
    }
}

impl TurnDiffTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_environment_display_roots(
        display_roots: impl IntoIterator<Item = (String, PathBuf)>,
    ) -> Self {
        let mut tracker = Self::new();
        tracker.display_roots_by_environment = display_roots.into_iter().collect();
        tracker
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
        self.rendered_diffs.clear();
        self.unified_diff = None;
        self.stats = DiffStats::default();
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn get_unified_diff(&self) -> Option<&str> {
        self.unified_diff.as_deref()
    }

    pub fn stats(&self) -> &DiffStats {
        &self.stats
    }

    pub fn track_add(
        &mut self,
        environment_id: &str,
        path: impl AsRef<Path>,
        content: &str,
        overwritten_content: Option<&str>,
    ) {
        if !self.valid {
            return;
        }
        let tracked_path = TrackedPath::new(environment_id, path);
        self.origin_by_current_path.remove(&tracked_path);
        if !self.current_by_path.contains_key(&tracked_path)
            && !self.baseline_by_path.contains_key(&tracked_path)
            && let Some(overwritten) = overwritten_content
        {
            let overwritten_entry = self.alloc_tracked_content(overwritten);
            self.baseline_by_path
                .insert(tracked_path.clone(), overwritten_entry);
        }
        let content_entry = self.alloc_tracked_content(content);
        self.current_by_path.insert(tracked_path, content_entry);
        self.refresh();
    }

    pub fn track_delete(&mut self, environment_id: &str, path: impl AsRef<Path>, content: &str) {
        if !self.valid {
            return;
        }
        let tracked_path = TrackedPath::new(environment_id, path);
        if self.current_by_path.remove(&tracked_path).is_none()
            && !self.baseline_by_path.contains_key(&tracked_path)
        {
            let content_entry = self.alloc_tracked_content(content);
            self.baseline_by_path
                .insert(tracked_path.clone(), content_entry);
        }
        self.origin_by_current_path.remove(&tracked_path);
        self.refresh();
    }

    pub fn track_update(
        &mut self,
        environment_id: &str,
        path: impl AsRef<Path>,
        move_path: Option<PathBuf>,
        old_content: &str,
        overwritten_move_content: Option<&str>,
        new_content: &str,
    ) {
        if !self.valid {
            return;
        }
        let source_path = TrackedPath::new(environment_id, path);
        if !self.current_by_path.contains_key(&source_path)
            && !self.baseline_by_path.contains_key(&source_path)
        {
            let old_entry = self.alloc_tracked_content(old_content);
            self.baseline_by_path.insert(source_path.clone(), old_entry);
        }

        match move_path {
            Some(dest) => {
                let dest_path = TrackedPath::new(environment_id, dest);
                if !self.current_by_path.contains_key(&dest_path)
                    && !self.baseline_by_path.contains_key(&dest_path)
                    && let Some(overwritten_move) = overwritten_move_content
                {
                    let overwritten_entry = self.alloc_tracked_content(overwritten_move);
                    self.baseline_by_path
                        .insert(dest_path.clone(), overwritten_entry);
                }
                let origin = self
                    .origin_by_current_path
                    .remove(&source_path)
                    .unwrap_or_else(|| source_path.clone());
                self.current_by_path.remove(&source_path);
                let new_entry = self.alloc_tracked_content(new_content);
                self.current_by_path.insert(dest_path.clone(), new_entry);
                self.origin_by_current_path.remove(&dest_path);
                if dest_path != origin {
                    self.origin_by_current_path.insert(dest_path, origin);
                }
            }
            None => {
                let new_entry = self.alloc_tracked_content(new_content);
                self.current_by_path.insert(source_path, new_entry);
            }
        }
        self.refresh();
    }

    fn alloc_tracked_content(&mut self, content: &str) -> TrackedContent {
        let rev = self.next_revision;
        self.next_revision += 1;
        TrackedContent {
            content: content.to_string(),
            revision: rev,
        }
    }

    fn rename_pairs(&self) -> HashMap<TrackedPath, TrackedPath> {
        self.origin_by_current_path
            .iter()
            .filter_map(|(dest_path, origin_path)| {
                if dest_path == origin_path
                    || self.current_by_path.contains_key(origin_path)
                    || !self.current_by_path.contains_key(dest_path)
                    || !self.baseline_by_path.contains_key(origin_path)
                    || self.baseline_by_path.contains_key(dest_path)
                {
                    return None;
                }
                Some((origin_path.clone(), dest_path.clone()))
            })
            .collect()
    }

    fn refresh(&mut self) {
        let rename_pairs = self.rename_pairs();
        let paired_destinations = rename_pairs.values().cloned().collect::<HashSet<_>>();
        let mut handled = HashSet::new();

        let mut paths: Vec<TrackedPath> = self
            .baseline_by_path
            .keys()
            .chain(self.current_by_path.keys())
            .cloned()
            .collect();
        paths.sort_by_key(|p| self.display_path(p));
        paths.dedup();

        let mut previous_diffs = std::mem::take(&mut self.rendered_diffs);
        let mut rendered_diffs = HashMap::new();
        let mut aggregated = String::new();
        let mut stats = DiffStats::default();

        for path in paths {
            if !handled.insert(path.clone()) {
                continue;
            }
            if paired_destinations.contains(&path) {
                continue;
            }

            let (left_path, right_path) = if let Some(dest) = rename_pairs.get(&path) {
                handled.insert(dest.clone());
                (&path, dest)
            } else {
                (&path, &path)
            };

            let left_content = self.baseline_by_path.get(left_path);
            let right_content = self.current_by_path.get(right_path);

            let key = DiffCacheKey {
                left_path: left_path.clone(),
                left_revision: left_content.map(|c| c.revision),
                right_path: right_path.clone(),
                right_revision: right_content.map(|c| c.revision),
            };

            let rendered = previous_diffs.remove(&key).unwrap_or_else(|| {
                self.render_diff(
                    left_path,
                    left_content.map(|c| c.content.as_str()),
                    right_path,
                    right_content.map(|c| c.content.as_str()),
                )
            });

            if let Some(diff) = rendered.as_deref() {
                aggregated.push_str(diff);
                if !aggregated.ends_with('\n') {
                    aggregated.push('\n');
                }
                stats.files_changed += 1;
                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        stats.lines_added += 1;
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        stats.lines_removed += 1;
                    }
                }
            }
            rendered_diffs.insert(key, rendered);
        }

        self.rendered_diffs = rendered_diffs;
        self.unified_diff = if aggregated.is_empty() {
            None
        } else {
            Some(aggregated)
        };
        self.stats = stats;
    }

    fn render_diff(
        &self,
        left_path: &TrackedPath,
        left_content: Option<&str>,
        right_path: &TrackedPath,
        right_content: Option<&str>,
    ) -> Option<String> {
        if left_content == right_content {
            return None;
        }

        let left_display = self.display_path(left_path).replace('\\', "/");
        let right_display = self.display_path(right_path).replace('\\', "/");

        let left_oid = left_content
            .map(|c| git_blob_oid(c.as_bytes()))
            .unwrap_or_else(|| ZERO_OID.to_string());
        let right_oid = right_content
            .map(|c| git_blob_oid(c.as_bytes()))
            .unwrap_or_else(|| ZERO_OID.to_string());

        let mut diff = format!("diff --git a/{left_display} b/{right_display}\n");
        match (left_content, right_content) {
            (None, Some(_)) => diff.push_str(&format!("new file mode {REGULAR_FILE_MODE}\n")),
            (Some(_), None) => diff.push_str(&format!("deleted file mode {REGULAR_FILE_MODE}\n")),
            (Some(_), Some(_)) => {
                if left_display != right_display {
                    diff.push_str(&format!("rename from {left_display}\n"));
                    diff.push_str(&format!("rename to {right_display}\n"));
                }
            }
            (None, None) => return None,
        }

        diff.push_str(&format!("index {left_oid}..{right_oid}\n"));

        let old_header = if left_content.is_some() {
            format!("a/{left_display}")
        } else {
            DEV_NULL.to_string()
        };
        let new_header = if right_content.is_some() {
            format!("b/{right_display}")
        } else {
            DEV_NULL.to_string()
        };

        let left_str = left_content.unwrap_or("");
        let right_str = right_content.unwrap_or("");

        let text_diff = TextDiff::from_lines(left_str, right_str);
        let unified = text_diff
            .unified_diff()
            .context_radius(3)
            .header(&old_header, &new_header)
            .to_string();

        diff.push_str(&unified);
        Some(diff)
    }

    fn display_path(&self, path: &TrackedPath) -> String {
        if let Some(root) = self.display_roots_by_environment.get(&path.environment_id)
            && let Ok(rel) = path.path.strip_prefix(root)
        {
            return rel.to_string_lossy().to_string();
        }
        path.path.to_string_lossy().to_string()
    }
}

/// Compute standard Git SHA-1 blob object ID ("blob <len>\0<data>").
pub fn git_blob_oid(data: &[u8]) -> String {
    let header = format!("blob {}\0", data.len());
    let mut hasher = sha1::Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(data);
    let result = hasher.finalize();
    format!("{:040x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_blob_oid() {
        // "blob 5\0hello"
        let oid = git_blob_oid(b"hello");
        assert_eq!(oid, "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0");
    }

    #[test]
    fn test_turn_diff_add_file() {
        let mut tracker = TurnDiffTracker::new();
        tracker.track_add("env0", "foo.txt", "line1\nline2\n", None);
        assert!(tracker.is_valid());
        let diff = tracker.get_unified_diff().expect("diff should exist");
        assert!(diff.contains("diff --git a/foo.txt b/foo.txt"));
        assert!(diff.contains("new file mode 100644"));
        assert!(diff.contains("+line1"));
        assert!(diff.contains("+line2"));
        assert_eq!(tracker.stats().files_changed, 1);
        assert_eq!(tracker.stats().lines_added, 2);
    }

    #[test]
    fn test_turn_diff_update_file() {
        let mut tracker = TurnDiffTracker::new();
        tracker.track_update(
            "env0",
            "bar.rs",
            None,
            "fn main() {}\n",
            None,
            "fn main() {\n    println!(\"hello\");\n}\n",
        );
        let diff = tracker.get_unified_diff().expect("diff should exist");
        assert!(diff.contains("--- a/bar.rs"));
        assert!(diff.contains("+++ b/bar.rs"));
        assert!(diff.contains("+    println!(\"hello\");"));
    }

    #[test]
    fn test_turn_diff_rename_file() {
        let mut tracker = TurnDiffTracker::new();
        tracker.track_update(
            "env0",
            "old_name.txt",
            Some(PathBuf::from("new_name.txt")),
            "identical content\n",
            None,
            "identical content with modification\n",
        );
        let diff = tracker.get_unified_diff().expect("diff should exist");
        assert!(diff.contains("rename from old_name.txt"));
        assert!(diff.contains("rename to new_name.txt"));
    }

    #[test]
    fn test_turn_diff_delete_file() {
        let mut tracker = TurnDiffTracker::new();
        tracker.track_delete("env0", "obsolete.md", "# Obsolete Docs\n");
        let diff = tracker.get_unified_diff().expect("diff should exist");
        assert!(diff.contains("deleted file mode 100644"));
        assert!(diff.contains("-# Obsolete Docs"));
    }
}
