//! Focused Workspace Configuration Screen (Section 4).
//!
//! Provides the canonical workspace configuration experience:
//! - Focused, reliable, keyboard-driven interface.
//! - Captures essential project facts without an artificial 7-step wizard.
//! - Pre-fills sensible defaults via repository inspection (`Cargo.toml`, `package.json`, etc.).
//! - Allows single-confirmation acceptance of detected defaults.
//! - Atomically persists canonical configuration to `<workspace>/.m31a/M31A.toml`.
//! - Never writes credentials or secrets to workspace files.
//! - Preserves existing configuration sections during reconfiguration.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::paths::PlatformPaths;
use crate::config::schema::{
    AppConfig, GitPushPolicy, WorkspaceVerificationConfig, parse_and_validate_config,
    validate_config,
};
use crate::tui::input::text_input::TextInput;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Outcome of handling an input event in the workspace setup screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSetupOutcome {
    None,
    AcceptedDefaults,
    Confirmed,
    Cancelled,
    Error(String),
}

/// Project type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectTypeChoice {
    Library,
    Cli,
    WebService,
    Frontend,
    FullStackMonorepo,
    SystemsTool,
    DataPipeline,
    Generic,
}

impl ProjectTypeChoice {
    pub const ALL: &'static [Self] = &[
        Self::Cli,
        Self::Library,
        Self::WebService,
        Self::Frontend,
        Self::FullStackMonorepo,
        Self::SystemsTool,
        Self::DataPipeline,
        Self::Generic,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Cli => "cli",
            Self::WebService => "web_service",
            Self::Frontend => "frontend",
            Self::FullStackMonorepo => "monorepo",
            Self::SystemsTool => "systems_tool",
            Self::DataPipeline => "data_pipeline",
            Self::Generic => "generic",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Library => "Library / Package",
            Self::Cli => "Command-Line Tool (CLI)",
            Self::WebService => "Backend / Web Service",
            Self::Frontend => "Frontend Web Application",
            Self::FullStackMonorepo => "Full-Stack Monorepo",
            Self::SystemsTool => "Systems / Embedded Tool",
            Self::DataPipeline => "Data / ML Pipeline",
            Self::Generic => "Generic Project",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "library" | "lib" | "package" => Self::Library,
            "cli" | "cmd" | "tool" | "binary" => Self::Cli,
            "web_service" | "web" | "service" | "backend" | "api" => Self::WebService,
            "frontend" | "spa" | "ui" => Self::Frontend,
            "monorepo" | "fullstack" | "full-stack" => Self::FullStackMonorepo,
            "systems_tool" | "systems" | "system" => Self::SystemsTool,
            "data_pipeline" | "data" | "pipeline" | "ml" => Self::DataPipeline,
            _ => Self::Generic,
        }
    }
}

/// Git execution isolation preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitIsolationChoice {
    WorktreePerExecution,
    WorktreePerFeature,
    BranchOnly,
    InPlace,
}

impl GitIsolationChoice {
    pub const ALL: &'static [Self] = &[
        Self::WorktreePerExecution,
        Self::WorktreePerFeature,
        Self::BranchOnly,
        Self::InPlace,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorktreePerExecution => "worktree_per_execution",
            Self::WorktreePerFeature => "worktree_per_feature",
            Self::BranchOnly => "branch_only",
            Self::InPlace => "in_place",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::WorktreePerExecution => "Isolated Worktree per Execution (Recommended)",
            Self::WorktreePerFeature => "Isolated Worktree per Feature",
            Self::BranchOnly => "Branch-only Isolation (Shared Worktree)",
            Self::InPlace => "In-place modifications with Rollback Protection",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "worktree_per_feature" | "feature" => Self::WorktreePerFeature,
            "branch_only" | "branch" => Self::BranchOnly,
            "in_place" | "inplace" => Self::InPlace,
            _ => Self::WorktreePerExecution,
        }
    }
}

/// Autonomous policy posture choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyPostureChoice {
    Balanced,
    ReviewRequired,
    ReadOnlyAnalysis,
    LocalChangesOnly,
}

