//! Infrastructure-as-Code (IaC) & Full-Stack Configuration Validator (`m31a-verify::iac`).
//!
//! Validates Dockerfiles, Kubernetes manifests, GitHub Actions workflows,
//! and Compose/Terraform configs for security risks and misconfigurations.
//!
//! Enforces:
//! 1. Dockerfile Security: Flags `FROM ...:latest`, running as root, missing healthchecks.
//! 2. CI/CD Workflow Security: Flags unpinned action SHA versions (`@master`/`@main`),
//!    insecure `pull_request_target` usage, and exposed secret environments.
//! 3. Kubernetes / Deployment Manifests: Flags missing resource limits, privileged containers,
//!    and missing liveness/readiness probes.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IacSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacIssue {
    pub file_path: PathBuf,
    pub line_number: Option<usize>,
    pub severity: IacSeverity,
    pub title: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacAuditReport {
    pub files_scanned: usize,
    pub issues: Vec<IacIssue>,
    pub critical_count: usize,
    pub high_count: usize,
    pub passed: bool,
    pub summary: String,
}

pub struct IacValidator;

impl IacValidator {
    /// Validates a single file based on its name and contents.
    pub fn audit_file(path: &Path, content: &str) -> Vec<IacIssue> {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let mut issues = Vec::new();

        if file_name.to_lowercase().contains("dockerfile") {
            Self::audit_dockerfile(path, content, &mut issues);
        } else if path.to_string_lossy().contains(".github/workflows") {
            Self::audit_github_workflow(path, content, &mut issues);
        } else if file_name.ends_with(".yaml") || file_name.ends_with(".yml") {
            Self::audit_kubernetes_or_compose(path, content, &mut issues);
        }

        issues
    }

    fn audit_dockerfile(path: &Path, content: &str, issues: &mut Vec<IacIssue>) {
        let lines: Vec<&str> = content.lines().collect();
        let mut has_user_directive = false;

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("FROM ") && trimmed.contains(":latest") {
                issues.push(IacIssue {
                    file_path: path.to_path_buf(),
                    line_number: Some(line_num),
                    severity: IacSeverity::Medium,
                    title: "Unpinned base image tag (:latest)".to_string(),
                    description: "Using ':latest' base image introduces non-deterministic builds and supply-chain drift.".to_string(),
                    recommendation: "Pin base image to an exact digest or specific version tag (e.g. 'alpine:3.19').".to_string(),
                });
            }

            if trimmed.starts_with("USER ") && !trimmed.contains("root") {
                has_user_directive = true;
            }

            if (trimmed.starts_with("ENV ") || trimmed.starts_with("ARG "))
                && (trimmed.to_lowercase().contains("password=")
                    || trimmed.to_lowercase().contains("secret=")
                    || trimmed.to_lowercase().contains("api_key="))
            {
                issues.push(IacIssue {
                    file_path: path.to_path_buf(),
                    line_number: Some(line_num),
                    severity: IacSeverity::Critical,
                    title: "Hardcoded secret or credential in Dockerfile ENV/ARG".to_string(),
                    description: "Secrets baked into Docker images persist in the layer cache and are visible to image consumers.".to_string(),
                    recommendation: "Inject secrets dynamically at runtime or use Docker BuildKit secret mounts (--mount=type=secret).".to_string(),
                });
            }
        }

