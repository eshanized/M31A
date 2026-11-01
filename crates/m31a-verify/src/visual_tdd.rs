//! Autonomous Visual TDD and UI Verification Engine (`m31a-verify::visual_tdd`).
//!
//! Provides spec-driven DOM inspection, computed style validation, visual regression auditing,
//! user action replay, and targeted self-healing diagnostics for web frontends.

use anyhow::Result;
use m31a_browser::{BrowserConfig, BrowserSession};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// User interaction step within a Visual TDD scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiActionStep {
    Navigate {
        url: String,
    },
    Click {
        selector: String,
    },
    Type {
        selector: String,
        text: String,
    },
    WaitSelector {
        selector: String,
        timeout_ms: Option<u64>,
    },
    WaitMillis {
        ms: u64,
    },
    Screenshot {
        name: String,
    },
}

/// An assertion to evaluate against the live browser DOM and state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAssertion {
    ElementPresent {
        selector: String,
        should_be_visible: bool,
    },
    TextMatches {
        selector: String,
        expected: String,
        exact: bool,
    },
    ComputedStyleMatches {
        selector: String,
        property: String,
        expected: String,
    },
    AttributeMatches {
        selector: String,
        attribute: String,
        expected: String,
    },
    ZeroConsoleErrors,
    ZeroNetworkFailures,
    CustomScript {
        description: String,
        script: String,
    },
}

/// Specification for a visual TDD test scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualTddScenario {
    pub name: String,
    pub description: Option<String>,
    pub initial_url: String,
    #[serde(default)]
    pub viewport: Option<(u32, u32)>,
    #[serde(default)]
    pub actions: Vec<UiActionStep>,
    pub assertions: Vec<UiAssertion>,
}

/// The result of an individual assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionOutcome {
    pub description: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
    pub error_details: Option<String>,
}

/// Complete report of a Visual TDD test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualTddReport {
    pub scenario_name: String,
    pub initial_url: String,
    pub passed: bool,
    pub total_assertions: usize,
    pub passed_assertions: usize,
    pub failed_assertions: usize,
    pub assertion_outcomes: Vec<AssertionOutcome>,
    pub console_errors: Vec<String>,
    pub network_errors: Vec<String>,
    pub captured_screenshots: Vec<PathBuf>,
    pub self_healing_guidance: Option<String>,
}

pub struct VisualTddRunner;

impl VisualTddRunner {
    /// Executes a Visual TDD scenario using an active or newly launched browser session.
    pub async fn run_scenario(
        scenario: &VisualTddScenario,
        session_opt: Option<Arc<BrowserSession>>,
        artifact_dir: Option<&Path>,
    ) -> Result<VisualTddReport> {
        info!(name = %scenario.name, url = %scenario.initial_url, "Starting Visual TDD execution");

        let (viewport_w, viewport_h) = scenario.viewport.unwrap_or((1280, 800));
        let session = match session_opt {
            Some(s) => s,
            None => {
                let config = BrowserConfig {
                    headless: true,
                    viewport_width: viewport_w,
                    viewport_height: viewport_h,
                    ..Default::default()
                };
                Arc::new(BrowserSession::launch(config).await?)
            }
        };

        // 1. Initial navigation
        session.navigate(&scenario.initial_url).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let mut captured_screenshots = Vec::new();
        let out_dir = artifact_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("m31a-visual-tdd"));
        let _ = tokio::fs::create_dir_all(&out_dir).await;