impl PolicyPostureChoice {
    pub const ALL: &'static [Self] = &[
        Self::Balanced,
        Self::ReviewRequired,
        Self::ReadOnlyAnalysis,
        Self::LocalChangesOnly,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::ReviewRequired => "review_required",
            Self::ReadOnlyAnalysis => "read_only_analysis",
            Self::LocalChangesOnly => "local_changes_only",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Balanced => "Balanced Autonomy (Governed autonomous execution)",
            Self::ReviewRequired => "Review Required (Approvals on file writes / shell commands)",
            Self::ReadOnlyAnalysis => "Read-Only Analysis (Inspection and planning only)",
            Self::LocalChangesOnly => "Local Changes Only (No remote git push or external network)",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "review_required" | "conservative" | "safe" => Self::ReviewRequired,
            "read_only_analysis" | "readonly" => Self::ReadOnlyAnalysis,
            "local_changes_only" | "local" => Self::LocalChangesOnly,
            _ => Self::Balanced,
        }
    }
}

/// Information detected automatically from workspace inspection.
#[derive(Debug, Clone, Default)]
pub struct DetectedRepoContext {
    pub manifest_file: Option<String>,
    pub detected_type: Option<ProjectTypeChoice>,
    pub detected_languages: Vec<String>,
    pub detected_build_system: Option<String>,
    pub detected_package_manager: Option<String>,
    pub detected_test_command: Option<String>,
    pub detected_framework: Option<String>,
    pub detected_database: Option<String>,
    pub is_git_repo: bool,
    pub is_empty_dir: bool,
}

impl DetectedRepoContext {
    /// Inspect a directory and deduce sensible defaults.
    pub fn inspect(root: &Path) -> Self {
        let mut ctx = Self::default();

        // Check if directory is empty
        let is_empty = match fs::read_dir(root) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => true,
        };
        ctx.is_empty_dir = is_empty;
        ctx.is_git_repo = root.join(".git").exists();

        if is_empty {
            return ctx;
        }

        // Rust inspection
        if root.join("Cargo.toml").is_file() {
            ctx.manifest_file = Some("Cargo.toml".to_string());
            ctx.detected_languages.push("rust".to_string());
            ctx.detected_build_system = Some("cargo".to_string());
            ctx.detected_package_manager = Some("cargo".to_string());
            ctx.detected_test_command = Some("cargo test".to_string());

            // Check if CLI or Library
            if root.join("src").join("main.rs").is_file() {
                ctx.detected_type = Some(ProjectTypeChoice::Cli);
            } else {
                ctx.detected_type = Some(ProjectTypeChoice::Library);
            }

            // Check for common Rust frameworks
            if let Ok(cargo_content) = fs::read_to_string(root.join("Cargo.toml")) {
                if cargo_content.contains("axum") {
                    ctx.detected_framework = Some("axum".to_string());
                    ctx.detected_type = Some(ProjectTypeChoice::WebService);
                } else if cargo_content.contains("actix-web") {
                    ctx.detected_framework = Some("actix-web".to_string());
                    ctx.detected_type = Some(ProjectTypeChoice::WebService);
                } else if cargo_content.contains("ratatui") {
                    ctx.detected_type = Some(ProjectTypeChoice::Cli);
                }

                if cargo_content.contains("sqlx") || cargo_content.contains("rusqlite") {
                    ctx.detected_database = Some("sqlite".to_string());
                } else if cargo_content.contains("tokio-postgres")
                    || cargo_content.contains("diesel")
                {
                    ctx.detected_database = Some("postgres".to_string());
                }
            }
            return ctx;
        }

