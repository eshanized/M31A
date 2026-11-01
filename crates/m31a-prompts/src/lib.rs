//! Prompt Library and Management
//!
//! Versioned prompt templates with variable substitution, categories,
//! and sharing capabilities for reusable prompt engineering.
//!
//! Also provides system prompt fragment resolution for model injection.

pub mod fragment_registry;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: PromptCategory,
    pub tags: Vec<String>,
    pub version: String,
    pub author: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub template: String,
    pub variables: Vec<PromptVariable>,
    pub examples: Vec<PromptExample>,
    pub is_builtin: bool,
}

/// Prompt categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptCategory {
    Coding,
    Debugging,
    CodeReview,
    Architecture,
    Documentation,
    Testing,
    Refactoring,
    Security,
    Performance,
    DataAnalysis,
    DevOps,
    Learning,
    Custom,
}

/// Prompt variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    pub name: String,
    pub description: String,
    pub var_type: VariableType,
    pub required: bool,
    pub default: Option<String>,
    pub options: Option<Vec<String>>, // For enum types
}

/// Variable types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    String,
    Integer,
    Float,
    Boolean,
    Enum,
    FilePath,
    CodeSnippet,
    Language,
}

/// Prompt example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptExample {
    pub name: String,
    pub description: String,
    pub variables: HashMap<String, String>,
    pub expected_output: Option<String>,
}

/// Rendered prompt with variables substituted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedPrompt {
    pub template_id: String,
    pub template_version: String,
    pub rendered: String,
    pub variables_used: HashMap<String, String>,
    pub rendered_at: u64,
}

/// Prompt library
pub struct PromptLibrary {
    templates: Arc<RwLock<HashMap<String, PromptTemplate>>>,
    library_dir: PathBuf,
    builtin_templates: Vec<PromptTemplate>,
}

impl PromptLibrary {
    pub fn new(library_dir: PathBuf) -> Self {
        let mut lib = Self {
            templates: Arc::new(RwLock::new(HashMap::new())),
            library_dir,
            builtin_templates: Vec::new(),
        };
        lib.register_builtin_templates();
        lib.load_custom_templates().ok();
        lib
    }

