//! Autonomous Visual Regression TDD v2 Engine.
//!
//! Upgraded visual verification subsystem supporting multi-resolution matrix testing,
//! automated screenshot comparison with perceptual color distance (ΔE), visual difference
//! heatmaps, region-specific dynamic masking, baseline cryptographic integrity management,
//! and bounded layout repair.

pub mod baseline;
pub mod comparator;
pub mod heatmap;
pub mod masking;
pub mod multi_res;
pub mod repair;
pub mod report;

pub use baseline::{BaselineManifest, BaselineRecord, BaselineStatus, BaselineStore};
pub use comparator::{ComparisonOutcome, ComparisonThreshold, DiffBoundingBox, VisualComparator};
pub use heatmap::{HeatmapOptions, VisualHeatmap};
pub use masking::{MaskRule, MaskSet, RectRegion};
pub use multi_res::{ViewportConfig, ViewportPreset};
pub use repair::{
    LayoutRepairCoordinator, LayoutRepairPatch, LayoutRepairReport, RepairCheckpoint,
    RepairLoopConfig,
};
pub use report::{ViewportReport, VisualRegressionReport};

use crate::visual_tdd::{UiActionStep, UiAssertion};
use anyhow::Result;
use m31a_browser::{BrowserConfig, BrowserSession};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Complete specification for a Visual TDD v2 scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualTddV2Scenario {
    pub name: String,
    pub description: Option<String>,
    pub initial_url: String,
    #[serde(default = "ViewportPreset::standard_matrix")]
    pub viewports: Vec<ViewportConfig>,
    #[serde(default)]
    pub masks: MaskSet,
    #[serde(default)]
    pub threshold: ComparisonThreshold,
    #[serde(default)]
    pub actions: Vec<UiActionStep>,
    #[serde(default)]
    pub assertions: Vec<UiAssertion>,
    #[serde(default)]
    pub auto_repair: bool,
    #[serde(default)]
    pub repair_config: Option<RepairLoopConfig>,
}

impl VisualTddV2Scenario {
    pub fn new(name: impl Into<String>, initial_url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            initial_url: initial_url.into(),
            viewports: ViewportPreset::standard_matrix(),
            masks: MaskSet::new(),
            threshold: ComparisonThreshold::default(),
            actions: Vec::new(),
            assertions: Vec::new(),
            auto_repair: false,
            repair_config: None,
        }
    }
}

pub struct VisualTddV2Runner;