        // Node.js / JavaScript / TypeScript inspection
        if root.join("package.json").is_file() {
            ctx.manifest_file = Some("package.json".to_string());
            let is_ts = root.join("tsconfig.json").is_file();
            ctx.detected_languages.push(if is_ts {
                "typescript".to_string()
            } else {
                "javascript".to_string()
            });

            if root.join("pnpm-lock.yaml").is_file() {
                ctx.detected_package_manager = Some("pnpm".to_string());
            } else if root.join("yarn.lock").is_file() {
                ctx.detected_package_manager = Some("yarn".to_string());
            } else if root.join("bun.lockb").is_file() {
                ctx.detected_package_manager = Some("bun".to_string());
            } else {
                ctx.detected_package_manager = Some("npm".to_string());
            }

            ctx.detected_test_command = Some(format!(
                "{} test",
                ctx.detected_package_manager.as_deref().unwrap_or("npm")
            ));

            if let Ok(pkg_content) = fs::read_to_string(root.join("package.json")) {
                if pkg_content.contains("\"next\"") {
                    ctx.detected_framework = Some("nextjs".to_string());
                    ctx.detected_type = Some(ProjectTypeChoice::FullStackMonorepo);
                } else if pkg_content.contains("\"react\"") || pkg_content.contains("\"vue\"") {
                    ctx.detected_framework = Some("frontend".to_string());
                    ctx.detected_type = Some(ProjectTypeChoice::Frontend);
                } else if pkg_content.contains("\"express\"") || pkg_content.contains("\"fastify\"")
                {
                    ctx.detected_framework = Some("express".to_string());
                    ctx.detected_type = Some(ProjectTypeChoice::WebService);
                }
            }
            return ctx;
        }

        // Go inspection
        if root.join("go.mod").is_file() {
            ctx.manifest_file = Some("go.mod".to_string());
            ctx.detected_languages.push("go".to_string());
            ctx.detected_build_system = Some("go".to_string());
            ctx.detected_package_manager = Some("go".to_string());
            ctx.detected_test_command = Some("go test ./...".to_string());
            ctx.detected_type = Some(ProjectTypeChoice::WebService);
            return ctx;
        }

        // Python inspection
        if root.join("pyproject.toml").is_file()
            || root.join("setup.py").is_file()
            || root.join("requirements.txt").is_file()
        {
            ctx.manifest_file = Some(if root.join("pyproject.toml").is_file() {
                "pyproject.toml".to_string()
            } else if root.join("requirements.txt").is_file() {
                "requirements.txt".to_string()
            } else {
                "setup.py".to_string()
            });
            ctx.detected_languages.push("python".to_string());
            ctx.detected_package_manager = Some("pip".to_string());
            ctx.detected_test_command = Some("pytest".to_string());
            ctx.detected_type = Some(ProjectTypeChoice::WebService);
            return ctx;
        }

        ctx
    }
}

/// Setup screen field identifiers for keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupField {
    ProjectName,
    Purpose,
    ProjectType,
    Languages,
    TestCommand,
    GitIsolation,
    PolicyPosture,
    Database,
}

impl SetupField {
    pub const ALL: &'static [Self] = &[
        Self::ProjectName,
        Self::Purpose,
        Self::ProjectType,
        Self::Languages,
        Self::TestCommand,
        Self::GitIsolation,
        Self::PolicyPosture,
        Self::Database,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::ProjectName => "Project Name",
            Self::Purpose => "Purpose / Goal",
            Self::ProjectType => "Project Type",
            Self::Languages => "Primary Languages",
            Self::TestCommand => "Verification Command",
            Self::GitIsolation => "Git Isolation",
            Self::PolicyPosture => "Policy Posture",
            Self::Database => "Database / Storage",
        }
    }
}

/// Interactive Workspace Configuration Screen.
pub struct WorkspaceSetupScreen {
    pub workspace_root: PathBuf,
    pub detected: DetectedRepoContext,

    // Form inputs
    pub project_name_input: TextInput,
    pub purpose_input: TextInput,
    pub project_type: ProjectTypeChoice,
    pub languages_input: TextInput,
    pub test_command_input: TextInput,
    pub git_isolation: GitIsolationChoice,
    pub policy_posture: PolicyPostureChoice,
    pub database_input: TextInput,

    // UI State
    pub active_field_idx: usize,
    pub editing_text: bool,
    pub is_review_mode: bool,
    pub status_message: Option<String>,
    pub theme_tokens: ThemeTokens,
}