    /// Register built-in prompt templates
    fn register_builtin_templates(&mut self) {
        // Code Explanation Template
        self.builtin_templates.push(PromptTemplate {
            id: "code-explanation".to_string(),
            name: "Code Explanation".to_string(),
            description: "Explain code in simple terms with examples".to_string(),
            category: PromptCategory::Learning,
            tags: vec!["explain".to_string(), "education".to_string()],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            template: r#"You are an expert software engineer and teacher. Explain the following code in clear, simple terms.

**Code to explain:**
```{{language}}
{{code_snippet}}
```

**Target audience:** {{audience}}

**Please cover:**
1. **Purpose** - What does this code do?
2. **Key concepts** - Important patterns, algorithms, or techniques used
3. **Step-by-step walkthrough** - Line-by-line or block-by-block explanation
4. **Potential issues** - Bugs, edge cases, or improvements
5. **Real-world analogy** - Relate to something familiar

**Output format:** Use clear headings, code blocks for examples, and bullet points for clarity."#.to_string(),
            variables: vec![
                PromptVariable { name: "code_snippet".to_string(), description: "The code to explain".to_string(), var_type: VariableType::CodeSnippet, required: true, default: None, options: None },
                PromptVariable { name: "language".to_string(), description: "Programming language".to_string(), var_type: VariableType::Language, required: true, default: Some("python".to_string()), options: Some(vec!["python".to_string(), "rust".to_string(), "javascript".to_string(), "typescript".to_string(), "go".to_string(), "java".to_string(), "cpp".to_string()]) },
                PromptVariable { name: "audience".to_string(), description: "Target audience level".to_string(), var_type: VariableType::Enum, required: false, default: Some("intermediate".to_string()), options: Some(vec!["beginner".to_string(), "intermediate".to_string(), "advanced".to_string()]) },
            ],
            examples: vec![
                PromptExample {
                    name: "Explain a recursive function".to_string(),
                    description: "Explaining a factorial implementation".to_string(),
                    variables: {
                        let mut m = HashMap::new();
                        m.insert("code_snippet".to_string(), "def factorial(n):\n    if n <= 1:\n        return 1\n    return n * factorial(n - 1)".to_string());
                        m.insert("language".to_string(), "python".to_string());
                        m.insert("audience".to_string(), "beginner".to_string());
                        m
                    },
                    expected_output: None,
                },
            ],
            is_builtin: true,
        });

        // Code Review Template
        self.builtin_templates.push(PromptTemplate {
            id: "code-review".to_string(),
            name: "Code Review".to_string(),
            description: "Comprehensive code review with structured feedback".to_string(),
            category: PromptCategory::CodeReview,
            tags: vec!["review".to_string(), "quality".to_string()],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            template: r#"You are a senior software engineer conducting a thorough code review. Analyze the following changes and provide structured feedback.

**Context:**
- **Repository:** {{repo_name}}
- **Branch:** {{branch_name}}
- **PR/Commit:** {{pr_number}}
- **Author:** {{author}}
- **Files changed:** {{files_count}}

**Code to review:**
```{{language}}
{{code_diff}}
```

**Review criteria:**
1. **Correctness** - Logic errors, edge cases, off-by-one errors
2. **Security** - Vulnerabilities, injection risks, data exposure
3. **Performance** - Algorithmic complexity, memory usage, unnecessary operations
4. **Maintainability** - Code clarity, naming, modularity, DRY principles
5. **Style** - Consistency with project conventions, formatting
6. **Testing** - Test coverage, test quality, edge case coverage
6. **Documentation** - Comments, docstrings, README updates needed

**Output format:**
```
## Summary
[Overall assessment: Approve / Request Changes / Comment]

## Findings

### Critical (must fix)
- [ ] Finding 1: Description, file:line, suggestion

### Major (should fix)
- [ ] Finding 2: Description, file:line, suggestion

### Minor (nice to have)
- [ ] Finding 3: Description, file:line, suggestion

## Praise
- Positive aspects worth noting

## Questions
- Clarifying questions for the author
```"#.to_string(),
            variables: vec![
                PromptVariable { name: "repo_name".to_string(), description: "Repository name".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "branch_name".to_string(), description: "Branch name".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "pr_number".to_string(), description: "PR number or commit hash".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "author".to_string(), description: "PR author".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "files_count".to_string(), description: "Number of files changed".to_string(), var_type: VariableType::Integer, required: true, default: None, options: None },
                PromptVariable { name: "language".to_string(), description: "Primary language".to_string(), var_type: VariableType::Language, required: true, default: Some("python".to_string()), options: None },
                PromptVariable { name: "code_diff".to_string(), description: "Git diff of changes".to_string(), var_type: VariableType::CodeSnippet, required: true, default: None, options: None },
            ],
            examples: vec![],
            is_builtin: true,
        });

        // Debugging Assistant Template
        self.builtin_templates.push(PromptTemplate {
            id: "debugging-assistant".to_string(),
            name: "Debugging Assistant".to_string(),
            description: "Systematic debugging with hypothesis-driven approach".to_string(),
            category: PromptCategory::Debugging,
            tags: vec!["debugging".to_string(), "troubleshooting".to_string()],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            template: r#"You are a debugging expert. Help me systematically debug this issue.

**Issue Description:**
{{bug_description}}

**Environment:**
- Language: {{language}}
- Framework: {{framework}}
- Error message: {{error_message}}
- Stack trace: {{stack_trace}}

**Steps to reproduce:**
{{reproduction_steps}}

**Current behavior:** {{current_behavior}}
**Expected behavior:** {{expected_behavior}}

**Relevant code:**
```{{language}}
{{relevant_code}}
```

**Debugging approach:**
1. **Understand** - Confirm the problem and expected behavior
2. **Reproduce** - Create minimal reproduction
3. **Hypothesize** - Generate potential root causes
4. **Test** - Design experiments to validate/invalidate hypotheses
5. **Fix** - Apply minimal fix
6. **Verify** - Confirm fix works, no regressions

**Please provide:**
1. **Root cause hypotheses** (ranked by likelihood)
2. **Debugging steps** with specific commands/code to test each
3. **Potential fixes** with trade-offs
4. **Prevention** - How to avoid similar issues"#
                .to_string(),
            variables: vec![
                PromptVariable {
                    name: "bug_description".to_string(),
                    description: "What is the bug?".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "language".to_string(),
                    description: "Programming language".to_string(),
                    var_type: VariableType::Language,
                    required: true,
                    default: Some("python".to_string()),
                    options: None,
                },
                PromptVariable {
                    name: "framework".to_string(),
                    description: "Framework/library".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "error_message".to_string(),
                    description: "Error message".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "stack_trace".to_string(),
                    description: "Stack trace".to_string(),
                    var_type: VariableType::CodeSnippet,
                    required: false,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "reproduction_steps".to_string(),
                    description: "Steps to reproduce".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "current_behavior".to_string(),
                    description: "What happens now".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "expected_behavior".to_string(),
                    description: "What should happen".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "relevant_code".to_string(),
                    description: "Code around the bug".to_string(),
                    var_type: VariableType::CodeSnippet,
                    required: false,
                    default: None,
                    options: None,
                },
            ],
            examples: vec![],
            is_builtin: true,
        });

        // Architecture Decision Record Template
        self.builtin_templates.push(PromptTemplate {
            id: "adr-template".to_string(),
            name: "Architecture Decision Record (ADR)".to_string(),
            description: "Create structured Architecture Decision Records".to_string(),
            category: PromptCategory::Architecture,
            tags: vec![
                "architecture".to_string(),
                "adr".to_string(),
                "decision".to_string(),
            ],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            template: r#"# ADR {{adr_number}}: {{title}}

**Status:** {{status}} (Proposed / Accepted / Deprecated / Superseded)
**Date:** {{date}}
**Deciders:** {{deciders}}
**Technical Story:** {{story_link}}

## Context and Problem Statement

{{context}}

## Decision Drivers

{{drivers}}

## Considered Options

{{options}}

## Decision Outcome

**Chosen option:** {{chosen_option}}

### Positive Consequences

{{positive_consequences}}

### Negative Consequences

{{negative_consequences}}

## Pros and Cons of the Options

{{pros_cons}}

## Implementation Plan

{{implementation_plan}}

## Links

- [Related Issue]({{issue_link}})
- [Documentation]({{doc_link}})

---

*This ADR follows the [ADR format](https://adr.github.io/).*"#
                .to_string(),
            variables: vec![
                PromptVariable {
                    name: "adr_number".to_string(),
                    description: "ADR number".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "title".to_string(),
                    description: "Decision title".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "status".to_string(),
                    description: "Decision status".to_string(),
                    var_type: VariableType::Enum,
                    required: true,
                    default: Some("Proposed".to_string()),
                    options: Some(vec![
                        "Proposed".to_string(),
                        "Accepted".to_string(),
                        "Deprecated".to_string(),
                        "Superseded".to_string(),
                    ]),
                },
                PromptVariable {
                    name: "date".to_string(),
                    description: "Decision date".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: Some(chrono::Utc::now().format("%Y-%m-%d").to_string()),
                    options: None,
                },
                PromptVariable {
                    name: "deciders".to_string(),
                    description: "Decision makers".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "story_link".to_string(),
                    description: "Link to related story".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "context".to_string(),
                    description: "Context and problem".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "drivers".to_string(),
                    description: "Decision drivers".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "options".to_string(),
                    description: "Options considered".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "chosen_option".to_string(),
                    description: "Selected option".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "positive_consequences".to_string(),
                    description: "Positive outcomes".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "negative_consequences".to_string(),
                    description: "Negative outcomes".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "pros_cons".to_string(),
                    description: "Pros/cons table".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "implementation_plan".to_string(),
                    description: "Implementation steps".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "issue_link".to_string(),
                    description: "Related issue".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default: None,
                    options: None,
                },
                PromptVariable {
                    name: "doc_link".to_string(),
                    description: "Documentation link".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default: None,
                    options: None,
                },
            ],
            examples: vec![],
            is_builtin: true,
        });

        // TDD Test Generation Template
        self.builtin_templates.push(PromptTemplate {
            id: "tdd-test-generation".to_string(),
            name: "TDD Test Generation".to_string(),
            description: "Generate comprehensive test cases for TDD".to_string(),
            category: PromptCategory::Testing,
            tags: vec!["tdd".to_string(), "testing".to_string(), "generation".to_string()],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            template: r#"You are a test engineer practicing Test-Driven Development. Generate comprehensive test cases for the following requirement.

**Feature:** {{feature_name}}
**Description:** {{feature_description}}
**Language:** {{language}}
**Testing Framework:** {{test_framework}}

**Requirements:**
{{requirements}}

**Edge cases to consider:**
{{edge_cases}}

**Generate:**
1. **Happy path tests** - Normal usage scenarios
2. **Edge case tests** - Boundary values, empty inputs, null/None
3. **Error case tests** - Invalid inputs, exceptions, timeouts
4. **Integration tests** - Interaction with dependencies
5. **Property-based tests** - Invariants that should always hold

**Test structure:**
```{{language}}
# Use {{test_framework}} conventions
# Include setup/teardown if needed
# Use descriptive test names: test_<what>_<when>_<expected>
# Follow AAA pattern: Arrange, Act, Assert
```

**Output only the test code, no explanations.**"#.to_string(),
            variables: vec![
                PromptVariable { name: "feature_name".to_string(), description: "Feature name".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "feature_description".to_string(), description: "Feature description".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "language".to_string(), description: "Programming language".to_string(), var_type: VariableType::Language, required: true, default: Some("python".to_string()), options: None },
                PromptVariable { name: "test_framework".to_string(), description: "Testing framework".to_string(), var_type: VariableType::String, required: true, default: Some("pytest".to_string()), options: Some(vec!["pytest".to_string(), "unittest".to_string(), "jest".to_string(), "mocha".to_string(), "go test".to_string(), "cargo test".to_string()]) },
                PromptVariable { name: "requirements".to_string(), description: "Functional requirements".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "edge_cases".to_string(), description: "Edge cases to test".to_string(), var_type: VariableType::String, required: false, default: None, options: None },
            ],
            examples: vec![],
            is_builtin: true,
        });

        // Refactoring Plan Template
        self.builtin_templates.push(PromptTemplate {
            id: "refactoring-plan".to_string(),
            name: "Refactoring Plan".to_string(),
            description: "Create safe, incremental refactoring plans".to_string(),
            category: PromptCategory::Refactoring,
            tags: vec!["refactoring".to_string(), "planning".to_string(), "safe".to_string()],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            template: r#"# Refactoring Plan: {{target}}

## Current State Analysis

**Target:** {{target}}
**Current Complexity:** {{complexity}} (lines: {{loc}}, functions: {{fn_count}}, cyclomatic: {{cyclomatic}})
**Test Coverage:** {{coverage}}%

## Code Smells Identified

{{smells}}

## Refactoring Goals

{{goals}}

## Refactoring Strategy

**Approach:** {{approach}} (Strangler Fig / Incremental / Big Bang)
**Risk Level:** {{risk_level}}

## Step-by-Step Plan

{{steps}}

## Safety Measures

1. **Characterization Tests** - {{char_tests}}
2. **Incremental Commits** - Commit after each refactoring step
3. **Run Tests After Each Step** - {{test_command}}
4. **Rollback Plan** - {{rollback_plan}}

## Success Criteria

{{success_criteria}}

## Rollback Plan

{{rollback_plan}}

## Timeline Estimate

{{timeline}}"#.to_string(),
            variables: vec![
                PromptVariable { name: "target".to_string(), description: "Code to refactor".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "complexity".to_string(), description: "Complexity rating".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "loc".to_string(), description: "Lines of code".to_string(), var_type: VariableType::Integer, required: true, default: None, options: None },
                PromptVariable { name: "fn_count".to_string(), description: "Function count".to_string(), var_type: VariableType::Integer, required: true, default: None, options: None },
                PromptVariable { name: "cyclomatic".to_string(), description: "Cyclomatic complexity".to_string(), var_type: VariableType::Integer, required: true, default: None, options: None },
                PromptVariable { name: "coverage".to_string(), description: "Test coverage %".to_string(), var_type: VariableType::Integer, required: true, default: None, options: None },
                PromptVariable { name: "smells".to_string(), description: "Code smells found".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "goals".to_string(), description: "Refactoring goals".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "approach".to_string(), description: "Refactoring approach".to_string(), var_type: VariableType::Enum, required: true, default: Some("Incremental".to_string()), options: Some(vec!["Strangler Fig".to_string(), "Incremental".to_string(), "Big Bang".to_string()]) },
                PromptVariable { name: "risk_level".to_string(), description: "Risk level".to_string(), var_type: VariableType::Enum, required: true, default: Some("Medium".to_string()), options: Some(vec!["Low".to_string(), "Medium".to_string(), "High".to_string()]) },
                PromptVariable { name: "steps".to_string(), description: "Step-by-step plan".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "char_tests".to_string(), description: "Characterization tests".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "test_command".to_string(), description: "Test command".to_string(), var_type: VariableType::String, required: true, default: Some("pytest".to_string()), options: None },
                PromptVariable { name: "rollback_plan".to_string(), description: "Rollback plan".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "success_criteria".to_string(), description: "Success criteria".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "timeline".to_string(), description: "Timeline estimate".to_string(), var_type: VariableType::String, required: false, default: None, options: None },
            ],
            examples: vec![],
            is_builtin: true,
        });

        // Security Threat Modeling Template
        self.builtin_templates.push(PromptTemplate {
            id: "threat-modeling".to_string(),
            name: "Security Threat Modeling (STRIDE)".to_string(),
            description: "Create STRIDE threat models for systems".to_string(),
            category: PromptCategory::Security,
            tags: vec!["security".to_string(), "threat-modeling".to_string(), "stride".to_string()],
            version: "1.0.0".to_string(),
            author: "M31A".to_string(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            template: r#"# Threat Model: {{system_name}}

**System:** {{system_name}}
**Scope:** {{scope}}
**Date:** {{date}}
**Author:** {{author}}
**Version:** {{version}}

## System Overview

{{overview}}

## Data Flow Diagram

{{dfd_description}}

## Trust Boundaries

{{trust_boundaries}}

## STRIDE Analysis

| Component | Spoofing | Tampering | Repudiation | Information Disclosure | DoS | Elevation of Privilege |
|-----------|----------|-----------|-------------|------------------------|-----|------------------------|
{{stride_table}}

## Threat Details

{{threat_details}}

## Mitigations

| Threat | Likelihood | Impact | Severity | Mitigation | Status |
|--------|------------|--------|----------|------------|--------|
{{mitigations_table}}

## Residual Risks

{{residual_risks}}

## Action Items

{{action_items}}

## Review

**Reviewed by:** {{reviewers}}
**Next review:** {{next_review_date}}"#.to_string(),
            variables: vec![
                PromptVariable { name: "system_name".to_string(), description: "System name".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "scope".to_string(), description: "Scope of threat model".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "date".to_string(), description: "Date".to_string(), var_type: VariableType::String, required: true, default: Some(chrono::Utc::now().format("%Y-%m-%d").to_string()), options: None },
                PromptVariable { name: "author".to_string(), description: "Author".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "version".to_string(), description: "Version".to_string(), var_type: VariableType::String, required: true, default: Some("1.0".to_string()), options: None },
                PromptVariable { name: "overview".to_string(), description: "System overview".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "dfd_description".to_string(), description: "Data flow diagram".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "trust_boundaries".to_string(), description: "Trust boundaries".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "stride_table".to_string(), description: "STRIDE table".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "threat_details".to_string(), description: "Threat details".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "mitigations_table".to_string(), description: "Mitigations table".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "residual_risks".to_string(), description: "Residual risks".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "action_items".to_string(), description: "Action items".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "reviewers".to_string(), description: "Reviewers".to_string(), var_type: VariableType::String, required: true, default: None, options: None },
                PromptVariable { name: "next_review_date".to_string(), description: "Next review date".to_string(), var_type: VariableType::String, required: false, default: None, options: None },
            ],
            examples: vec![],
            is_builtin: true,
        });

        // Register all builtin templates
        for template in &self.builtin_templates {
            self.templates
                .try_write()
                .expect("uncontended")
                .insert(template.id.clone(), template.clone());
        }
    }

    /// Load custom templates from directory
    fn load_custom_templates(&mut self) -> Result<()> {
        if !self.library_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.library_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(template) = toml::from_str::<PromptTemplate>(&content) {
                    self.templates
                        .try_write()
                        .expect("uncontended")
                        .insert(template.id.clone(), template);
                }
            }
        }
        Ok(())
    }

