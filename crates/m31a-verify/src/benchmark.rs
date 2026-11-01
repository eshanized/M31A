//! Performance Engineering, Profiling, and Benchmark Regression Prevention (`m31a-verify::benchmark`).
//!
//! Profiles execution bottlenecks, benchmarks before/after changes, and enforces
//! automated regression gates to prevent latency regressions.
//!
//! Features:
//! 1. `PerformanceProfiler`: Detects workspace benchmarks (Cargo Criterion, Pytest Benchmark, Go Benchmark).
//! 2. Baseline vs. Candidate comparison: Computes latency delta percentage, throughput variance,
//!    and regression classifications.
//! 3. Performance Gate: Rejects changes introducing performance regressions exceeding a configurable threshold.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceVerdict {
    Improved,
    Neutral,
    RegressionDetected,
    BenchmarkFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetric {
    pub name: String,
    pub baseline_ms: f64,
    pub candidate_ms: f64,
    pub delta_pct: f64,
    pub is_regression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub total_benchmarks: usize,
    pub metrics: Vec<BenchmarkMetric>,
    pub average_delta_pct: f64,
    pub regression_threshold_pct: f64,
    pub verdict: PerformanceVerdict,
    pub summary: String,
}

pub struct PerformanceProfiler;

impl PerformanceProfiler {
    /// Detects benchmark runner based on workspace files.
    pub fn detect_benchmark_command(root: &Path) -> Option<String> {
        if root.join("benches").is_dir() || root.join("Cargo.toml").is_file() {
            Some("cargo bench --no-run".to_string())
        } else if root.join("pytest.ini").is_file() || root.join("requirements.txt").is_file() {
            Some("pytest -m benchmark --benchmark-only".to_string())
        } else if root.join("go.mod").is_file() {
            Some("go test -bench=.".to_string())
        } else {
            None
        }
    }

    /// Evaluates benchmark metrics between baseline and candidate runs.
    pub fn compare_runs(
        metrics: Vec<(String, f64, f64)>, // (name, baseline_ms, candidate_ms)
        regression_threshold_pct: f64,
    ) -> BenchmarkReport {
        let mut results = Vec::new();
        let mut total_delta = 0.0;
        let mut regressions_count = 0;

        for (name, baseline, candidate) in metrics {
            let delta = if baseline > 0.0 {
                ((candidate - baseline) / baseline) * 100.0
            } else {
                0.0
            };
            total_delta += delta;

            let is_regression = delta > regression_threshold_pct;
            if is_regression {
                regressions_count += 1;
            }

            results.push(BenchmarkMetric {
                name,
                baseline_ms: baseline,
                candidate_ms: candidate,
                delta_pct: delta,
                is_regression,
            });
        }

        let count = results.len();
        let avg_delta = if count > 0 {
            total_delta / count as f64
        } else {
            0.0
        };

        let verdict = if regressions_count > 0 {
            PerformanceVerdict::RegressionDetected
        } else if avg_delta < -5.0 {
            PerformanceVerdict::Improved
        } else {
            PerformanceVerdict::Neutral
        };

        let summary = format!(
            "Evaluated {} benchmarks: avg delta {:.2}%, {} regressions exceeding {:.1}% threshold (Verdict: {:?})",
            count, avg_delta, regressions_count, regression_threshold_pct, verdict
        );

        BenchmarkReport {
            total_benchmarks: count,
            metrics: results,
            average_delta_pct: avg_delta,
            regression_threshold_pct,
            verdict,
            summary,
        }
    }

    /// Measures the execution duration of an arbitrary command (e.g. CLI or integration test)
    pub fn measure_command_latency(cwd: &Path, command: &str, iterations: usize) -> Result<f64> {
        let parts =
            shlex::split(command).unwrap_or_else(|| vec!["cargo".to_string(), "check".to_string()]);
        let mut durations = Vec::new();

        for _ in 0..iterations.max(1) {
            let start = Instant::now();
            let mut cmd = Command::new(&parts[0]);
            if parts.len() > 1 {
                cmd.args(&parts[1..]);
            }
            cmd.current_dir(cwd);
            let _ = cmd.output()?;
            durations.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Return median
        let mid = durations.len() / 2;
        Ok(durations[mid])
    }
}

/// A sample of an execution call stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackSample {
    pub frames: Vec<String>,
    pub count: usize,
}

/// Summary report of sampling profiler data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileReport {
    pub duration_ms: f64,
    pub total_samples: usize,
    pub samples: Vec<StackSample>,
    pub top_bottlenecks: Vec<(String, usize, f64)>,
}

/// Sampling profiler that aggregates folded stack samples and identifies hotspots.
pub struct SamplingProfiler;

