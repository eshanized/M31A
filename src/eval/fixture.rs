//! Programmatic temporary Git fixture repository builder (D-15, TST-03).
//!
//! Provides pristine, isolated Git repositories in temporary directories with
//! strict environment isolation (global and system Git configuration pointed
//! at the platform null device) preventing any contamination from the user's
//! host Git configuration.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Represents an isolated temporary Git repository fixture.
pub struct FixtureRepo {
    _dir: TempDir,
    path: PathBuf,
    head_commit: String,
}

impl FixtureRepo {
    /// Absolute path to the fixture repository root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Latest commit hash (HEAD) of the repository.
    pub fn head_commit(&self) -> &str {
        &self.head_commit
    }

    /// Prepare a Git `Command` pre-configured with environment isolation.
    pub fn git_command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.path);
        let null = crate::platform::filesystem::HostFilesystem::null_device();
        cmd.env("GIT_CONFIG_GLOBAL", null);
        cmd.env("GIT_CONFIG_SYSTEM", null);
        cmd.env("GIT_AUTHOR_NAME", "M31A Eval");
        cmd.env("GIT_AUTHOR_EMAIL", "eval@m31a.local");
        cmd.env("GIT_COMMITTER_NAME", "M31A Eval");
        cmd.env("GIT_COMMITTER_EMAIL", "eval@m31a.local");
        cmd
    }

    /// Run an isolated git command in the repository.
    pub fn run_git(&self, args: &[&str]) -> Result<Output, std::io::Error> {
        let mut cmd = self.git_command();
        cmd.args(args);
        cmd.output()
    }

    /// Read file content relative to repository root.
    pub fn read_file(&self, relative_path: impl AsRef<Path>) -> Result<String, std::io::Error> {
        let full_path = self.path.join(relative_path);
        fs::read_to_string(full_path)
    }

    /// Write file content relative to repository root.
    pub fn write_file(
        &self,
        relative_path: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> Result<(), std::io::Error> {
        let full_path = self.path.join(relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, content)
    }
}

/// Fluent builder for constructing isolated test Git repositories (D-15).
pub struct FixtureRepoBuilder {
    dir: TempDir,
    pending_files: Vec<(PathBuf, Vec<u8>)>,
    dirty_files: Vec<(PathBuf, Vec<u8>)>,
    commits: Vec<String>,
}

impl FixtureRepoBuilder {
    /// Create a new builder backed by a pristine temporary directory.
    pub fn new() -> Result<Self, std::io::Error> {
        let dir = tempfile::tempdir()?;
        Ok(Self {
            dir,
            pending_files: Vec::new(),
            dirty_files: Vec::new(),
            commits: Vec::new(),
        })
    }

    /// Stage a file to be added and committed during `build()`.
    pub fn with_file(
        &mut self,
        relative_path: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> &mut Self {
        self.pending_files.push((
            relative_path.as_ref().to_path_buf(),
            content.as_ref().to_vec(),
        ));
        self
    }

    /// Record a commit message for staged files.
    pub fn with_commit(&mut self, message: impl Into<String>) -> &mut Self {
        self.commits.push(message.into());
        self
    }

    /// Stage an uncommitted (dirty) file created after the last commit.
    pub fn with_dirty_file(
        &mut self,
        relative_path: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> &mut Self {
        self.dirty_files.push((
            relative_path.as_ref().to_path_buf(),
            content.as_ref().to_vec(),
        ));
        self
    }

    /// Execute repository initialization, file creation, and initial commits.
    pub fn build(self) -> Result<FixtureRepo, std::io::Error> {
        let repo_path = self.dir.path().to_path_buf();

        // Helper to run git with isolated configuration
        let run_git = |args: &[&str]| -> Result<Output, std::io::Error> {
            let mut cmd = Command::new("git");
            cmd.current_dir(&repo_path);
            let null = crate::platform::filesystem::HostFilesystem::null_device();
            cmd.env("GIT_CONFIG_GLOBAL", null);
            cmd.env("GIT_CONFIG_SYSTEM", null);
            cmd.env("GIT_AUTHOR_NAME", "M31A Eval");
            cmd.env("GIT_AUTHOR_EMAIL", "eval@m31a.local");
            cmd.env("GIT_COMMITTER_NAME", "M31A Eval");
            cmd.env("GIT_COMMITTER_EMAIL", "eval@m31a.local");
            cmd.args(args);
            cmd.output()
        };

        // 1. git init -b main
        let init_out = run_git(&["init", "-b", "main"])?;
        if !init_out.status.success() {
            // Fallback for older git versions that don't support -b
            run_git(&["init"])?;
            run_git(&["checkout", "-b", "main"])?;
        }

        // Configure local repository settings
        run_git(&["config", "user.name", "M31A Eval"])?;
        run_git(&["config", "user.email", "eval@m31a.local"])?;
        run_git(&["config", "commit.gpgsign", "false"])?;

        // 2. Write initial pending files
        if self.pending_files.is_empty() {
            // Guarantee at least one file for initial commit
            let readme_path = repo_path.join("README.md");
            fs::write(readme_path, "# M31A Fixture Repository\n")?;
        } else {
            for (rel_path, content) in &self.pending_files {
                let full_path = repo_path.join(rel_path);
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(full_path, content)?;
            }
        }

        // 3. Stage and commit
        run_git(&["add", "-A"])?;
        let commit_msg = self
            .commits
            .first()
            .cloned()
            .unwrap_or_else(|| "Initial fixture baseline".to_string());
        let commit_out = run_git(&["commit", "-m", &commit_msg])?;
        if !commit_out.status.success() {
            return Err(std::io::Error::other(format!(
                "git commit failed: {}",
                String::from_utf8_lossy(&commit_out.stderr)
            )));
        }

        // 4. Retrieve HEAD commit hash
        let rev_out = run_git(&["rev-parse", "HEAD"])?;
        let head_commit = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

        // 5. Write any dirty files that should remain uncommitted
        for (rel_path, content) in &self.dirty_files {
            let full_path = repo_path.join(rel_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(full_path, content)?;
        }

        Ok(FixtureRepo {
            _dir: self.dir,
            path: repo_path,
            head_commit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_builder_creates_isolated_git_repo() {
        let mut builder = FixtureRepoBuilder::new().unwrap();
        builder
            .with_file("src/main.rs", "fn main() { println!(\"Hello\"); }\n")
            .with_commit("feat: initial commit")
            .with_dirty_file("dirty.txt", "uncommitted text\n");

        let repo = builder.build().unwrap();

        assert!(repo.path().exists());
        assert_eq!(repo.head_commit().len(), 40);

        let content = repo.read_file("src/main.rs").unwrap();
        assert!(content.contains("println!(\"Hello\")"));

        let dirty = repo.read_file("dirty.txt").unwrap();
        assert_eq!(dirty, "uncommitted text\n");

        // Status should show dirty.txt as untracked
        let status_out = repo.run_git(&["status", "--porcelain"]).unwrap();
        let status_str = String::from_utf8_lossy(&status_out.stdout);
        assert!(status_str.contains("?? dirty.txt"));
    }
}