        // 2. Execute Action Steps
        for (idx, action) in scenario.actions.iter().enumerate() {
            debug!(step = idx, action = ?action, "Executing action step");
            match action {
                UiActionStep::Navigate { url } => {
                    session.navigate(url).await?;
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                UiActionStep::Click { selector } => {
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector({});
                            if (!el) throw new Error("Element not found: " + {});
                            el.click();
                            return "clicked";
                        }})()"#,
                        serde_json::to_string(selector)?,
                        serde_json::to_string(selector)?
                    );
                    session.evaluate(&script).await?;
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
                UiActionStep::Type { selector, text } => {
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector({});
                            if (!el) throw new Error("Element not found: " + {});
                            el.focus();
                            el.value = {};
                            el.dispatchEvent(new Event("input", {{ bubbles: true }}));
                            el.dispatchEvent(new Event("change", {{ bubbles: true }}));
                            return "typed";
                        }})()"#,
                        serde_json::to_string(selector)?,
                        serde_json::to_string(selector)?,
                        serde_json::to_string(text)?
                    );
                    session.evaluate(&script).await?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                UiActionStep::WaitSelector {
                    selector,
                    timeout_ms,
                } => {
                    let timeout = timeout_ms.unwrap_or(3000);
                    let start = std::time::Instant::now();
                    let check_script = format!(
                        r#"document.querySelector({}) !== null"#,
                        serde_json::to_string(selector)?
                    );
                    let mut found = false;
                    while start.elapsed().as_millis() < timeout as u128 {
                        if let Ok(res) = session.evaluate(&check_script).await
                            && res.trim() == "true"
                        {
                            found = true;
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    if !found {
                        warn!(selector = %selector, timeout_ms = timeout, "Timeout waiting for selector");
                    }
                }
                UiActionStep::WaitMillis { ms } => {
                    tokio::time::sleep(Duration::from_millis(*ms)).await;
                }
                UiActionStep::Screenshot { name } => {
                    let path = out_dir.join(format!("{}.png", name));
                    if let Ok(p) = session.screenshot(Some(&path)).await {
                        captured_screenshots.push(PathBuf::from(p));
                    }
                }
            }
        }

        // 3. Inspect Console & Network
        let console_logs = session.get_console_logs().await;
        let mut console_errors = Vec::new();
        for msg in &console_logs {
            if msg.level.eq_ignore_ascii_case("error") {
                console_errors.push(msg.text.clone());
            }
        }

        let network_records = session.get_network_records().await;
        let mut network_errors = Vec::new();
        for rec in &network_records {
            if let Some(code) = rec.status
                && code >= 400
            {
                network_errors.push(format!("HTTP {} on {}", code, rec.url));
            }
        }

        // 4. Evaluate Assertions
        let mut outcomes = Vec::new();
        for assertion in &scenario.assertions {
            let outcome = match assertion {
                UiAssertion::ElementPresent {
                    selector,
                    should_be_visible,
                } => {
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector({});
                            if (!el) return {{ present: false, visible: false }};
                            const rect = el.getBoundingClientRect();
                            const style = window.getComputedStyle(el);
                            const visible = style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0' && (rect.width > 0 || rect.height > 0);
                            return {{ present: true, visible: visible }};
                        }})()"#,
                        serde_json::to_string(selector)?
                    );
                    let val_str = session
                        .evaluate(&script)
                        .await
                        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
                    let parsed: serde_json::Value =
                        serde_json::from_str(&val_str).unwrap_or_default();
                    let present = parsed
                        .get("present")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let visible = parsed
                        .get("visible")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let passed = if *should_be_visible {
                        present && visible
                    } else {
                        present
                    };

                    AssertionOutcome {
                        description: format!(
                            "Element '{}' present (visible={})",
                            selector, should_be_visible
                        ),
                        passed,
                        expected: format!("present=true, visible={}", should_be_visible),
                        actual: format!("present={}, visible={}", present, visible),
                        error_details: if !passed {
                            Some(format!(
                                "Selector '{}' did not satisfy presence/visibility criteria",
                                selector
                            ))
                        } else {
                            None
                        },
                    }
                }
                UiAssertion::TextMatches {
                    selector,
                    expected,
                    exact,
                } => {
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector({});
                            if (!el) return null;
                            return el.textContent || el.innerText || "";
                        }})()"#,
                        serde_json::to_string(selector)?
                    );
                    let text = session.evaluate(&script).await.unwrap_or_default();
                    let passed = if *exact {
                        text.trim() == expected.trim()
                    } else {
                        text.contains(expected)
                    };

                    AssertionOutcome {
                        description: format!(
                            "Text of '{}' matches '{}' (exact={})",
                            selector, expected, exact
                        ),
                        passed,
                        expected: expected.clone(),
                        actual: text.clone(),
                        error_details: if !passed {
                            Some(format!("Expected text '{}' but found '{}'", expected, text))
                        } else {
                            None
                        },
                    }
                }
                UiAssertion::ComputedStyleMatches {
                    selector,
                    property,
                    expected,
                } => {
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector({});
                            if (!el) return null;
                            return window.getComputedStyle(el).getPropertyValue({}) || "";
                        }})()"#,
                        serde_json::to_string(selector)?,
                        serde_json::to_string(property)?
                    );
                    let actual = session
                        .evaluate(&script)
                        .await
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    let passed = actual.eq_ignore_ascii_case(expected.trim());

                    AssertionOutcome {
                        description: format!(
                            "Computed CSS '{}: {}' on '{}'",
                            property, expected, selector
                        ),
                        passed,
                        expected: expected.clone(),
                        actual: actual.clone(),
                        error_details: if !passed {
                            Some(format!(
                                "Expected CSS property '{}' to be '{}' but received '{}'",
                                property, expected, actual
                            ))
                        } else {
                            None
                        },
                    }
                }
                UiAssertion::AttributeMatches {
                    selector,
                    attribute,
                    expected,
                } => {
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector({});
                            if (!el) return null;
                            return el.getAttribute({}) || "";
                        }})()"#,
                        serde_json::to_string(selector)?,
                        serde_json::to_string(attribute)?
                    );
                    let actual = session
                        .evaluate(&script)
                        .await
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    let passed = actual == expected.trim();

                    AssertionOutcome {
                        description: format!(
                            "Attribute '[{}={}]' on '{}'",
                            attribute, expected, selector
                        ),
                        passed,
                        expected: expected.clone(),
                        actual: actual.clone(),
                        error_details: if !passed {
                            Some(format!(
                                "Expected attribute '{}' to be '{}' but received '{}'",
                                attribute, expected, actual
                            ))
                        } else {
                            None
                        },
                    }
                }
                UiAssertion::ZeroConsoleErrors => {
                    let passed = console_errors.is_empty();
                    AssertionOutcome {
                        description: "Zero JavaScript console errors".to_string(),
                        passed,
                        expected: "0 console errors".to_string(),
                        actual: format!("{} console error(s)", console_errors.len()),
                        error_details: if !passed {
                            Some(console_errors.join("\n"))
                        } else {
                            None
                        },
                    }
                }
                UiAssertion::ZeroNetworkFailures => {
                    let passed = network_errors.is_empty();
                    AssertionOutcome {
                        description: "Zero HTTP 4xx/5xx network failures".to_string(),
                        passed,
                        expected: "0 failed requests".to_string(),
                        actual: format!("{} failed request(s)", network_errors.len()),
                        error_details: if !passed {
                            Some(network_errors.join("\n"))
                        } else {
                            None
                        },
                    }
                }
                UiAssertion::CustomScript {
                    description,
                    script,
                } => {
                    let res = session.evaluate(script).await.unwrap_or_default();
                    let passed = res.trim() == "true" || res.trim() == "\"true\"";
                    AssertionOutcome {
                        description: description.clone(),
                        passed,
                        expected: "truthy (true)".to_string(),
                        actual: res.clone(),
                        error_details: if !passed {
                            Some(format!("Custom evaluation failed with outcome: {}", res))
                        } else {
                            None
                        },
                    }
                }
            };
            outcomes.push(outcome);
        }

        // 5. Final fallback screenshot if assertions failed
        let total_assertions = outcomes.len();
        let passed_assertions = outcomes.iter().filter(|o| o.passed).count();
        let failed_assertions = total_assertions.saturating_sub(passed_assertions);
        let all_passed = failed_assertions == 0;

        if !all_passed {
            let failure_shot =
                out_dir.join(format!("{}_failure.png", scenario.name.replace(' ', "_")));
            if let Ok(p) = session.screenshot(Some(&failure_shot)).await {
                captured_screenshots.push(PathBuf::from(p));
            }
        }

        // 6. Generate Self-Healing Guidance
        let self_healing_guidance = if !all_passed {
            let mut prompt = format!(
                "Visual TDD Assertion Failures on '{}':\n",
                scenario.initial_url
            );
            for o in outcomes.iter().filter(|o| !o.passed) {
                prompt.push_str(&format!(
                    "- [ASSERTION FAILED] {}\n  Expected: {}\n  Actual: {}\n",
                    o.description, o.expected, o.actual
                ));
                if let Some(ref d) = o.error_details {
                    prompt.push_str(&format!("  Details: {}\n", d));
                }
            }
            if !console_errors.is_empty() {
                prompt.push_str("\nRuntime Browser Console Errors:\n");
                for err in &console_errors {
                    prompt.push_str(&format!("  - {}\n", err));
                }
            }
            prompt.push_str("\nPlease inspect the relevant frontend components/styles and produce a patch to resolve these visual/DOM regressions.");
            Some(prompt)
        } else {
            None
        };

        Ok(VisualTddReport {
            scenario_name: scenario.name.clone(),
            initial_url: scenario.initial_url.clone(),
            passed: all_passed,
            total_assertions,
            passed_assertions,
            failed_assertions,
            assertion_outcomes: outcomes,
            console_errors,
            network_errors,
            captured_screenshots,
            self_healing_guidance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_visual_tdd_scenario_execution() {
        let scenario = VisualTddScenario {
            name: "Homepage Navigation Check".to_string(),
            description: Some("Ensures DOM loads with zero console errors".to_string()),
            initial_url: "https://example.com".to_string(),
            viewport: Some((1280, 720)),
            actions: vec![
                UiActionStep::Navigate {
                    url: "https://example.com".to_string(),
                },
                UiActionStep::WaitMillis { ms: 50 },
            ],
            assertions: vec![
                UiAssertion::ZeroConsoleErrors,
                UiAssertion::ZeroNetworkFailures,
            ],
        };

        let report = VisualTddRunner::run_scenario(&scenario, None, None)
            .await
            .unwrap();
        assert_eq!(report.scenario_name, "Homepage Navigation Check");
        assert_eq!(report.total_assertions, 2);
        assert!(report.passed);
    }
}