impl WorkspaceSetupScreen {
    /// Construct a new workspace setup screen for the given workspace path.
    pub fn new(workspace_root: PathBuf) -> Self {
        let detected = DetectedRepoContext::inspect(&workspace_root);
        let default_name = workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-project")
            .to_string();

        let mut project_name_input = TextInput::single_line();
        project_name_input.set_text(&default_name);

        let mut purpose_input = TextInput::single_line();
        purpose_input.set_text("Production-ready software project");

        let project_type = detected.detected_type.unwrap_or(ProjectTypeChoice::Cli);

        let mut languages_input = TextInput::single_line();
        if !detected.detected_languages.is_empty() {
            languages_input.set_text(&detected.detected_languages.join(", "));
        } else {
            languages_input.set_text("rust");
        }

        let mut test_command_input = TextInput::single_line();
        if let Some(ref cmd) = detected.detected_test_command {
            test_command_input.set_text(cmd);
        } else {
            test_command_input.set_text("cargo test");
        }

        let mut database_input = TextInput::single_line();
        if let Some(ref db) = detected.detected_database {
            database_input.set_text(db);
        } else {
            database_input.set_text("none");
        }

        let git_isolation = GitIsolationChoice::WorktreePerExecution;
        let policy_posture = PolicyPostureChoice::Balanced;

        let theme_tokens = ThemeTokens::resolve(ThemeMode::canonical_default());

        // Pre-fill existing configuration values if M31A.toml or legacy config exists
        let config_file = PlatformPaths::workspace_config_file(&workspace_root);
        let mut screen = Self {
            workspace_root,
            detected,
            project_name_input,
            purpose_input,
            project_type,
            languages_input,
            test_command_input,
            git_isolation,
            policy_posture,
            database_input,
            active_field_idx: 0,
            editing_text: false,
            is_review_mode: false,
            status_message: None,
            theme_tokens,
        };

        if config_file.is_file() {
            screen.load_from_existing(&config_file);
        }

        screen
    }

