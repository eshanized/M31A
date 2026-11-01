//! Collaboration Session Templates
//!
//! Pre-defined session configurations for common workflows like pair programming,
//! code review, debugging, architecture design, etc.

use anyhow::{Context, Result};
use m31a_core::{
    config::{ApprovalPolicy, SandboxPolicy},
    prompts::CollaborationMode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Collaboration session template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: TemplateCategory,
    pub icon: String,
    pub config: TemplateConfig,
    pub steps: Vec<TemplateStep>,
    pub tags: Vec<String>,
    pub estimated_duration_minutes: Option<u32>,
}

/// Template categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateCategory {
    PairProgramming,
    CodeReview,
    Debugging,
    ArchitectureDesign,
    FeatureDevelopment,
    Refactoring,
    Testing,
    Documentation,
    Onboarding,
    SecurityAudit,
    PerformanceOptimization,
    Custom,
}

/// Template configuration overrides
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateConfig {
    pub collaboration_mode: Option<CollaborationMode>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub approval_policy: Option<ApprovalPolicy>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub environment_vars: HashMap<String, String>,
    pub disabled_tools: Vec<String>,
    pub enabled_tools: Vec<String>,
    pub context_files: Vec<String>,
    pub initial_prompt: Option<String>,
}

/// Template step for guided workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStep {
    pub order: usize,
    pub title: String,
    pub description: String,
    pub prompt_template: Option<String>,
    pub expected_tools: Vec<String>,
    pub validation: Option<String>,
    pub is_optional: bool,
}

/// Template library/registry
pub struct TemplateLibrary {
    templates: HashMap<String, CollaborationTemplate>,
    custom_templates_dir: PathBuf,
}

impl TemplateLibrary {
    pub fn new(custom_templates_dir: PathBuf) -> Self {
        let mut library = Self {
            templates: HashMap::new(),
            custom_templates_dir,
        };
        library.register_builtin_templates();
        library.load_custom_templates().ok();
        library
    }

