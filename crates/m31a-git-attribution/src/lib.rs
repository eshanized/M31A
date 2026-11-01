//! Git Attribution and Provenance Tracking
//!
//! Tracks code authorship, blame information, and provenance metadata
//! for audit trails and collaboration insights.

use anyhow::{Context, Result};
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Builder for standardized AI co-authorship and provenance Git commit trailers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitTrailerBuilder {
    pub agent_name: String,
    pub agent_email: String,
    pub session_id: Option<String>,
    pub model_id: Option<String>,
    pub phase_or_wave: Option<String>,
    pub custom_trailers: HashMap<String, String>,
}

impl Default for CommitTrailerBuilder {
    fn default() -> Self {
        Self {
            agent_name: "M31A".to_string(),
            agent_email: "m31a@local".to_string(),
            session_id: None,
            model_id: None,
            phase_or_wave: None,
            custom_trailers: HashMap::new(),
        }
    }
}

impl CommitTrailerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    pub fn with_model_id(mut self, model: impl Into<String>) -> Self {
        self.model_id = Some(model.into());
        self
    }

    pub fn with_phase_or_wave(mut self, wave: impl Into<String>) -> Self {
        self.phase_or_wave = Some(wave.into());
        self
    }

    pub fn with_trailer(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_trailers.insert(key.into(), value.into());
        self
    }

    /// Renders Git trailers in canonical format.
    pub fn render_trailers(&self) -> String {
        let mut trailers = Vec::new();
        trailers.push(format!(
            "Co-authored-by: {} <{}>",
            self.agent_name, self.agent_email
        ));

        if let Some(ref session) = self.session_id {
            trailers.push(format!("X-AI-Session: {}", session));
        }
        if let Some(ref model) = self.model_id {
            trailers.push(format!("X-AI-Model: {}", model));
        }
        if let Some(ref wave) = self.phase_or_wave {
            trailers.push(format!("X-AI-Wave: {}", wave));
        }

        for (k, v) in &self.custom_trailers {
            trailers.push(format!("{}: {}", k, v));
        }

        trailers.join("\n")
    }

    /// Composes a headline, optional body, and standardized trailers into a complete commit message.
    pub fn compose_commit_message(&self, headline: &str, body: Option<&str>) -> String {
        let mut msg = headline.trim().to_string();
        msg.push('\n');

        if let Some(b) = body {
            let b_trimmed = b.trim();
            if !b_trimmed.is_empty() {
                msg.push('\n');
                msg.push_str(b_trimmed);
                msg.push('\n');
            }
        }

        msg.push('\n');
        msg.push_str(&self.render_trailers());
        msg.push('\n');
        msg
    }

    /// Extracts key-value trailers from an existing commit message.
    pub fn parse_trailers(commit_message: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in commit_message.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                // Trailers are separated by empty lines
                if !map.is_empty() {
                    break;
                }
                continue;
            }

            if let Some((key, val)) = line.split_once(':') {
                let k = key.trim().to_string();
                let v = val.trim().to_string();
                if !k.contains(' ') || k.eq_ignore_ascii_case("Co-authored-by") {
                    map.insert(k, v);
                }
            }
        }
        map
    }
}

/// Git author information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub name: String,
    pub email: String,
    pub commit_count: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub first_commit: u64,
    pub last_commit: u64,
}

/// File attribution data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAttribution {
    pub path: String,
    pub total_lines: usize,
    pub authors: Vec<LineAttribution>,
    pub last_modified: u64,
    pub commit_count: usize,
}

/// Line-level attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineAttribution {
    pub author: String,
    pub email: String,
    pub lines: usize,
    pub percentage: f32,
}

/// Commit provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitProvenance {
    pub oid: String,
    pub short_oid: String,
    pub author: AuthorInfo,
    pub committer: AuthorInfo,
    pub message: String,
    pub timestamp: u64,
    pub parents: Vec<String>,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub is_merge: bool,
}

/// Blame information for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameInfo {
    pub file_path: String,
    pub hunks: Vec<BlameHunk>,
}