    /// Load existing settings into screen to support loss-free reconfiguration.
    fn load_from_existing(&mut self, path: &Path) {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(app_cfg) = parse_and_validate_config(&content) {
                if let Some(ref name) = app_cfg.workspace.project_name {
                    self.project_name_input.set_text(name);
                }
                if let Some(ref desc) = app_cfg.workspace.description {
                    self.purpose_input.set_text(desc);
                }
                if let Some(ref ptype) = app_cfg.workspace.project_type {
                    self.project_type = ProjectTypeChoice::from_str_loose(ptype);
                }
                if !app_cfg.workspace.languages.is_empty() {
                    self.languages_input
                        .set_text(&app_cfg.workspace.languages.join(", "));
                }
                if let Some(ref verif) = app_cfg.workspace.verification {
                    if let Some(ref cmd) = verif.tier3_tests {
                        self.test_command_input.set_text(cmd);
                    }
                }
                if let Some(ref db) = app_cfg.workspace.database {
                    self.database_input.set_text(db);
                }
                if let Some(ref iso) = app_cfg.workspace.git_isolation {
                    self.git_isolation = GitIsolationChoice::from_str_loose(iso);
                }
                if let Some(ref pol) = app_cfg.workspace.policy_posture {
                    self.policy_posture = PolicyPostureChoice::from_str_loose(pol);
                }
            }
        }
    }

    /// Active field identifier.
    pub fn active_field(&self) -> SetupField {
        SetupField::ALL[self.active_field_idx % SetupField::ALL.len()]
    }

    /// Active field index.
    pub fn active_field_idx(&self) -> usize {
        self.active_field_idx
    }

    /// Handle a keyboard event.
    pub fn handle_key(&mut self, key: KeyEvent) -> WorkspaceSetupOutcome {
        // Quick confirmation shortcut when defaults detected: Shift+Enter or Ctrl+Y
        if (key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y')) && !self.editing_text
        {
            if let Err(e) = self.persist_to_workspace() {
                self.status_message = Some(format!("Error saving configuration: {e}"));
                return WorkspaceSetupOutcome::Error(e);
            }
            return WorkspaceSetupOutcome::AcceptedDefaults;
        }

        match key.code {
            KeyCode::Esc => {
                if self.editing_text {
                    self.editing_text = false;
                    WorkspaceSetupOutcome::None
                } else {
                    WorkspaceSetupOutcome::Cancelled
                }
            }

            KeyCode::Up | KeyCode::BackTab if !self.editing_text => {
                if self.active_field_idx == 0 {
                    self.active_field_idx = SetupField::ALL.len() - 1;
                } else {
                    self.active_field_idx -= 1;
                }
                WorkspaceSetupOutcome::None
            }

            KeyCode::Down | KeyCode::Tab if !self.editing_text => {
                self.active_field_idx = (self.active_field_idx + 1) % SetupField::ALL.len();
                WorkspaceSetupOutcome::None
            }

            KeyCode::Left if !self.editing_text => {
                self.cycle_field_value(-1);
                WorkspaceSetupOutcome::None
            }

            KeyCode::Right if !self.editing_text => {
                self.cycle_field_value(1);
                WorkspaceSetupOutcome::None
            }

            KeyCode::Enter => {
                if self.editing_text {
                    self.editing_text = false;
                    WorkspaceSetupOutcome::None
                } else {
                    match self.active_field() {
                        SetupField::ProjectType
                        | SetupField::GitIsolation
                        | SetupField::PolicyPosture => {
                            self.cycle_field_value(1);
                            WorkspaceSetupOutcome::None
                        }
                        SetupField::ProjectName
                        | SetupField::Purpose
                        | SetupField::Languages
                        | SetupField::TestCommand
                        | SetupField::Database => {
                            self.editing_text = true;
                            WorkspaceSetupOutcome::None
                        }
                    }
                }
            }

            // Text input handling when editing
            _ if self.editing_text => {
                match self.active_field() {
                    SetupField::ProjectName => {
                        self.project_name_input.handle_key(key);
                    }
                    SetupField::Purpose => {
                        self.purpose_input.handle_key(key);
                    }
                    SetupField::Languages => {
                        self.languages_input.handle_key(key);
                    }
                    SetupField::TestCommand => {
                        self.test_command_input.handle_key(key);
                    }
                    SetupField::Database => {
                        self.database_input.handle_key(key);
                    }
                    _ => {}
                }
                WorkspaceSetupOutcome::None
            }

            // Final confirmation shortcut
            KeyCode::Char('s') | KeyCode::Char('S')
                if !self.editing_text && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Err(e) = self.persist_to_workspace() {
                    self.status_message = Some(format!("Error saving configuration: {e}"));
                    WorkspaceSetupOutcome::Error(e)
                } else {
                    WorkspaceSetupOutcome::Confirmed
                }
            }

            _ => WorkspaceSetupOutcome::None,
        }
    }

    /// Cycle through options for discrete enum fields.
    fn cycle_field_value(&mut self, direction: isize) {
        match self.active_field() {
            SetupField::ProjectType => {
                let all = ProjectTypeChoice::ALL;
                let cur = all
                    .iter()
                    .position(|x| *x == self.project_type)
                    .unwrap_or(0);
                let next = ((cur as isize + direction).rem_euclid(all.len() as isize)) as usize;
                self.project_type = all[next];
            }
            SetupField::GitIsolation => {
                let all = GitIsolationChoice::ALL;
                let cur = all
                    .iter()
                    .position(|x| *x == self.git_isolation)
                    .unwrap_or(0);
                let next = ((cur as isize + direction).rem_euclid(all.len() as isize)) as usize;
                self.git_isolation = all[next];
            }
            SetupField::PolicyPosture => {
                let all = PolicyPostureChoice::ALL;
                let cur = all
                    .iter()
                    .position(|x| *x == self.policy_posture)
                    .unwrap_or(0);
                let next = ((cur as isize + direction).rem_euclid(all.len() as isize)) as usize;
                self.policy_posture = all[next];
            }
            _ => {}
        }
    }

    /// Build the updated AppConfig preserving all unrelated sections.
    pub fn build_app_config(&self) -> AppConfig {
        let config_file = PlatformPaths::workspace_config_file(&self.workspace_root);
        let mut base_config = if config_file.is_file() {
            fs::read_to_string(&config_file)
                .ok()
                .and_then(|c| parse_and_validate_config(&c).ok())
                .unwrap_or_default()
        } else {
            AppConfig::default()
        };

        // Extract primary language tokens
        let raw_langs = self.languages_input.text().trim();
        let languages: Vec<String> = raw_langs
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let test_cmd = self.test_command_input.text().trim().to_string();
        let verification = if !test_cmd.is_empty() {
            Some(WorkspaceVerificationConfig {
                tier1_manifest: None,
                tier2_compiler: None,
                tier3_tests: Some(test_cmd),
                tier4_linter: None,
                timeout_secs: Some(300),
            })
        } else {
            None
        };

        // Set workspace fields
        base_config.workspace.project_name =
            Some(self.project_name_input.text().trim().to_string());
        base_config.workspace.description = Some(self.purpose_input.text().trim().to_string());
        base_config.workspace.project_type = Some(self.project_type.as_str().to_string());
        base_config.workspace.languages = languages;
        base_config.workspace.git_isolation = Some(self.git_isolation.as_str().to_string());
        base_config.workspace.policy_posture = Some(self.policy_posture.as_str().to_string());

        let db_str = self.database_input.text().trim().to_string();
        if !db_str.is_empty() && db_str != "none" {
            base_config.workspace.database = Some(db_str);
        }

        if let Some(verif) = verification {
            base_config.workspace.verification = Some(verif);
        }

        // Policy alignment
        match self.policy_posture {
            PolicyPostureChoice::Balanced => {
                base_config.policy.interactive_approvals = false;
            }
            PolicyPostureChoice::ReviewRequired => {
                base_config.policy.interactive_approvals = true;
            }
            PolicyPostureChoice::ReadOnlyAnalysis => {
                base_config.policy.interactive_approvals = true;
                base_config.policy.denied_tools.push("fs.write".to_string());
                base_config
                    .policy
                    .denied_tools
                    .push("process.execute".to_string());
            }
            PolicyPostureChoice::LocalChangesOnly => {
                base_config.git.push_policy = GitPushPolicy::Deny;
            }
        }

        base_config
    }

    /// Atomically persist configuration to `.m31a/config.md` and mark workspace initialized.
    pub fn persist_to_workspace(&self) -> Result<PathBuf, String> {
        let app_config = self.build_app_config();
        validate_config(&app_config).map_err(|e| format!("Validation error: {e}"))?;

        let canonical_file = PlatformPaths::canonical_workspace_config_file(&self.workspace_root);

        let existing_body = if canonical_file.exists() {
            let content = fs::read_to_string(&canonical_file).map_err(|e| {
                format!(
                    "Failed to read existing configuration at '{}': {e}",
                    canonical_file.display()
                )
            })?;
            let (_, raw_body) = crate::document::split_frontmatter(&content).map_err(|e| {
                format!(
                    "Existing configuration at '{}' has corrupt front matter: {e}",
                    canonical_file.display()
                )
            })?;
            if !raw_body.trim().is_empty() {
                Some(raw_body.to_string())
            } else {
                None
            }
        } else {
            None
        };

        let body = existing_body.unwrap_or_else(|| {
            "# M31A Workspace Configuration\n\nAuthoritative workspace configuration for M31A.\nConfiguration parameters are defined in the YAML front matter above.\n".to_string()
        });

        let doc = crate::document::MarkdownDocument {
            kind: crate::document::DocumentKind::Config,
            schema_version: 1,
            data: app_config,
            body,
        };

        let md_content = crate::document::serialize_markdown_document(&doc)
            .map_err(|e| format!("Serialization error: {e}"))?;

        crate::document::atomic_write_document(&canonical_file, &md_content)
            .map_err(|e| format!("Atomic write failed: {e}"))?;

        // Ensure init sentinel exists so workspace is marked initialized
        let sentinel_path = self.workspace_root.join(".m31a").join("init.json");
        if !sentinel_path.exists() {
            let sentinel_json = serde_json::json!({
                "schema_version": 1,
                "initialized_at": chrono::Utc::now().to_rfc3339(),
                "workspace_root": self.workspace_root.display().to_string(),
                "canonical_config": ".m31a/config.md"
            });
            let _ = fs::write(
                &sentinel_path,
                serde_json::to_string_pretty(&sentinel_json).unwrap_or_default(),
            );
        }

        Ok(canonical_file)
    }

    /// Render the screen to the terminal frame.
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme_tokens;

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(tokens.focus)
            .title(Span::styled(
                " M31A · Workspace Configuration ",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ));

        let inner_area = outer_block.inner(area);
        f.render_widget(outer_block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(4), // Header / Detection banner
                Constraint::Min(14),   // Form fields
                Constraint::Length(3), // Footer / Hotkeys
            ])
            .split(inner_area);

        // Header
        let detect_str = if let Some(ref m) = self.detected.manifest_file {
            format!(
                "Detected existing repository manifest: {} · Sensible defaults pre-filled.",
                m
            )
        } else if self.detected.is_empty_dir {
            "New empty workspace · Scaffolding initial configuration.".to_string()
        } else {
            "Existing directory without detected manifests · Documented safe defaults applied."
                .to_string()
        };

        let header_lines = vec![
            Line::from(vec![
                Span::styled("Workspace: ", tokens.text_secondary),
                Span::styled(
                    self.workspace_root.display().to_string(),
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Status:    ", tokens.text_secondary),
                Span::styled(detect_str, tokens.success),
            ]),
        ];
        f.render_widget(Paragraph::new(header_lines), chunks[0]);

        // Form Fields
        let mut field_lines = Vec::new();
        field_lines.push(Line::from(vec![Span::styled(
            "Project Context & Boundaries:",
            tokens.focus.add_modifier(Modifier::BOLD),
        )]));
        field_lines.push(Line::from(""));

        for (idx, field) in SetupField::ALL.iter().enumerate() {
            let is_active = idx == (self.active_field_idx % SetupField::ALL.len());
            let cursor = if is_active { " ▶ " } else { "   " };
            let label_style = if is_active {
                tokens.focus.add_modifier(Modifier::BOLD)
            } else {
                tokens.text_secondary
            };

            let value_span = match field {
                SetupField::ProjectName => {
                    let text = if self.editing_text && is_active {
                        format!("{}█", self.project_name_input.text())
                    } else {
                        self.project_name_input.text().to_string()
                    };
                    Span::styled(text, tokens.text_primary)
                }
                SetupField::Purpose => {
                    let text = if self.editing_text && is_active {
                        format!("{}█", self.purpose_input.text())
                    } else {
                        self.purpose_input.text().to_string()
                    };
                    Span::styled(text, tokens.text_primary)
                }
                SetupField::ProjectType => Span::styled(
                    format!("[ ◀ {} ▶ ]", self.project_type.display_label()),
                    tokens.focus,
                ),
                SetupField::Languages => {
                    let text = if self.editing_text && is_active {
                        format!("{}█", self.languages_input.text())
                    } else {
                        self.languages_input.text().to_string()
                    };
                    Span::styled(text, tokens.text_primary)
                }
                SetupField::TestCommand => {
                    let text = if self.editing_text && is_active {
                        format!("{}█", self.test_command_input.text())
                    } else {
                        self.test_command_input.text().to_string()
                    };
                    Span::styled(text, tokens.text_primary)
                }
                SetupField::GitIsolation => Span::styled(
                    format!("[ ◀ {} ▶ ]", self.git_isolation.display_label()),
                    tokens.focus,
                ),
                SetupField::PolicyPosture => Span::styled(
                    format!("[ ◀ {} ▶ ]", self.policy_posture.display_label()),
                    tokens.focus,
                ),
                SetupField::Database => {
                    let text = if self.editing_text && is_active {
                        format!("{}█", self.database_input.text())
                    } else {
                        self.database_input.text().to_string()
                    };
                    Span::styled(text, tokens.text_primary)
                }
            };

            field_lines.push(Line::from(vec![
                Span::styled(cursor, tokens.focus),
                Span::styled(format!("{:<22}", field.title()), label_style),
                value_span,
            ]));
        }

        f.render_widget(
            Paragraph::new(field_lines).wrap(Wrap { trim: false }),
            chunks[1],
        );

        // Footer / Status
        let footer_text = if let Some(ref msg) = self.status_message {
            vec![Span::styled(msg, tokens.error.add_modifier(Modifier::BOLD))]
        } else if self.editing_text {
            vec![
                Span::styled(" [Editing] ", tokens.focus.add_modifier(Modifier::BOLD)),
                Span::styled(
                    "Type to update · Enter to confirm · Esc to revert",
                    tokens.text_muted,
                ),
            ]
        } else {
            vec![
                Span::styled(" [Y] ", tokens.success.add_modifier(Modifier::BOLD)),
                Span::styled("Accept Detected Defaults  |  ", tokens.text_secondary),
                Span::styled("[↑/↓] ", tokens.focus.add_modifier(Modifier::BOLD)),
                Span::styled("Select  |  ", tokens.text_secondary),
                Span::styled("[←/→/Enter] ", tokens.focus.add_modifier(Modifier::BOLD)),
                Span::styled("Change/Edit  |  ", tokens.text_secondary),
                Span::styled("[Ctrl+S] ", tokens.success.add_modifier(Modifier::BOLD)),
                Span::styled("Save .m31a/M31A.toml  |  ", tokens.text_secondary),
                Span::styled("[Esc] ", tokens.text_muted.add_modifier(Modifier::BOLD)),
                Span::styled("Cancel", tokens.text_muted),
            ]
        };

        let footer_paragraph = Paragraph::new(Line::from(footer_text))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(tokens.border_default),
            );
        f.render_widget(footer_paragraph, chunks[2]);
    }
}

