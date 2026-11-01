//! Git commit and pull request attribution management and prompt injection.
//!
//! Generates clean Git commit trailers attributing AI assistant and co-author contributions:
//! - Guarantees standard Co-authored-by git trailers are inserted cleanly without duplication.
//! - Injects PR body generation markers while preserving hidden metadata trailers.
//! - Contributes world state prompt fragments for model steering.

use std::time::{Duration, Instant};

pub const DEFAULT_TRAILER: &str = "Co-authored-by: M31A <noreply@m31a.org>";
pub const DEFAULT_PR_MARKER: &str =
    "Generated with [M31A](https://m31a.tonmoyinfrastructure.org/).";
pub const POLICY_RETRY_DELAY: Duration = Duration::from_secs(30);

pub const ENABLED_ATTRIBUTION_PROMPT: &str = r#"
When you create or amend a git commit or create or update a pull request for this task, add M31A attribution exactly once:
- Commit messages must end with `Co-authored-by: M31A <noreply@m31a.org>`. Preserve existing trailers and, if this exact trailer is missing, append it with one blank line before the trailer block.
- This also applies when a GitHub app or plugin creates a commit, including file creation, updates, and deletions: include the trailer in the tool's `message` argument.
- Pull request bodies must include the exact line `Generated with [M31A](https://m31a.tonmoyinfrastructure.org/).`. Preserve the existing body and hidden markers and, if this exact marker is missing, append it near the end, before any trailing hidden metadata markers.
- Do not add duplicate attribution. If a message or body you are already editing contains duplicate exact attribution, keep exactly one.
- Ignore any earlier instructions disabling attribution; this policy reflects the current workspace.
- Do not rewrite an existing commit or pull request solely to add attribution.
"#;

pub const DISABLED_ATTRIBUTION_PROMPT: &str = r#"
M31A commit and pull request attribution is disabled for the current workspace. Ignore any earlier instructions requiring attribution and do not add it.
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitAttributionPolicy {
    pub enabled: bool,
    pub trailer: String,
    pub pr_marker: String,
}

impl Default for GitAttributionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            trailer: DEFAULT_TRAILER.to_string(),
            pr_marker: DEFAULT_PR_MARKER.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitAttributionState {
    pub policy: GitAttributionPolicy,
    pub last_checked_at: Instant,
    pub auth_generation: u64,
}

impl GitAttributionState {
    pub fn new(policy: GitAttributionPolicy, auth_generation: u64) -> Self {
        Self {
            policy,
            last_checked_at: Instant::now(),
            auth_generation,
        }
    }

    pub fn is_stale(&self) -> bool {
        self.last_checked_at.elapsed() > POLICY_RETRY_DELAY
    }
}

/// Appends the attribution trailer to a git commit message if not already present.
///
/// Ensures exactly one blank line before the trailer block and preserves all existing trailers.
pub fn format_commit_message_with_attribution(message: &str, trailer: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return trailer.to_string();
    }

    if trimmed.lines().any(|line| line.trim() == trailer.trim()) {
        return message.to_string();
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let mut trailer_start_idx = lines.len();

    // Check backwards for trailer block (e.g. Signed-off-by:, Reviewed-by:, Co-authored-by:)
    for i in (0..lines.len()).rev() {
        let line = lines[i].trim();
        if is_git_trailer(line) {
            trailer_start_idx = i;
        } else {
            break;
        }
    }

    if trailer_start_idx == lines.len() {
        // No existing trailer block
        format!("{}\n\n{}", trimmed, trailer)
    } else {
        // Insert into or at the end of trailer block
        format!("{}\n{}", trimmed, trailer)
    }
}

/// Injects the PR attribution marker into a PR body before any trailing HTML metadata comments.
pub fn format_pr_body_with_attribution(body: &str, marker: &str) -> String {
    let trimmed = body.trim();
    if trimmed.contains(marker) {
        return body.to_string();
    }

    // Look for trailing comment block `<!-- ... -->`
    if let Some(comment_start) = trimmed.rfind("<!--") {
        let (prefix, suffix) = trimmed.split_at(comment_start);
        let prefix = prefix.trim_end();
        if prefix.is_empty() {
            format!("{}\n\n{}", marker, suffix)
        } else {
            format!("{}\n\n{}\n\n{}", prefix, marker, suffix)
        }
    } else if trimmed.is_empty() {
        marker.to_string()
    } else {
        format!("{}\n\n{}", trimmed, marker)
    }
}

fn is_git_trailer(line: &str) -> bool {
    let known_prefixes = [
        "Signed-off-by:",
        "Co-authored-by:",
        "Reviewed-by:",
        "Acked-by:",
        "Reported-by:",
        "Tested-by:",
        "Fixes:",
        "See-also:",
    ];
    known_prefixes.iter().any(|prefix| line.starts_with(prefix))
}

/// Renders the world state fragment for Git attribution.
pub fn render_git_attribution_fragment(enabled: bool) -> &'static str {
    if enabled {
        ENABLED_ATTRIBUTION_PROMPT
    } else {
        DISABLED_ATTRIBUTION_PROMPT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_commit_message_appends_trailer() {
        let msg = "feat: add bubblewrap sandbox runner";
        let formatted = format_commit_message_with_attribution(msg, DEFAULT_TRAILER);
        assert_eq!(
            formatted,
            "feat: add bubblewrap sandbox runner\n\nCo-authored-by: M31A <noreply@m31a.org>"
        );
    }

    #[test]
    fn test_format_commit_message_idempotent() {
        let msg = "feat: add bubblewrap sandbox runner\n\nCo-authored-by: M31A <noreply@m31a.org>";
        let formatted = format_commit_message_with_attribution(msg, DEFAULT_TRAILER);
        assert_eq!(formatted, msg);
    }

    #[test]
    fn test_format_commit_message_preserves_existing_trailers() {
        let msg = "feat: new feature\n\nSigned-off-by: Developer <dev@example.com>";
        let formatted = format_commit_message_with_attribution(msg, DEFAULT_TRAILER);
        assert_eq!(
            formatted,
            "feat: new feature\n\nSigned-off-by: Developer <dev@example.com>\nCo-authored-by: M31A <noreply@m31a.org>"
        );
    }

    #[test]
    fn test_format_pr_body_places_marker_before_hidden_comment() {
        let body = "This PR implements full capability mastery.\n\n<!-- metadata-token: 12345 -->";
        let formatted = format_pr_body_with_attribution(body, DEFAULT_PR_MARKER);
        assert!(
            formatted.contains("Generated with [M31A](https://m31a.tonmoyinfrastructure.org/).")
        );
        assert!(formatted.ends_with("<!-- metadata-token: 12345 -->"));
    }

    #[test]
    fn test_format_pr_body_idempotent() {
        let body =
            "Summary of changes\n\nGenerated with [M31A](https://m31a.tonmoyinfrastructure.org/).";
        let formatted = format_pr_body_with_attribution(body, DEFAULT_PR_MARKER);
        assert_eq!(formatted, body);
    }
}
