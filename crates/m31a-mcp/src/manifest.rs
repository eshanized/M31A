//! Application package manifest and security boundaries.

use serde::{Deserialize, Serialize};

/// Application package manifest defining exposed tools, resources, and required permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAppManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,

    pub tools: Vec<CodexAppTool>,
    pub resources: Vec<CodexAppResource>,
    pub prompts: Vec<CodexAppPrompt>,

    pub permissions: CodexAppPermissions,
    pub sandbox: Option<String>,
}

/// Application tool definition specifying invocation schema and approval policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAppTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub requires_approval: bool,
    pub sandbox_policy: Option<String>,
}

/// Application resource descriptor specifying URI scheme and MIME type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAppResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: Option<String>,
}

/// Application prompt template definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAppPrompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<CodexAppPromptArgument>,
}

/// Application prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAppPromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub schema: serde_json::Value,
}

/// Application permission boundaries governing file access, networking, and process execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexAppPermissions {
    pub file_read: Vec<String>,
    pub file_write: Vec<String>,
    pub network: Vec<String>,
    pub process_exec: Vec<String>,
    pub environment_vars: Vec<String>,
}