    /// Render a prompt template with variables
    pub async fn render(
        &self,
        template_id: &str,
        variables: HashMap<String, String>,
    ) -> Result<RenderedPrompt> {
        let template = {
            let templates = self.templates.read().await;
            templates.get(template_id).cloned()
        }
        .context(format!("Template '{}' not found", template_id))?;

        // Validate required variables
        for var in &template.variables {
            if var.required && !variables.contains_key(&var.name) && var.default.is_none() {
                return Err(anyhow::anyhow!(
                    "Required variable '{}' not provided",
                    var.name
                ));
            }
        }

        // Apply defaults
        let mut final_vars = variables;
        for var in &template.variables {
            if !final_vars.contains_key(&var.name)
                && let Some(default) = &var.default
            {
                final_vars.insert(var.name.clone(), default.clone());
            }
        }

        // Render template
        let mut rendered = template.template.clone();
        for (key, value) in &final_vars {
            rendered = rendered.replace(&format!("{{{{{}}}}}", key), value);
        }

        Ok(RenderedPrompt {
            template_id: template.id,
            template_version: template.version,
            rendered,
            variables_used: final_vars,
            rendered_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Get a template by ID
    pub async fn get(&self, id: &str) -> Option<PromptTemplate> {
        let templates = self.templates.read().await;
        templates.get(id).cloned()
    }

    /// List all templates
    pub async fn list(&self) -> Vec<PromptTemplate> {
        self.templates.read().await.values().cloned().collect()
    }

    /// List templates by category
    pub async fn list_by_category(&self, category: PromptCategory) -> Vec<PromptTemplate> {
        self.templates
            .read()
            .await
            .values()
            .filter(|t| t.category == category)
            .cloned()
            .collect()
    }

    /// Search templates
    pub async fn search(&self, query: &str) -> Vec<PromptTemplate> {
        let query = query.to_lowercase();
        self.templates
            .read()
            .await
            .values()
            .filter(|t| {
                t.name.to_lowercase().contains(&query)
                    || t.description.to_lowercase().contains(&query)
                    || t.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    /// Save a custom template
    pub async fn save(&self, template: PromptTemplate) -> Result<()> {
        std::fs::create_dir_all(&self.library_dir)?;
        let path = self.library_dir.join(format!("{}.toml", template.id));
        let content = toml::to_string_pretty(&template)?;
        std::fs::write(path, content)?;
        self.templates
            .write()
            .await
            .insert(template.id.clone(), template);
        Ok(())
    }

    /// Delete a template
    pub async fn delete(&self, id: &str) -> Result<()> {
        let template = self.get(id).await.context("Template not found")?;
        if template.is_builtin {
            return Err(anyhow::anyhow!("Cannot delete builtin template"));
        }
        let path = self.library_dir.join(format!("{}.toml", id));
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        self.templates.write().await.remove(id);
        Ok(())
    }
}

impl Default for PromptVariable {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            var_type: VariableType::String,
            required: true,
            default: None,
            options: None,
        }
    }
}

pub use fragment_registry::{
    Fragment, FragmentCondition, FragmentRegistry, ModelFamily, ResolvedFragments,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builtin_templates() {
        let library = PromptLibrary::new(PathBuf::from("/tmp/prompts"));
        assert!(library.get("code-explanation").await.is_some());
        assert!(library.get("code-review").await.is_some());
        assert!(library.get("debugging-assistant").await.is_some());
        assert!(library.get("adr-template").await.is_some());
        assert!(library.get("tdd-test-generation").await.is_some());
        assert!(library.get("refactoring-plan").await.is_some());
        assert!(library.get("threat-modeling").await.is_some());
    }

    #[tokio::test]
    async fn test_render_template() {
        let library = PromptLibrary::new(PathBuf::from("/tmp/prompts"));
        let mut vars = HashMap::new();
        vars.insert(
            "code_snippet".to_string(),
            "def hello():\n    print('Hello')".to_string(),
        );
        vars.insert("language".to_string(), "python".to_string());
        vars.insert("audience".to_string(), "beginner".to_string());

        let rendered = library.render("code-explanation", vars).await.unwrap();
        assert!(rendered.rendered.contains("def hello()"));
        assert!(rendered.rendered.contains("python"));
        assert!(rendered.rendered.contains("beginner"));
    }
}