/// Headless / non-interactive workspace initialization helper.
pub fn initialize_workspace_headless(
    workspace_root: &Path,
    project_name: Option<String>,
    project_type: Option<String>,
    languages: Option<Vec<String>>,
    test_command: Option<String>,
) -> Result<PathBuf, String> {
    let detected = DetectedRepoContext::inspect(workspace_root);
    let mut config = AppConfig::default();

    let name = project_name.unwrap_or_else(|| {
        workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string()
    });
    config.workspace.project_name = Some(name);
    config.workspace.description = Some("Autonomous software engineering project".to_string());

    let ptype = project_type.unwrap_or_else(|| {
        detected
            .detected_type
            .unwrap_or(ProjectTypeChoice::Cli)
            .as_str()
            .to_string()
    });
    config.workspace.project_type = Some(ptype);

    let langs = languages.unwrap_or_else(|| {
        if !detected.detected_languages.is_empty() {
            detected.detected_languages.clone()
        } else {
            vec!["rust".to_string()]
        }
    });
    config.workspace.languages = langs;

    let test_cmd = test_command.or(detected.detected_test_command);
    if let Some(cmd) = test_cmd {
        config.workspace.verification = Some(WorkspaceVerificationConfig {
            tier1_manifest: None,
            tier2_compiler: None,
            tier3_tests: Some(cmd),
            tier4_linter: None,
            timeout_secs: Some(300),
        });
    }

    config.workspace.git_isolation = Some("worktree_per_execution".to_string());
    config.workspace.policy_posture = Some("balanced".to_string());

    validate_config(&config).map_err(|e| format!("Validation error: {e}"))?;

    let canonical_file = PlatformPaths::canonical_workspace_config_file(workspace_root);

    let doc = crate::document::MarkdownDocument {
        kind: crate::document::DocumentKind::Config,
        schema_version: 1,
        data: config,
        body: "# M31A Workspace Configuration\n\nAuthoritative workspace configuration for M31A.\nConfiguration parameters are defined in the YAML front matter above.\n".to_string(),
    };

    let md_content = crate::document::serialize_markdown_document(&doc)
        .map_err(|e| format!("Serialization error: {e}"))?;

    crate::document::atomic_write_document(&canonical_file, &md_content)
        .map_err(|e| format!("Atomic write failed: {e}"))?;

    // Record sentinel
    let sentinel_path = workspace_root.join(".m31a").join("init.json");
    let sentinel_json = serde_json::json!({
        "schema_version": 1,
        "initialized_at": chrono::Utc::now().to_rfc3339(),
        "workspace_root": workspace_root.display().to_string(),
        "canonical_config": ".m31a/config.md"
    });
    let _ = fs::write(
        &sentinel_path,
        serde_json::to_string_pretty(&sentinel_json).unwrap_or_default(),
    );

    Ok(canonical_file)
}
