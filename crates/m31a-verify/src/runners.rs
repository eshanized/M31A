//! Project detection and test/compiler runner.

use anyhow::Result;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Rust,
    Bazel,
    NodeTypeScript,
    Python,
    Go,
    CMake,
    Gradle,
    Generic,
}

#[derive(Debug, Clone)]
pub struct RawRunResult {
    pub project_kind: ProjectKind,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
    pub timed_out: bool,
}

pub struct ProjectRunner;

impl ProjectRunner {
    pub fn detect(cwd: &Path) -> ProjectKind {
        if cwd.join("Cargo.toml").exists() {
            ProjectKind::Rust
        } else if cwd.join("BUILD.bazel").exists()
            || cwd.join("MODULE.bazel").exists()
            || cwd.join("WORKSPACE").exists()
        {
            ProjectKind::Bazel
        } else if cwd.join("package.json").exists() || cwd.join("tsconfig.json").exists() {
            ProjectKind::NodeTypeScript
        } else if cwd.join("pyproject.toml").exists()
            || cwd.join("requirements.txt").exists()
            || cwd.join("pytest.ini").exists()
        {
            ProjectKind::Python
        } else if cwd.join("go.mod").exists() {
            ProjectKind::Go
        } else if cwd.join("CMakeLists.txt").exists() {
            ProjectKind::CMake
        } else if cwd.join("build.gradle").exists() || cwd.join("build.gradle.kts").exists() {
            ProjectKind::Gradle
        } else {
            ProjectKind::Generic
        }
    }

    /// Runs compiler / fast check command for the detected project.
    pub async fn run_check(cwd: &Path, timeout: Duration) -> Result<RawRunResult> {
        let kind = Self::detect(cwd);
        let cmd = match kind {
            ProjectKind::Rust => "cargo check --message-format=json",
            ProjectKind::Bazel => "bazel build //...",
            ProjectKind::NodeTypeScript => {
                if cwd.join("tsconfig.json").exists() {
                    "npx tsc --noEmit"
                } else {
                    "npm test -- --watchAll=false"
                }
            }
            ProjectKind::Python => "pytest --maxfail=3",
            ProjectKind::Go => "go test ./...",
            ProjectKind::CMake => "cmake -B build && cmake --build build",
            ProjectKind::Gradle => "./gradlew check --dry-run",
            ProjectKind::Generic => "echo 'No recognized build tool in workspace'",
        };

        Self::execute_cmd(cmd, cwd, kind, timeout).await
    }

    /// Runs full test suite for the detected project.
    pub async fn run_tests(cwd: &Path, timeout: Duration) -> Result<RawRunResult> {
        let kind = Self::detect(cwd);
        let cmd = match kind {
            ProjectKind::Rust => "cargo test",
            ProjectKind::Bazel => "bazel test //...",
            ProjectKind::NodeTypeScript => "npm test",
            ProjectKind::Python => "pytest",
            ProjectKind::Go => "go test -v ./...",
            ProjectKind::CMake => "ctest --test-dir build --output-on-failure",
            ProjectKind::Gradle => "./gradlew test",
            ProjectKind::Generic => "echo 'No test suite defined'",
        };

        Self::execute_cmd(cmd, cwd, kind, timeout).await
    }

    /// Runs compiler / fast check command for the detected project with optional sandbox isolation.
    pub async fn run_check_sandboxed(
        cwd: &Path,
        timeout: Duration,
        policy: Option<&m31a_core::SandboxPolicy>,
    ) -> Result<RawRunResult> {
        let kind = Self::detect(cwd);
        let cmd = match kind {
            ProjectKind::Rust => "cargo check --message-format=json",
            ProjectKind::Bazel => "bazel build //...",
            ProjectKind::NodeTypeScript => {
                if cwd.join("tsconfig.json").exists() {
                    "npx tsc --noEmit"
                } else {
                    "npm test -- --watchAll=false"
                }
            }
            ProjectKind::Python => "pytest --maxfail=3",
            ProjectKind::Go => "go test ./...",
            ProjectKind::CMake => "cmake -B build && cmake --build build",
            ProjectKind::Gradle => "./gradlew check --dry-run",
            ProjectKind::Generic => "echo 'No recognized build tool in workspace'",
        };

        Self::execute_cmd_sandboxed(cmd, cwd, kind, timeout, policy).await
    }

    /// Runs full test suite for the detected project with optional sandbox isolation.
    pub async fn run_tests_sandboxed(
        cwd: &Path,
        timeout: Duration,
        policy: Option<&m31a_core::SandboxPolicy>,
    ) -> Result<RawRunResult> {
        let kind = Self::detect(cwd);
        let cmd = match kind {
            ProjectKind::Rust => "cargo test",
            ProjectKind::Bazel => "bazel test //...",
            ProjectKind::NodeTypeScript => "npm test",
            ProjectKind::Python => "pytest",
            ProjectKind::Go => "go test -v ./...",
            ProjectKind::CMake => "ctest --test-dir build --output-on-failure",
            ProjectKind::Gradle => "./gradlew test",
            ProjectKind::Generic => "echo 'No test suite defined'",
        };

        Self::execute_cmd_sandboxed(cmd, cwd, kind, timeout, policy).await
    }

    pub async fn execute_cmd(
        cmd: &str,
        cwd: &Path,
        kind: ProjectKind,
        timeout: Duration,
    ) -> Result<RawRunResult> {
        Self::execute_cmd_sandboxed(cmd, cwd, kind, timeout, None).await
    }

    pub async fn execute_cmd_sandboxed(
        cmd: &str,
        cwd: &Path,
        kind: ProjectKind,
        timeout: Duration,
        policy: Option<&m31a_core::SandboxPolicy>,
    ) -> Result<RawRunResult> {
        let runner = m31a_sandbox::runner::SandboxRunner::new();
        let pol = policy
            .cloned()
            .unwrap_or(m31a_core::SandboxPolicy::WorkspaceWrite);
        let out = runner.execute(cmd, cwd, pol, timeout).await?;

        let success = out.exit_code == 0 && !out.timed_out;
        Ok(RawRunResult {
            project_kind: kind,
            command: cmd.to_string(),
            stdout: out.stdout,
            stderr: out.stderr,
            exit_code: out.exit_code,
            success,
            timed_out: out.timed_out,
        })
    }
}