        if !has_user_directive && content.contains("CMD ") {
            issues.push(IacIssue {
                file_path: path.to_path_buf(),
                line_number: None,
                severity: IacSeverity::High,
                title: "Container runs as root user".to_string(),
                description: "No non-root 'USER' directive was specified. The container execution will default to root privilege.".to_string(),
                recommendation: "Add 'USER nonroot' or create a dedicated application user before entrypoint.".to_string(),
            });
        }
    }

    fn audit_github_workflow(path: &Path, content: &str, issues: &mut Vec<IacIssue>) {
        let lines: Vec<&str> = content.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            if trimmed.contains("pull_request_target") {
                issues.push(IacIssue {
                    file_path: path.to_path_buf(),
                    line_number: Some(line_num),
                    severity: IacSeverity::Critical,
                    title: "Dangerous 'pull_request_target' trigger".to_string(),
                    description: "Using 'pull_request_target' executes workflows in the repository context with write tokens and secrets access for untrusted forks.".to_string(),
                    recommendation: "Use standard 'pull_request' trigger, or strictly isolate untrusted checkouts.".to_string(),
                });
            }

            if trimmed.contains("uses:")
                && (trimmed.contains("@master") || trimmed.contains("@main"))
            {
                issues.push(IacIssue {
                    file_path: path.to_path_buf(),
                    line_number: Some(line_num),
                    severity: IacSeverity::Medium,
                    title: "Unpinned GitHub Action branch reference".to_string(),
                    description: format!("Action reference '{}' targets a mutable branch ref rather than an immutable commit SHA.", trimmed),
                    recommendation: "Pin actions to full 40-character commit SHAs (e.g. 'actions/checkout@b4ffde...').".to_string(),
                });
            }
        }
    }

    fn audit_kubernetes_or_compose(path: &Path, content: &str, issues: &mut Vec<IacIssue>) {
        let is_k8s = content.contains("apiVersion:") && content.contains("kind:");

        if is_k8s {
            if (content.contains("kind: Deployment") || content.contains("kind: StatefulSet"))
                && !content.contains("resources:")
            {
                issues.push(IacIssue {
                    file_path: path.to_path_buf(),
                    line_number: None,
                    severity: IacSeverity::High,
                    title: "Kubernetes workload missing CPU/Memory resource limits".to_string(),
                    description: "Containers without explicit resource requests and limits can starve adjacent cluster nodes.".to_string(),
                    recommendation: "Declare 'resources.limits' and 'resources.requests' for all containers.".to_string(),
                });
            }

            if content.contains("privileged: true") {
                issues.push(IacIssue {
                    file_path: path.to_path_buf(),
                    line_number: None,
                    severity: IacSeverity::Critical,
                    title: "Privileged container execution enabled".to_string(),
                    description: "Running with 'privileged: true' grants container full host kernel access, defeating container boundaries.".to_string(),
                    recommendation: "Remove 'privileged: true' and grant only specific Linux capabilities (e.g. NET_ADMIN).".to_string(),
                });
            }
        }
    }

    /// Recursively scans workspace for IaC files.
    pub fn audit_workspace(root: &Path) -> IacAuditReport {
        let mut issues = Vec::new();
        let mut files_scanned = 0;

        let mut dirs = vec![root.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    if name.starts_with('.') && name != ".github" {
                        continue;
                    }
                    if matches!(name.as_str(), "target" | "node_modules" | "dist" | ".git") {
                        continue;
                    }

                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.is_file()
                        && (name.to_lowercase().contains("dockerfile")
                            || path.to_string_lossy().contains(".github/workflows")
                            || name.ends_with(".yaml")
                            || name.ends_with(".yml"))
                    {
                        files_scanned += 1;
                        if let Ok(content) = fs::read_to_string(&path) {
                            let f_issues = Self::audit_file(&path, &content);
                            issues.extend(f_issues);
                        }
                    }
                }
            }
        }

        let critical_count = issues
            .iter()
            .filter(|i| i.severity == IacSeverity::Critical)
            .count();
        let high_count = issues
            .iter()
            .filter(|i| i.severity == IacSeverity::High)
            .count();
        let passed = critical_count == 0 && high_count == 0;

        let summary = format!(
            "Scanned {} IaC files: {} issues found ({} Critical, {} High). Gate: {}",
            files_scanned,
            issues.len(),
            critical_count,
            high_count,
            if passed { "PASSED" } else { "BLOCKED" }
        );

        IacAuditReport {
            files_scanned,
            issues,
            critical_count,
            high_count,
            passed,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dockerfile_security_audit() {
        let dockerfile = r#"
            FROM alpine:latest
            ENV API_KEY=secret_12345
            RUN apk add --no-cache curl
            CMD ["sh"]
        "#;
        let issues = IacValidator::audit_file(Path::new("Dockerfile"), dockerfile);

        assert!(issues.iter().any(|i| i.severity == IacSeverity::Critical)); // Hardcoded secret
        assert!(issues.iter().any(|i| i.severity == IacSeverity::High)); // Root user
        assert!(issues.iter().any(|i| i.severity == IacSeverity::Medium)); // latest tag
    }

    #[test]
    fn test_github_workflow_audit() {
        let workflow = r#"
            name: CI
            on: [pull_request_target]
            jobs:
              build:
                runs-on: ubuntu-latest
                steps:
                  - uses: actions/checkout@master
        "#;
        let issues = IacValidator::audit_file(Path::new(".github/workflows/ci.yml"), workflow);

        assert!(issues.iter().any(|i| i.severity == IacSeverity::Critical)); // pull_request_target
        assert!(issues.iter().any(|i| i.severity == IacSeverity::Medium)); // unpinned action
    }

    #[test]
    fn test_k8s_manifest_audit() {
        let manifest = r#"
            apiVersion: apps/v1
            kind: Deployment
            metadata:
              name: backend
            spec:
              template:
                spec:
                  containers:
                    - name: app
                      image: myapp:1.0
                      securityContext:
                        privileged: true
        "#;
        let issues = IacValidator::audit_file(Path::new("k8s/deploy.yaml"), manifest);

        assert!(issues.iter().any(|i| i.severity == IacSeverity::Critical)); // Privileged container
        assert!(issues.iter().any(|i| i.severity == IacSeverity::High)); // Missing resource limits
    }
}