/// Blame hunk (contiguous lines by same author)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameHunk {
    pub start_line: usize,
    pub end_line: usize,
    pub author: String,
    pub email: String,
    pub commit_oid: String,
    pub commit_short_oid: String,
    pub commit_message: String,
    pub commit_time: u64,
}

/// Repository provenance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoProvenance {
    pub repo_path: PathBuf,
    pub total_commits: usize,
    pub total_contributors: usize,
    pub lines_of_code: usize,
    pub first_commit: u64,
    pub last_commit: u64,
    pub top_contributors: Vec<AuthorInfo>,
    pub file_attributions: Vec<FileAttribution>,
    pub recent_commits: Vec<CommitProvenance>,
}

/// Git attribution tracker
pub struct GitAttribution {
    repo: Repository,
    repo_path: PathBuf,
}

impl GitAttribution {
    /// Open a repository for attribution tracking
    pub fn open(repo_path: &Path) -> Result<Self> {
        let repo = Repository::open(repo_path)
            .with_context(|| format!("Failed to open git repository at {}", repo_path.display()))?;
        Ok(Self {
            repo,
            repo_path: repo_path.to_path_buf(),
        })
    }

    /// Get repository provenance summary
    pub fn get_provenance(&self, max_commits: usize) -> Result<RepoProvenance> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        let mut author_stats: HashMap<String, AuthorInfo> = HashMap::new();
        let mut file_stats: HashMap<String, FileAttribution> = HashMap::new();

        for (count, oid_result) in revwalk.enumerate() {
            if count >= max_commits {
                break;
            }
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;

            let provenance = self.commit_to_provenance(&commit)?;
            commits.push(provenance.clone());

            // Update author stats
            let author_key = format!("{} <{}>", provenance.author.name, provenance.author.email);
            let stats = author_stats
                .entry(author_key.clone())
                .or_insert_with(|| AuthorInfo {
                    name: provenance.author.name.clone(),
                    email: provenance.author.email.clone(),
                    commit_count: 0,
                    lines_added: 0,
                    lines_removed: 0,
                    first_commit: provenance.timestamp,
                    last_commit: provenance.timestamp,
                });
            stats.commit_count += 1;
            stats.lines_added += provenance.insertions;
            stats.lines_removed += provenance.deletions;
            stats.first_commit = stats.first_commit.min(provenance.timestamp);
            stats.last_commit = stats.last_commit.max(provenance.timestamp);
        }

        // Get file attributions
        self.analyze_file_attributions(&mut file_stats, max_commits)?;

        let mut ranked: Vec<AuthorInfo> = author_stats.into_values().collect();
        ranked.sort_by_key(|a| std::cmp::Reverse(a.commit_count));
        let top_contributors: Vec<AuthorInfo> = ranked.into_iter().take(10).collect();

        let total_commits = self.count_total_commits()?;
        let total_contributors = self.count_contributors()?;
        let lines_of_code = self.count_lines_of_code()?;