impl SamplingProfiler {
    /// Ingests raw folded stack traces (e.g. from perf, pprof, cargo-flamegraph)
    /// and generates a structured profile report with hotspot identification.
    pub fn parse_folded_stacks(folded_lines: &str) -> ProfileReport {
        let mut samples = Vec::new();
        let mut total_samples = 0;
        let mut frame_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for line in folded_lines.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Folded format: "funcA;funcB;funcC 42"
            if let Some((stack_part, count_part)) = trimmed.rsplit_once(' ')
                && let Ok(count) = count_part.parse::<usize>()
            {
                total_samples += count;
                let frames: Vec<String> = stack_part
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                for frame in &frames {
                    *frame_counts.entry(frame.clone()).or_insert(0) += count;
                }

                samples.push(StackSample { frames, count });
            }
        }

        let mut bottlenecks: Vec<(String, usize, f64)> = frame_counts
            .into_iter()
            .map(|(frame, count)| {
                let pct = if total_samples > 0 {
                    (count as f64 / total_samples as f64) * 100.0
                } else {
                    0.0
                };
                (frame, count, pct)
            })
            .collect();

        bottlenecks.sort_by_key(|a| std::cmp::Reverse(a.1));
        bottlenecks.truncate(10);

        ProfileReport {
            duration_ms: total_samples as f64 * 10.0,
            total_samples,
            samples,
            top_bottlenecks: bottlenecks,
        }
    }
}

/// Generator producing interactive SVG flamegraphs from sampling profile data.
pub struct FlamegraphGenerator;

