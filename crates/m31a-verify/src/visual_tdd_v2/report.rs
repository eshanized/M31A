//! Structured Visual Regression Reports.
//!
//! Provides comprehensive, machine-readable reporting and developer-friendly Markdown
//! summaries for multi-resolution visual testing runs.

use super::baseline::BaselineStatus;
use super::comparator::ComparisonOutcome;
use super::multi_res::ViewportConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Visual verification outcome for an individual viewport and display scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportReport {
    pub viewport: ViewportConfig,
    pub baseline_status: BaselineStatus,
    pub comparison: Option<ComparisonOutcome>,
    pub screenshot_path: Option<PathBuf>,
    pub heatmap_path: Option<PathBuf>,
    pub passed: bool,
    pub failure_reason: Option<String>,
}

/// Aggregated multi-resolution visual regression test report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualRegressionReport {
    pub scenario_name: String,
    pub initial_url: String,
    pub all_passed: bool,
    pub total_viewports: usize,
    pub passed_viewports: usize,
    pub failed_viewports: usize,
    pub viewport_reports: Vec<ViewportReport>,
    pub self_healing_guidance: Option<String>,
}

impl VisualRegressionReport {
    /// Formats a GitHub Flavored Markdown summary of the visual test execution.
    pub fn render_markdown_summary(&self) -> String {
        let mut md = format!(
            "## Visual Regression TDD v2 Report: `{}`\n\n",
            self.scenario_name
        );
        md.push_str(&format!("- **Target URL:** {}\n", self.initial_url));
        md.push_str(&format!(
            "- **Status:** {}\n",
            if self.all_passed {
                "✅ **PASSED**"
            } else {
                "❌ **REGRESSION DETECTED**"
            }
        ));
        md.push_str(&format!(
            "- **Viewports Verified:** {}/{} passed\n\n",
            self.passed_viewports, self.total_viewports
        ));

        md.push_str("| Viewport | Resolution | DPR | Status | Mismatch % | Heatmap |\n");
        md.push_str("|---|---|---|---|---|---|\n");

        for vr in &self.viewport_reports {
            let status_badge = if vr.passed {
                "✅ Pass"
            } else {
                "❌ Fail"
            };

            let mismatch_str = vr
                .comparison
                .as_ref()
                .map(|c| format!("{:.3}%", c.mismatch_ratio * 100.0))
                .unwrap_or_else(|| "N/A".to_string());

            let heatmap_str = vr
                .heatmap_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                .unwrap_or_else(|| "None".to_string());

            md.push_str(&format!(
                "| {} | {}x{} | {:.1}x | {} | {} | `{}` |\n",
                vr.viewport.name,
                vr.viewport.width,
                vr.viewport.height,
                vr.viewport.device_scale_factor,
                status_badge,
                mismatch_str,
                heatmap_str
            ));
        }

        if let Some(ref guidance) = self.self_healing_guidance {
            md.push_str(&format!("\n### 🩹 Layout Self-Healing Guidance\n\n{}\n", guidance));
        }

        md
    }
}
