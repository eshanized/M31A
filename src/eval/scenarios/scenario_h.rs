//! Acceptance Scenario H: Huge Compiler Output Artifact Handling (CONTEXT_M31A.md §99, TST-01, BST-01, D-08, D-15).
//!
//! Objectives:
//! 1. Set up isolated fixture repository.
//! 2. Generate large compiler/tool output simulating high-volume diagnostic dumps.
//! 3. Verify `StreamingQuotaWriter` / `QuotaEnforcer` intercepts the write in real time.
//! 4. Confirm disk and memory limits are respected without process crashes.
//! 5. Ensure protected verification evidence remains intact.

use async_trait::async_trait;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::persistence::artifacts::quota::{ArtifactExemption, QuotaEnforcer};

pub struct ScenarioH;

#[async_trait]
impl EvalScenario for ScenarioH {
    fn id(&self) -> &'static str {
        "h"
    }

    fn name(&self) -> &'static str {
        "Scenario H: Huge Output Artifact Quota Handling"
    }

    fn description(&self) -> &'static str {
        "Intercept huge compiler/tool output streams; enforce byte quota boundaries without crashes"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file("src/main.rs", "fn main() { println!(\"hello\"); }\n")
            .with_commit("feat: initial main");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. Initialize QuotaEnforcer with strict 64KB artifact limit and 128KB mission quota
        let max_single = 64 * 1024; // 64 KB
        let max_cum = 128 * 1024; // 128 KB
        let enforcer = QuotaEnforcer::new(Some(max_single), Some(max_cum));

        // 3. Attempt to stream 256KB of compiler diagnostics into standard unexempted writer
        let mut sink = Vec::new();
        let mut streaming_writer = enforcer.wrap_writer(&mut sink, ArtifactExemption::Standard);

        let chunk = vec![b'A'; 16 * 1024]; // 16 KB chunks
        let mut quota_exceeded_detected = false;

        // Write chunks until quota trips
        for _ in 0..16 {
            match streaming_writer.write_all(&chunk).await {
                Ok(_) => {}
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::FileTooLarge {
                        quota_exceeded_detected = true;
                        break;
                    }
                }
            }
        }

        if !quota_exceeded_detected {
            return Err(
                "StreamingQuotaWriter failed to enforce 64KB artifact quota limit".to_string(),
            );
        }

        // 4. Verify bytes written did not exceed limit
        let bytes_written = streaming_writer.bytes_written();
        if bytes_written > max_single {
            return Err(format!(
                "Bytes written {} exceeded max_single {}",
                bytes_written, max_single
            ));
        }

        // 5. Verify protected artifacts (e.g. completion report / verification evidence) bypass bounds
        let mut protected_sink = Vec::new();
        let mut protected_writer =
            enforcer.wrap_writer(&mut protected_sink, ArtifactExemption::Protected);

        let protected_data = vec![b'P'; 80 * 1024]; // 80 KB > 64 KB limit
        protected_writer
            .write_all(&protected_data)
            .await
            .map_err(|e| format!("Protected write failed: {}", e))?;

        if protected_writer.bytes_written() != 80 * 1024 {
            return Err("Protected artifact write length mismatch".to_string());
        }

        // 6. Verify fixture repository is undisturbed
        let status_out = fixture
            .run_git(&["status", "--porcelain"])
            .map_err(|e| e.to_string())?;
        let clean_worktree = status_out.stdout.is_empty();

        let verification_passed = quota_exceeded_detected && clean_worktree;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScenarioResult {
            scenario_id: "h".to_string(),
            name: self.name().to_string(),
            status: if verification_passed {
                ScenarioStatus::Passed
            } else {
                ScenarioStatus::Failed
            },
            duration_ms,
            tokens_used: 0,
            cost_usd: 0.0,
            verification_passed,
            replans_count: 0,
            retries_count: 0,
            files_modified: 0,
            details: format!(
                "Quota enforcer clamped standard stream at {} bytes (< {}). Protected write (80KB) succeeded.",
                bytes_written, max_single
            ),
        })
    }
}
