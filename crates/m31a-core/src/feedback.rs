//! Self-contained user feedback report generation and issue packaging.
//!
//! Gathers diagnostic snapshots, system information, and sanitized error logs for support bundles.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use crate::sanitizer::redact_secrets;

/// Single attached artifact inside a diagnostic bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackAttachment {
    pub name: String,
    pub mime_type: String,
    pub content: String,
}

/// Limits for exported diagnostic bundles to prevent unbounded disk usage.
#[derive(Debug, Clone)]
pub struct FeedbackLimits {
    pub max_bytes: usize,
    pub max_attachments: usize,
}

impl Default for FeedbackLimits {
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024 * 1024, // 50 MB
            max_attachments: 64,
        }
    }
}

/// Self-contained feedback report envelope packaging system state, error logs, and environment metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackReport {
    pub bundle_id: String,
    pub timestamp_micros: u64,
    pub user_note: Option<String>,
    pub m31a_version: String,
    pub system_info: HashMap<String, String>,
    pub attachments: Vec<FeedbackAttachment>,
}

/// Diagnostic Feedback Bundle Generator.
pub struct FeedbackBundleManager;

impl FeedbackBundleManager {
    /// Collects system diagnostics, sanitizes attached content, and writes an atomic JSON bundle to disk.
    pub fn create_bundle(
        target_dir: &Path,
        user_note: Option<&str>,
        attachments: Vec<FeedbackAttachment>,
        limits: Option<FeedbackLimits>,
    ) -> Result<PathBuf> {
        let limits = limits.unwrap_or_default();

        if attachments.len() > limits.max_attachments {
            return Err(anyhow!(
                "Attachment count ({}) exceeds limit ({})",
                attachments.len(),
                limits.max_attachments
            ));
        }

        fs::create_dir_all(target_dir).with_context(|| {
            format!(
                "Failed to create feedback directory at {}",
                target_dir.display()
            )
        })?;

        let now_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let bundle_id = format!(
            "bundle-{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );

        // Gather host system environment
        let mut sys_info = HashMap::new();
        sys_info.insert("os".to_string(), std::env::consts::OS.to_string());
        sys_info.insert("arch".to_string(), std::env::consts::ARCH.to_string());
        sys_info.insert("num_cpus".to_string(), num_cpus_estimate().to_string());
        if let Ok(user) = std::env::var("USER") {
            sys_info.insert("user".to_string(), user);
        }

        // Sanitize all attachments before packaging
        let mut total_bytes = 0;
        let mut sanitized_attachments = Vec::with_capacity(attachments.len());

        for att in attachments {
            let redacted_content = redact_secrets(&att.content);
            total_bytes += redacted_content.len();

            if total_bytes > limits.max_bytes {
                return Err(anyhow!(
                    "Feedback bundle size exceeds {} MB limit",
                    limits.max_bytes / (1024 * 1024)
                ));
            }

            sanitized_attachments.push(FeedbackAttachment {
                name: att.name,
                mime_type: att.mime_type,
                content: redacted_content,
            });
        }

        let report = FeedbackReport {
            bundle_id: bundle_id.clone(),
            timestamp_micros: now_micros,
            user_note: user_note.map(str::to_string),
            m31a_version: "0.1.13".to_string(),
            system_info: sys_info,
            attachments: sanitized_attachments,
        };

        let file_path = target_dir.join(format!("m31a-feedback-{}.bundle.json", bundle_id));
        let serialized = serde_json::to_string_pretty(&report)?;

        let mut file = File::create(&file_path).with_context(|| {
            format!(
                "Failed to create feedback bundle file at {}",
                file_path.display()
            )
        })?;

        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;

        debug!("Created feedback bundle at {}", file_path.display());
        Ok(file_path)
    }
}

fn num_cpus_estimate() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_bundle_creation_and_redaction() {
        let dir = tempfile::tempdir().unwrap();
        let attachments = vec![
            FeedbackAttachment {
                name: "transcript.txt".to_string(),
                mime_type: "text/plain".to_string(),
                content: "User requested: sk-proj-abcdefghijklmnopqrstuvwxyz1234. Please review."
                    .to_string(),
            },
            FeedbackAttachment {
                name: "diagnostics.json".to_string(),
                mime_type: "application/json".to_string(),
                content: r#"{"status": "ok"}"#.to_string(),
            },
        ];

        let bundle_path = FeedbackBundleManager::create_bundle(
            dir.path(),
            Some("Subagent encountered a deadlock"),
            attachments,
            None,
        )
        .unwrap();

        assert!(bundle_path.exists());
        let content = fs::read_to_string(&bundle_path).unwrap();
        let parsed: FeedbackReport = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.attachments.len(), 2);
        assert_eq!(
            parsed.user_note.as_deref(),
            Some("Subagent encountered a deadlock")
        );
        assert!(parsed.system_info.contains_key("os"));

        // Verify secret redaction was applied inside bundle
        let transcript = &parsed.attachments[0].content;
        assert!(!transcript.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234"));
        assert!(transcript.contains("[REDACTED_SECRET]"));
    }
}