impl VisualTddV2Runner {
    /// Executes a Visual TDD v2 scenario across all configured viewports, comparing
    /// captured screenshots with baseline golden snapshots.
    pub async fn run_scenario(
        scenario: &VisualTddV2Scenario,
        session_opt: Option<Arc<BrowserSession>>,
        baseline_store: &BaselineStore,
        output_dir: &Path,
    ) -> Result<VisualRegressionReport> {
        info!(
            name = %scenario.name,
            viewports = scenario.viewports.len(),
            url = %scenario.initial_url,
            "Executing Visual TDD v2 multi-resolution test run"
        );

        tokio::fs::create_dir_all(output_dir).await?;

        let session = match session_opt {
            Some(s) => s,
            None => {
                let default_vp = scenario
                    .viewports
                    .first()
                    .cloned()
                    .unwrap_or_else(|| ViewportPreset::DesktopStandard.to_config());
                let config = BrowserConfig {
                    headless: true,
                    viewport_width: default_vp.width,
                    viewport_height: default_vp.height,
                    ..Default::default()
                };
                Arc::new(BrowserSession::launch(config).await?)
            }
        };

        let mut viewport_reports = Vec::new();
        let mut all_passed = true;
        let mut aggregated_repair_prompts = Vec::new();

        for vp in &scenario.viewports {
            debug!(viewport = %vp.id(), "Testing viewport");

            // 1. Override viewport & DPR in browser
            let _ = session
                .set_viewport(vp.width, vp.height, Some(vp.device_scale_factor))
                .await;

            // 2. Initial navigation
            session.navigate(&scenario.initial_url).await?;
            tokio::time::sleep(Duration::from_millis(250)).await;

            // 3. Replay interaction actions
            for action in &scenario.actions {
                match action {
                    UiActionStep::Navigate { url } => {
                        session.navigate(url).await?;
                        tokio::time::sleep(Duration::from_millis(150)).await;
                    }
                    UiActionStep::Click { selector } => {
                        let script = format!(
                            "(() => {{ const el = document.querySelector({}); if (el) el.click(); }})()",
                            serde_json::to_string(selector)?
                        );
                        let _ = session.evaluate(&script).await;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    UiActionStep::Type { selector, text } => {
                        let script = format!(
                            "(() => {{ const el = document.querySelector({}); if (el) {{ el.focus(); el.value = {}; el.dispatchEvent(new Event('input', {{ bubbles: true }})); }} }})()",
                            serde_json::to_string(selector)?,
                            serde_json::to_string(text)?
                        );
                        let _ = session.evaluate(&script).await;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    UiActionStep::WaitSelector {
                        selector,
                        timeout_ms,
                    } => {
                        let timeout = timeout_ms.unwrap_or(2000);
                        let start = std::time::Instant::now();
                        while start.elapsed().as_millis() < timeout as u128 {
                            let check = format!(
                                "document.querySelector({}) !== null",
                                serde_json::to_string(selector)?
                            );
                            if let Ok(res) = session.evaluate(&check).await
                                && res.trim() == "true"
                            {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                    UiActionStep::WaitMillis { ms } => {
                        tokio::time::sleep(Duration::from_millis(*ms)).await;
                    }
                    UiActionStep::Screenshot { .. } => {}
                }
            }

            // 4. Capture screenshot
            let vp_id = vp.id();
            let shot_path =
                output_dir.join(format!("{}_{}.png", scenario.name.replace(' ', "_"), vp_id));
            let shot_res = session.screenshot(Some(&shot_path)).await;

            let (shot_bytes, screenshot_saved_path) = match shot_res {
                Ok(_) if shot_path.exists() => {
                    let bytes = std::fs::read(&shot_path)?;
                    (Some(bytes), Some(shot_path.clone()))
                }
                _ => (None, None),
            };

            // 5. Baseline Evaluation & Image Comparison
            let baseline_status = baseline_store.check_baseline_status(&scenario.name, vp)?;

            let (vp_passed, failure_reason, comparison_outcome, heatmap_saved_path) =
                match baseline_status {
                    BaselineStatus::Missing => {
                        all_passed = false;
                        (
                            false,
                            Some(format!(
                                "Baseline snapshot missing for viewport '{}'. Review and approve captured screenshot to establish golden baseline.",
                                vp_id
                            )),
                            None,
                            None,
                        )
                    }
                    BaselineStatus::IntegrityMismatch {
                        ref expected_sha256,
                        ref actual_sha256,
                    } => {
                        all_passed = false;
                        (
                            false,
                            Some(format!(
                                "Baseline integrity check failed for viewport '{}': manifest expected SHA-256 {} but file has {}",
                                vp_id, expected_sha256, actual_sha256
                            )),
                            None,
                            None,
                        )
                    }
                    BaselineStatus::FileNotFound { ref path } => {
                        all_passed = false;
                        (
                            false,
                            Some(format!(
                                "Baseline file registered in manifest not found at {:?}",
                                path
                            )),
                            None,
                            None,
                        )
                    }
                    BaselineStatus::Valid => {
                        if let Some(ref current_bytes) = shot_bytes {
                            let golden_bytes = baseline_store
                                .get_baseline_image(&scenario.name, vp)?
                                .expect("Valid baseline must have image bytes");

                            match VisualComparator::compare_bytes(
                                &golden_bytes,
                                current_bytes,
                                &scenario.masks,
                                &scenario.threshold,
                            ) {
                                Ok((outcome, diff_img)) => {
                                    let mut heat_path_saved = None;
                                    let fail_reason = outcome.error_details.clone();
                                    if !outcome.passed {
                                        all_passed = false;

                                        // Generate and save difference heatmap
                                        let heatmap = VisualHeatmap::annotate_heatmap(
                                            diff_img,
                                            &outcome,
                                            &HeatmapOptions::default(),
                                        );
                                        let heat_path = output_dir.join(format!(
                                            "{}_{}_heatmap.png",
                                            scenario.name.replace(' ', "_"),
                                            vp_id
                                        ));
                                        if VisualHeatmap::save_heatmap(&heatmap, &heat_path).is_ok()
                                        {
                                            heat_path_saved = Some(heat_path);
                                        }

                                        // Collect self-healing guidance
                                        let prompt =
                                            LayoutRepairCoordinator::generate_repair_prompt(
                                                &scenario.name,
                                                &vp_id,
                                                &outcome,
                                                None,
                                            );
                                        aggregated_repair_prompts.push(prompt);
                                    }
                                    (outcome.passed, fail_reason, Some(outcome), heat_path_saved)
                                }
                                Err(e) => {
                                    all_passed = false;
                                    (
                                        false,
                                        Some(format!("Image comparison failed: {e}")),
                                        None,
                                        None,
                                    )
                                }
                            }
                        } else {
                            all_passed = false;
                            (
                                false,
                                Some(
                                    "Screenshot capture failed or browser unavailable".to_string(),
                                ),
                                None,
                                None,
                            )
                        }
                    }
                };

            viewport_reports.push(ViewportReport {
                viewport: vp.clone(),
                baseline_status,
                comparison: comparison_outcome,
                screenshot_path: screenshot_saved_path,
                heatmap_path: heatmap_saved_path,
                passed: vp_passed,
                failure_reason,
            });
        }

        let total_viewports = viewport_reports.len();
        let passed_viewports = viewport_reports.iter().filter(|r| r.passed).count();
        let failed_viewports = total_viewports.saturating_sub(passed_viewports);

        let self_healing_guidance = if !aggregated_repair_prompts.is_empty() {
            Some(aggregated_repair_prompts.join("\n\n---\n\n"))
        } else {
            None
        };

        Ok(VisualRegressionReport {
            scenario_name: scenario.name.clone(),
            initial_url: scenario.initial_url.clone(),
            all_passed,
            total_viewports,
            passed_viewports,
            failed_viewports,
            viewport_reports,
            self_healing_guidance,
        })
    }
}