    /// Register built-in templates
    fn register_builtin_templates(&mut self) {
        // Pair Programming Template
        self.register(CollaborationTemplate {
            id: "pair-programming".to_string(),
            name: "Pair Programming".to_string(),
            description: "Two developers working together on the same task".to_string(),
            category: TemplateCategory::PairProgramming,
            icon: "👥".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Default),
                sandbox_policy: Some(SandboxPolicy::WorkspaceWrite),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                reasoning_effort: Some("medium".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec![],
                enabled_tools: vec!["exec_command".to_string(), "read_file".to_string(), "write_file".to_string(), "apply_patch".to_string()],
                context_files: vec![],
                initial_prompt: Some("We are pair programming. I'll be the driver and you'll be the navigator. Let's work on: {task}".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Understand the Task".to_string(),
                    description: "Discuss and clarify what we're building".to_string(),
                    prompt_template: Some("Let's break down the task: {task_description}. What are the key requirements?".to_string()),
                    expected_tools: vec!["read_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Design Approach".to_string(),
                    description: "Agree on implementation strategy".to_string(),
                    prompt_template: Some("What's our approach? Consider: architecture, libraries, patterns, edge cases.".to_string()),
                    expected_tools: vec![],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Implement".to_string(),
                    description: "Write code together with driver/navigator roles".to_string(),
                    prompt_template: Some("Let's implement {component}. I'll type, you review.".to_string()),
                    expected_tools: vec!["write_file".to_string(), "apply_patch".to_string(), "exec_command".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 4,
                    title: "Test & Verify".to_string(),
                    description: "Run tests and verify implementation".to_string(),
                    prompt_template: Some("Run the tests to verify our implementation works correctly.".to_string()),
                    expected_tools: vec!["exec_command".to_string()],
                    validation: None,
                    is_optional: false,
                },
            ],
            tags: vec!["collaborative".to_string(), "real-time".to_string()],
            estimated_duration_minutes: Some(60),
        });

        // Code Review Template
        self.register(CollaborationTemplate {
            id: "code-review".to_string(),
            name: "Code Review".to_string(),
            description: "Structured code review with checklist and feedback".to_string(),
            category: TemplateCategory::CodeReview,
            icon: "🔍".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Plan),
                sandbox_policy: Some(SandboxPolicy::ReadOnly),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("deepseek-ai/deepseek-r1".to_string()),
                reasoning_effort: Some("high".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec!["exec_command".to_string(), "write_file".to_string(), "apply_patch".to_string()],
                enabled_tools: vec!["read_file".to_string(), "file_search".to_string(), "view_image".to_string()],
                context_files: vec![],
                initial_prompt: Some("Please review the following code changes. Focus on: correctness, security, performance, maintainability, and style. Provide specific, actionable feedback.".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Understand Context".to_string(),
                    description: "Read the PR description and related issues".to_string(),
                    prompt_template: Some("Let me first understand the context by reading the PR description and related files.".to_string()),
                    expected_tools: vec!["read_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Review Changes".to_string(),
                    description: "Examine each file change systematically".to_string(),
                    prompt_template: Some("Reviewing file: {file_path}. Checking for: logic errors, security issues, performance, style.".to_string()),
                    expected_tools: vec!["read_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Run Automated Checks".to_string(),
                    description: "Execute linters, type checkers, and tests".to_string(),
                    prompt_template: Some("Running automated verification on the changes.".to_string()),
                    expected_tools: vec!["exec_command".to_string()],
                    validation: Some("All checks should pass".to_string()),
                    is_optional: true,
                },
                TemplateStep {
                    order: 4,
                    title: "Provide Feedback".to_string(),
                    description: "Summarize findings with specific suggestions".to_string(),
                    prompt_template: Some("Here's my review summary with {count} findings: {findings}".to_string()),
                    expected_tools: vec![],
                    validation: None,
                    is_optional: false,
                },
            ],
            tags: vec!["review".to_string(), "quality".to_string()],
            estimated_duration_minutes: Some(30),
        });

        // Debugging Session Template
        self.register(CollaborationTemplate {
            id: "debugging".to_string(),
            name: "Debugging Session".to_string(),
            description: "Systematic debugging with hypothesis-driven approach".to_string(),
            category: TemplateCategory::Debugging,
            icon: "🐛".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Default),
                sandbox_policy: Some(SandboxPolicy::WorkspaceWrite),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                reasoning_effort: Some("high".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec![],
                enabled_tools: vec!["exec_command".to_string(), "read_file".to_string(), "write_file".to_string(), "apply_patch".to_string(), "file_search".to_string()],
                context_files: vec![],
                initial_prompt: Some("We're debugging: {bug_description}. Let's use a systematic approach: reproduce, hypothesize, test, fix.".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Reproduce the Bug".to_string(),
                    description: "Create a minimal reproduction case".to_string(),
                    prompt_template: Some("Let me first reproduce the issue: {reproduction_steps}".to_string()),
                    expected_tools: vec!["exec_command".to_string(), "read_file".to_string()],
                    validation: Some("Bug should be consistently reproducible".to_string()),
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Form Hypotheses".to_string(),
                    description: "List possible root causes".to_string(),
                    prompt_template: Some("Possible causes: 1) {hypothesis1} 2) {hypothesis2} 3) {hypothesis3}".to_string()),
                    expected_tools: vec![],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Test Hypotheses".to_string(),
                    description: "Test each hypothesis with targeted experiments".to_string(),
                    prompt_template: Some("Testing hypothesis: {hypothesis}. Running: {test_command}".to_string()),
                    expected_tools: vec!["exec_command".to_string(), "read_file".to_string(), "write_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 4,
                    title: "Implement Fix".to_string(),
                    description: "Apply the minimal fix and verify".to_string(),
                    prompt_template: Some("Applying fix for {root_cause}. The fix: {fix_description}".to_string()),
                    expected_tools: vec!["write_file".to_string(), "apply_patch".to_string()],
                    validation: Some("Bug should be fixed and tests pass".to_string()),
                    is_optional: false,
                },
                TemplateStep {
                    order: 5,
                    title: "Regression Test".to_string(),
                    description: "Ensure fix doesn't break existing functionality".to_string(),
                    prompt_template: Some("Running full test suite to check for regressions.".to_string()),
                    expected_tools: vec!["exec_command".to_string()],
                    validation: Some("All tests should pass".to_string()),
                    is_optional: true,
                },
            ],
            tags: vec!["debugging".to_string(), "systematic".to_string()],
            estimated_duration_minutes: Some(45),
        });

        // Architecture Design Template
        self.register(CollaborationTemplate {
            id: "architecture-design".to_string(),
            name: "Architecture Design".to_string(),
            description: "Collaborative system design with ADR documentation".to_string(),
            category: TemplateCategory::ArchitectureDesign,
            icon: "🏗️".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Plan),
                sandbox_policy: Some(SandboxPolicy::ReadOnly),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("deepseek-ai/deepseek-r1".to_string()),
                reasoning_effort: Some("high".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec!["exec_command".to_string(), "write_file".to_string(), "apply_patch".to_string()],
                enabled_tools: vec!["read_file".to_string(), "file_search".to_string()],
                context_files: vec![],
                initial_prompt: Some("We're designing: {system_description}. Let's create an architecture decision record (ADR) covering: context, decision, consequences, alternatives considered.".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Define Requirements".to_string(),
                    description: "Capture functional and non-functional requirements".to_string(),
                    prompt_template: Some("Requirements: Functional: {functional}. Non-functional: {non_functional}".to_string()),
                    expected_tools: vec![],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Explore Alternatives".to_string(),
                    description: "Research and compare architectural options".to_string(),
                    prompt_template: Some("Comparing alternatives: {alternatives}. Criteria: {criteria}".to_string()),
                    expected_tools: vec!["file_search".to_string(), "read_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Make Decision".to_string(),
                    description: "Choose architecture and document rationale".to_string(),
                    prompt_template: Some("Decision: {decision}. Rationale: {rationale}. Consequences: {consequences}".to_string()),
                    expected_tools: vec![],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 4,
                    title: "Document ADR".to_string(),
                    description: "Create Architecture Decision Record".to_string(),
                    prompt_template: Some("Creating ADR: {adr_content}".to_string()),
                    expected_tools: vec!["write_file".to_string()],
                    validation: None,
                    is_optional: true,
                },
            ],
            tags: vec!["architecture".to_string(), "design".to_string(), "adr".to_string()],
            estimated_duration_minutes: Some(60),
        });

        // Feature Development Template
        self.register(CollaborationTemplate {
            id: "feature-development".to_string(),
            name: "Feature Development".to_string(),
            description: "End-to-end feature implementation with TDD".to_string(),
            category: TemplateCategory::FeatureDevelopment,
            icon: "✨".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Default),
                sandbox_policy: Some(SandboxPolicy::WorkspaceWrite),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                reasoning_effort: Some("medium".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec![],
                enabled_tools: vec!["exec_command".to_string(), "read_file".to_string(), "write_file".to_string(), "apply_patch".to_string(), "file_search".to_string()],
                context_files: vec![],
                initial_prompt: Some("We're building: {feature_description}. Let's use TDD: write failing test, make it pass, refactor.".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Specify Requirements".to_string(),
                    description: "Write user stories and acceptance criteria".to_string(),
                    prompt_template: Some("User story: As a {user}, I want {action} so that {benefit}. Acceptance criteria: {criteria}".to_string()),
                    expected_tools: vec![],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Write Failing Tests".to_string(),
                    description: "Implement test cases for the feature".to_string(),
                    prompt_template: Some("Writing tests for: {feature}. Test cases: {test_cases}".to_string()),
                    expected_tools: vec!["write_file".to_string()],
                    validation: Some("Tests should fail initially".to_string()),
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Implement Feature".to_string(),
                    description: "Write minimal code to pass tests".to_string(),
                    prompt_template: Some("Implementing {component} to make tests pass.".to_string()),
                    expected_tools: vec!["write_file".to_string(), "apply_patch".to_string()],
                    validation: Some("All tests should pass".to_string()),
                    is_optional: false,
                },
                TemplateStep {
                    order: 4,
                    title: "Refactor & Polish".to_string(),
                    description: "Clean up code, add documentation".to_string(),
                    prompt_template: Some("Refactoring {component} for clarity and maintainability.".to_string()),
                    expected_tools: vec!["write_file".to_string(), "apply_patch".to_string()],
                    validation: None,
                    is_optional: true,
                },
                TemplateStep {
                    order: 5,
                    title: "Verify & Demo".to_string(),
                    description: "Run full test suite and demonstrate feature".to_string(),
                    prompt_template: Some("Running verification and demonstrating the feature.".to_string()),
                    expected_tools: vec!["exec_command".to_string()],
                    validation: Some("All tests pass, feature works as expected".to_string()),
                    is_optional: false,
                },
            ],
            tags: vec!["tdd".to_string(), "feature".to_string()],
            estimated_duration_minutes: Some(90),
        });

        // Refactoring Template
        self.register(CollaborationTemplate {
            id: "refactoring".to_string(),
            name: "Refactoring".to_string(),
            description: "Safe, incremental refactoring with characterization tests".to_string(),
            category: TemplateCategory::Refactoring,
            icon: "♻️".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Default),
                sandbox_policy: Some(SandboxPolicy::WorkspaceWrite),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("nvidia/nemotron-3.5-lightning-30b-a3b".to_string()),
                reasoning_effort: Some("medium".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec![],
                enabled_tools: vec!["exec_command".to_string(), "read_file".to_string(), "write_file".to_string(), "apply_patch".to_string(), "file_search".to_string()],
                context_files: vec![],
                initial_prompt: Some("We're refactoring: {target}. First, let's write characterization tests to lock in current behavior.".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Characterize Current Behavior".to_string(),
                    description: "Write tests that capture existing behavior".to_string(),
                    prompt_template: Some("Writing characterization tests for {target}. These tests lock in current behavior.".to_string()),
                    expected_tools: vec!["read_file".to_string(), "write_file".to_string(), "exec_command".to_string()],
                    validation: Some("Tests pass and capture current behavior".to_string()),
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Identify Refactoring Goals".to_string(),
                    description: "Define what to improve and why".to_string(),
                    prompt_template: Some("Refactoring goals: {goals}. Code smells identified: {smells}".to_string()),
                    expected_tools: vec!["read_file".to_string(), "file_search".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Apply Refactorings".to_string(),
                    description: "Make small, safe changes with tests passing after each".to_string(),
                    prompt_template: Some("Applying refactoring: {refactoring_type} to {target}. Running tests after each change.".to_string()),
                    expected_tools: vec!["write_file".to_string(), "apply_patch".to_string(), "exec_command".to_string()],
                    validation: Some("All tests pass after each refactoring step".to_string()),
                    is_optional: false,
                },
                TemplateStep {
                    order: 4,
                    title: "Verify Behavior Preserved".to_string(),
                    description: "Run full test suite to ensure no regressions".to_string(),
                    prompt_template: Some("Running full test suite to verify behavior is preserved.".to_string()),
                    expected_tools: vec!["exec_command".to_string()],
                    validation: Some("All tests pass".to_string()),
                    is_optional: false,
                },
            ],
            tags: vec!["refactoring".to_string(), "safe".to_string()],
            estimated_duration_minutes: Some(60),
        });

        // Security Audit Template
        self.register(CollaborationTemplate {
            id: "security-audit".to_string(),
            name: "Security Audit".to_string(),
            description: "Comprehensive security review with threat modeling".to_string(),
            category: TemplateCategory::SecurityAudit,
            icon: "🔒".to_string(),
            config: TemplateConfig {
                collaboration_mode: Some(CollaborationMode::Plan),
                sandbox_policy: Some(SandboxPolicy::ReadOnly),
                approval_policy: Some(ApprovalPolicy::OnRequest),
                model: Some("deepseek-ai/deepseek-r1".to_string()),
                reasoning_effort: Some("high".to_string()),
                environment_vars: HashMap::new(),
                disabled_tools: vec!["exec_command".to_string(), "write_file".to_string(), "apply_patch".to_string()],
                enabled_tools: vec!["read_file".to_string(), "file_search".to_string(), "view_image".to_string()],
                context_files: vec![],
                initial_prompt: Some("Conducting a security audit of: {target}. We'll use STRIDE threat modeling and check for OWASP Top 10 vulnerabilities.".to_string()),
            },
            steps: vec![
                TemplateStep {
                    order: 1,
                    title: "Threat Modeling".to_string(),
                    description: "Create STRIDE model and identify attack surfaces".to_string(),
                    prompt_template: Some("STRIDE Analysis for {target}: Spoofing: {spoofing} Tampering: {tampering} Repudiation: {repudiation} Information Disclosure: {info_disclosure} Denial of Service: {dos} Elevation of Privilege: {eop}".to_string()),
                    expected_tools: vec!["read_file".to_string(), "file_search".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 2,
                    title: "Code Review for Vulnerabilities".to_string(),
                    description: "Check for OWASP Top 10 and common vulnerabilities".to_string(),
                    prompt_template: Some("Reviewing for: Injection, Broken Auth, Sensitive Data Exposure, XXE, Broken Access Control, Security Misconfig, XSS, Insecure Deserialization, Vulnerable Components, Insufficient Logging.".to_string()),
                    expected_tools: vec!["read_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
                TemplateStep {
                    order: 3,
                    title: "Dependency Scanning".to_string(),
                    description: "Check for vulnerable dependencies".to_string(),
                    prompt_template: Some("Scanning dependencies for known vulnerabilities.".to_string()),
                    expected_tools: vec!["exec_command".to_string()],
                    validation: None,
                    is_optional: true,
                },
                TemplateStep {
                    order: 4,
                    title: "Report Findings".to_string(),
                    description: "Document vulnerabilities with severity and remediation".to_string(),
                    prompt_template: Some("Security findings: {findings}. Severity: {severity}. Remediation: {remediation}".to_string()),
                    expected_tools: vec!["write_file".to_string()],
                    validation: None,
                    is_optional: false,
                },
            ],
            tags: vec!["security".to_string(), "audit".to_string(), "threat-modeling".to_string()],
            estimated_duration_minutes: Some(60),
        });
    }

    /// Register a template
    pub fn register(&mut self, template: CollaborationTemplate) {
        self.templates.insert(template.id.clone(), template);
    }

    /// Load custom templates from directory
    fn load_custom_templates(&mut self) -> Result<()> {
        if !self.custom_templates_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.custom_templates_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(template) = toml::from_str::<CollaborationTemplate>(&content) {
                    self.register(template);
                }
            }
        }
        Ok(())
    }

    /// Get a template by ID
    pub fn get(&self, id: &str) -> Option<&CollaborationTemplate> {
        self.templates.get(id)
    }

    /// List all templates
    pub fn list(&self) -> Vec<&CollaborationTemplate> {
        self.templates.values().collect()
    }

    /// List templates by category
    pub fn list_by_category(&self, category: TemplateCategory) -> Vec<&CollaborationTemplate> {
        self.templates
            .values()
            .filter(|t| t.category == category)
            .collect()
    }

    /// Save a custom template
    pub fn save_custom(&self, template: &CollaborationTemplate) -> Result<()> {
        std::fs::create_dir_all(&self.custom_templates_dir)?;
        let path = self
            .custom_templates_dir
            .join(format!("{}.toml", template.id));
        let content = toml::to_string_pretty(template)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Apply a template to create a session config
    pub fn apply_template(
        &self,
        id: &str,
        variables: HashMap<String, String>,
    ) -> Result<TemplateConfig> {
        let template = self
            .get(id)
            .context(format!("Template '{}' not found", id))?;
        let mut config = template.config.clone();

        // Substitute variables in initial prompt
        if let Some(prompt) = &mut config.initial_prompt {
            for (key, value) in &variables {
                *prompt = prompt.replace(&format!("{{{}}}", key), value);
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_templates() {
        let library = TemplateLibrary::new(PathBuf::from("/tmp/templates"));
        assert!(library.get("pair-programming").is_some());
        assert!(library.get("code-review").is_some());
        assert!(library.get("debugging").is_some());
        assert!(library.get("architecture-design").is_some());
        assert!(library.get("feature-development").is_some());
        assert!(library.get("refactoring").is_some());
        assert!(library.get("security-audit").is_some());
    }

    #[test]
    fn test_template_variables() {
        let library = TemplateLibrary::new(PathBuf::from("/tmp/templates"));
        let mut vars = HashMap::new();
        vars.insert(
            "task".to_string(),
            "implement user authentication".to_string(),
        );
        vars.insert(
            "task_description".to_string(),
            "Add JWT-based auth".to_string(),
        );

        let config = library.apply_template("pair-programming", vars).unwrap();
        assert!(
            config
                .initial_prompt
                .unwrap()
                .contains("implement user authentication")
        );
    }
}