        Ok(RepoProvenance {
            repo_path: self.repo_path.clone(),
            total_commits,
            total_contributors,
            lines_of_code,
            first_commit: commits.last().map(|c| c.timestamp).unwrap_or(0),
            last_commit: commits.first().map(|c| c.timestamp).unwrap_or(0),
            top_contributors,
            file_attributions: file_stats.into_values().collect(),
            recent_commits: commits,
        })
    }

    /// Get blame for a specific file
    pub fn get_blame(&self, file_path: &Path) -> Result<BlameInfo> {
        let blame = self.repo.blame_file(file_path, None)?;
        let mut hunks = Vec::new();

        for hunk in blame.iter() {
            let signature = hunk.final_signature();
            let commit = hunk.final_commit_id();
            let commit_obj = self.repo.find_commit(commit)?;
            let start_line = hunk.final_start_line();

            hunks.push(BlameHunk {
                start_line,
                end_line: start_line + hunk.lines_in_hunk() - 1,
                author: signature
                    .as_ref()
                    .and_then(|s| s.name().ok())
                    .unwrap_or("Unknown")
                    .to_string(),
                email: signature
                    .as_ref()
                    .and_then(|s| s.email().ok())
                    .unwrap_or("unknown@example.com")
                    .to_string(),
                commit_oid: commit.to_string(),
                commit_short_oid: commit.to_string()[..8.min(commit.to_string().len())].to_string(),
                commit_message: commit_obj
                    .message()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
                commit_time: commit_obj.time().seconds() as u64,
            });
        }

        Ok(BlameInfo {
            file_path: file_path.to_string_lossy().to_string(),
            hunks,
        })
    }

    /// Get commit provenance by OID
    pub fn get_commit(&self, oid_str: &str) -> Result<CommitProvenance> {
        let oid = Oid::from_str(oid_str)?;
        let commit = self.repo.find_commit(oid)?;
        self.commit_to_provenance(&commit)
    }

    /// Get commits by author
    pub fn get_commits_by_author(
        &self,
        author_email: &str,
        limit: usize,
    ) -> Result<Vec<CommitProvenance>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        for oid_result in revwalk {
            if commits.len() >= limit {
                break;
            }
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;

            let author = commit.author();
            let commit_email = author.email().unwrap_or("");
            if commit_email == author_email {
                commits.push(self.commit_to_provenance(&commit)?);
            }
        }
        Ok(commits)
    }

    /// Get commits affecting a file
    pub fn get_file_history(
        &self,
        file_path: &Path,
        limit: usize,
    ) -> Result<Vec<CommitProvenance>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        for oid_result in revwalk {
            if commits.len() >= limit {
                break;
            }
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;

            // Check if commit touches this file
            let tree = commit.tree()?;
            let parent_tree = if commit.parent_count() > 0 {
                Some(commit.parent(0)?.tree()?)
            } else {
                None
            };

            let diff = self.repo.diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&tree),
                Some(git2::DiffOptions::new().pathspec(file_path)),
            )?;

            if diff.deltas().count() > 0 {
                commits.push(self.commit_to_provenance(&commit)?);
            }
        }
        Ok(commits)
    }

    // Private helper methods

    fn commit_to_provenance(&self, commit: &git2::Commit) -> Result<CommitProvenance> {
        let author = commit.author();
        let committer = commit.committer();

        let mut parents = Vec::new();
        for i in 0..commit.parent_count() {
            parents.push(commit.parent(i)?.id().to_string());
        }

        Ok(CommitProvenance {
            oid: commit.id().to_string(),
            short_oid: commit.id().to_string()[..8].to_string(),
            author: AuthorInfo {
                name: author.name().unwrap_or("Unknown").to_string(),
                email: author.email().unwrap_or("unknown@example.com").to_string(),
                commit_count: 0,
                lines_added: 0,
                lines_removed: 0,
                first_commit: 0,
                last_commit: 0,
            },
            committer: AuthorInfo {
                name: committer.name().unwrap_or("Unknown").to_string(),
                email: committer
                    .email()
                    .unwrap_or("unknown@example.com")
                    .to_string(),
                commit_count: 0,
                lines_added: 0,
                lines_removed: 0,
                first_commit: 0,
                last_commit: 0,
            },
            message: commit.message().unwrap_or("").to_string(),
            timestamp: commit.time().seconds() as u64,
            parents,
            files_changed: 0, // Would need diff to compute
            insertions: 0,
            deletions: 0,
            is_merge: commit.parent_count() > 1,
        })
    }

    fn analyze_file_attributions(
        &self,
        file_stats: &mut HashMap<String, FileAttribution>,
        max_commits: usize,
    ) -> Result<()> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        for (count, oid_result) in revwalk.enumerate() {
            if count >= max_commits {
                break;
            }
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;

            let tree = commit.tree()?;
            let parent_tree = if commit.parent_count() > 0 {
                Some(commit.parent(0)?.tree()?)
            } else {
                None
            };

            let diff = self
                .repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

            for delta in diff.deltas() {
                if let Some(path) = delta.new_file().path() {
                    let path_str = path.to_string_lossy().to_string();
                    let entry =
                        file_stats
                            .entry(path_str.clone())
                            .or_insert_with(|| FileAttribution {
                                path: path_str.clone(),
                                total_lines: 0,
                                authors: Vec::new(),
                                last_modified: 0,
                                commit_count: 0,
                            });
                    entry.commit_count += 1;
                    entry.last_modified = entry.last_modified.max(commit.time().seconds() as u64);
                }
            }
        }
        Ok(())
    }

    fn count_total_commits(&self) -> Result<usize> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        Ok(revwalk.count())
    }

    fn count_contributors(&self) -> Result<usize> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut emails = std::collections::HashSet::new();
        for oid_result in revwalk {
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;
            if let Ok(email) = commit.author().email() {
                emails.insert(email.to_string());
            }
        }
        Ok(emails.len())
    }

    fn count_lines_of_code(&self) -> Result<usize> {
        let head = self.repo.head()?.peel_to_tree()?;
        let mut loc = 0;

        fn count_in_tree(tree: &git2::Tree, repo: &Repository, loc: &mut usize) -> Result<()> {
            for entry in tree.iter() {
                if let Some(kind) = entry.kind() {
                    match kind {
                        git2::ObjectType::Blob => {
                            if let Ok(blob) = repo.find_blob(entry.id()) {
                                let content = std::str::from_utf8(blob.content()).unwrap_or("");
                                *loc += content.lines().count();
                            }
                        }
                        git2::ObjectType::Tree => {
                            if let Ok(subtree) = entry.to_object(repo)?.peel_to_tree() {
                                count_in_tree(&subtree, repo, loc)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        }

        count_in_tree(&head, &self.repo, &mut loc)?;
        Ok(loc)
    }
}

/// Extension trait for sorting
#[allow(dead_code)]
trait SortedByKey<T> {
    fn sorted_by_key<F, K>(self, f: F) -> Vec<T>
    where
        F: FnMut(&T) -> K,
        K: Ord;
}

impl<T> SortedByKey<T> for Vec<T> {
    fn sorted_by_key<F, K>(mut self, f: F) -> Vec<T>
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.sort_by_key(f);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_commit_trailer_builder() {
        let builder = CommitTrailerBuilder::new()
            .with_session_id("sess-12345")
            .with_model_id("nvidia/nemotron-3.5-lightning-30b-a3b")
            .with_phase_or_wave("wave-2")
            .with_trailer("X-Verified-By", "Nyquist");

        let msg = builder.compose_commit_message(
            "feat(core): implement new feature",
            Some("Detailed explanation of change."),
        );
        assert!(msg.contains("feat(core): implement new feature"));
        assert!(msg.contains("Detailed explanation of change."));
        assert!(msg.contains("Co-authored-by: M31A <m31a@local>"));
        assert!(msg.contains("X-AI-Session: sess-12345"));
        assert!(msg.contains("X-AI-Model: nvidia/nemotron-3.5-lightning-30b-a3b"));
        assert!(msg.contains("X-AI-Wave: wave-2"));
        assert!(msg.contains("X-Verified-By: Nyquist"));

        let parsed = CommitTrailerBuilder::parse_trailers(&msg);
        assert_eq!(
            parsed.get("Co-authored-by").map(|s| s.as_str()),
            Some("M31A <m31a@local>")
        );
        assert_eq!(
            parsed.get("X-AI-Session").map(|s| s.as_str()),
            Some("sess-12345")
        );
        assert_eq!(parsed.get("X-AI-Wave").map(|s| s.as_str()), Some("wave-2"));
    }

    #[test]
    fn test_git_attribution() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create a file and commit
        std::fs::write(repo_path.join("README.md"), "# Test\n\nContent").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Open and test
        let attribution = GitAttribution::open(repo_path).unwrap();
        let provenance = attribution.get_provenance(10).unwrap();

        assert_eq!(provenance.total_commits, 1);
        assert_eq!(provenance.total_contributors, 1);
        assert!(!provenance.recent_commits.is_empty());
    }
}