impl FlamegraphGenerator {
    /// Generates a valid interactive SVG flamegraph visualization from folded stack sample data.
    pub fn generate_flamegraph_svg(report: &ProfileReport, title: &str) -> String {
        let width = 1200;
        let row_height = 24;
        let max_depth = report
            .samples
            .iter()
            .map(|s| s.frames.len())
            .max()
            .unwrap_or(1);
        let height = (max_depth + 4) * row_height;

        let mut svg = format!(
            r##"<svg version="1.1" width="{width}" height="{height}" viewBox="0 0 {width} {height}" xmlns="http://www.w3.org/2000/svg">
<style>
    text {{ font-family: monospace; font-size: 12px; fill: #ffffff; }}
    .title {{ font-size: 18px; font-weight: bold; fill: #333333; }}
    rect {{ stroke: #ffffff; stroke-width: 0.5; }}
    rect:hover {{ opacity: 0.8; cursor: pointer; }}
</style>
<rect width="{width}" height="{height}" fill="#f8f9fa"/>
<text x="20" y="30" class="title">{title} - {total} samples ({duration:.1}ms)</text>
"##,
            width = width,
            height = height,
            title = title,
            total = report.total_samples,
            duration = report.duration_ms
        );

        if report.total_samples == 0 {
            svg.push_str("</svg>");
            return svg;
        }

        let colors = [
            "#d9534f", "#f0ad4e", "#f39c12", "#e67e22", "#d35400", "#e74c3c",
        ];
        let mut current_x = 20.0;
        let usable_width = (width - 40) as f64;

        for (sample_idx, sample) in report.samples.iter().enumerate() {
            let sample_width = (sample.count as f64 / report.total_samples as f64) * usable_width;
            if sample_width < 0.5 {
                current_x += sample_width;
                continue;
            }

            for (depth, frame) in sample.frames.iter().enumerate() {
                let y = height - (depth + 2) * row_height;
                let color_idx = (sample_idx + depth) % colors.len();
                let color = colors[color_idx];

                svg.push_str(&format!(
                    r#"<rect x="{:.1}" y="{}" width="{:.1}" height="{}" fill="{}"><title>{} ({} samples)</title></rect>
"#,
                    current_x, y, sample_width, row_height - 2, color, frame, sample.count
                ));

                if sample_width > 40.0 {
                    let max_chars = ((sample_width / 8.0) as usize).min(frame.len());
                    let display_text = &frame[..max_chars];
                    svg.push_str(&format!(
                        r#"<text x="{:.1}" y="{}">{}</text>
"#,
                        current_x + 3.0,
                        y + 16,
                        display_text
                    ));
                }
            }
            current_x += sample_width;
        }

        svg.push_str("</svg>");
        svg
    }
}

/// Evaluates high-level autonomous engineering benchmark metrics across multiple benchmark scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvaluationInput {
    pub task_id: String,
    pub declared_complete: bool,
    pub timeline_verified: bool,
    pub regression_detected: bool,
    pub recovered_from_error: bool,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousEngineeringBenchmarkReport {
    pub total_tasks: usize,
    pub verified_completions: usize,
    pub false_completions: usize,
    pub false_completion_rate: f64,
    pub regression_detection_rate: f64,
    pub recovery_success_rate: f64,
    pub average_duration_ms: f64,
}

pub struct AutonomousEngineeringBenchmark;

impl AutonomousEngineeringBenchmark {
    pub fn evaluate(tasks: &[TaskEvaluationInput]) -> AutonomousEngineeringBenchmarkReport {
        let total_tasks = tasks.len();
        if total_tasks == 0 {
            return AutonomousEngineeringBenchmarkReport {
                total_tasks: 0,
                verified_completions: 0,
                false_completions: 0,
                false_completion_rate: 0.0,
                regression_detection_rate: 1.0,
                recovery_success_rate: 1.0,
                average_duration_ms: 0.0,
            };
        }

        let mut verified_count = 0;
        let mut false_completion_count = 0;
        let mut regression_detected_count = 0;
        let mut recovery_count = 0;
        let mut total_duration = 0.0;

        for t in tasks {
            total_duration += t.duration_ms;
            if t.declared_complete && t.timeline_verified {
                verified_count += 1;
            } else if t.declared_complete && !t.timeline_verified {
                false_completion_count += 1;
            }

            if t.regression_detected {
                regression_detected_count += 1;
            }
            if t.recovered_from_error {
                recovery_count += 1;
            }
        }

        let false_completion_rate = if total_tasks > 0 {
            false_completion_count as f64 / total_tasks as f64
        } else {
            0.0
        };

        AutonomousEngineeringBenchmarkReport {
            total_tasks,
            verified_completions: verified_count,
            false_completions: false_completion_count,
            false_completion_rate,
            regression_detection_rate: regression_detected_count as f64 / total_tasks as f64,
            recovery_success_rate: recovery_count as f64 / total_tasks as f64,
            average_duration_ms: total_duration / total_tasks as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_comparison_detects_regression() {
        let metrics = vec![
            ("json_parse".to_string(), 10.0, 15.0), // +50% regression
            ("ast_chunk".to_string(), 20.0, 20.5),  // +2.5% neutral
        ];
        let report = PerformanceProfiler::compare_runs(metrics, 10.0);

        assert_eq!(report.verdict, PerformanceVerdict::RegressionDetected);
        assert!(report.metrics[0].is_regression);
        assert!(!report.metrics[1].is_regression);
    }

    #[test]
    fn test_benchmark_comparison_detects_improvement() {
        let metrics = vec![
            ("token_search".to_string(), 100.0, 80.0), // -20% improvement
        ];
        let report = PerformanceProfiler::compare_runs(metrics, 10.0);

        assert_eq!(report.verdict, PerformanceVerdict::Improved);
        assert!(report.average_delta_pct < 0.0);
    }

    #[test]
    fn test_detect_cargo_benchmark() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("benches")).unwrap();
        let cmd = PerformanceProfiler::detect_benchmark_command(temp.path());
        assert!(cmd.is_some());
        assert!(cmd.unwrap().contains("cargo bench"));
    }

    #[test]
    fn test_sampling_profiler_and_flamegraph() {
        let folded = "main;agent_loop;execute_turn 60\nmain;agent_loop;format_prompt 40\n";
        let report = SamplingProfiler::parse_folded_stacks(folded);
        assert_eq!(report.total_samples, 100);
        assert_eq!(report.samples.len(), 2);
        assert!(!report.top_bottlenecks.is_empty());

        let svg = FlamegraphGenerator::generate_flamegraph_svg(&report, "Turn Profiler");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Turn Profiler"));
        assert!(svg.contains("execute_turn"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_autonomous_engineering_benchmark_evaluation() {
        let tasks = vec![
            TaskEvaluationInput {
                task_id: "task-1".to_string(),
                declared_complete: true,
                timeline_verified: true,
                regression_detected: false,
                recovered_from_error: true,
                duration_ms: 1200.0,
            },
            TaskEvaluationInput {
                task_id: "task-2".to_string(),
                declared_complete: true,
                timeline_verified: true,
                regression_detected: true,
                recovered_from_error: true,
                duration_ms: 800.0,
            },
            TaskEvaluationInput {
                task_id: "task-3".to_string(),
                declared_complete: false,
                timeline_verified: false,
                regression_detected: true,
                recovered_from_error: false,
                duration_ms: 1000.0,
            },
        ];

        let rep = AutonomousEngineeringBenchmark::evaluate(&tasks);
        assert_eq!(rep.total_tasks, 3);
        assert_eq!(rep.verified_completions, 2);
        assert_eq!(rep.false_completions, 0);
        assert_eq!(rep.false_completion_rate, 0.0);
        assert_eq!(rep.average_duration_ms, 1000.0);
    }
}
