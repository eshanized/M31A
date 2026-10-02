//! Structured policy matching for tools, paths, arguments, roles, and autonomy modes (POL-01, D-03).

use regex::Regex;
use std::path::{Path, PathBuf};

use crate::kernel::seams::policy::{PolicyError, PolicyEvaluationRequest};
use crate::policy::rule::PolicyRule;
use crate::state::intake::AutonomyMode;
use crate::state_machine::agent::AgentRole;

/// Strongly typed evaluation context passed to policy evaluation.
#[derive(Debug, Clone)]
pub struct PolicyEvaluationContext {
    pub tool_id: String,
    pub target_paths: Vec<PathBuf>,
    pub args: serde_json::Value,
    pub role: Option<AgentRole>,
    pub mode: AutonomyMode,
    pub workspace_root: PathBuf,
}

impl PolicyEvaluationContext {
    pub fn new(tool_id: impl Into<String>, workspace_root: PathBuf) -> Self {
        Self {
            tool_id: tool_id.into(),
            target_paths: Vec::new(),
            args: serde_json::Value::Null,
            role: None,
            mode: AutonomyMode::Autonomous,
            workspace_root,
        }
    }

    pub fn with_target_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.target_paths = paths;
        self
    }

    pub fn with_args(mut self, args: serde_json::Value) -> Self {
        self.args = args;
        self
    }

    pub fn with_role(mut self, role: AgentRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn with_mode(mut self, mode: AutonomyMode) -> Self {
        self.mode = mode;
        self
    }

    /// Construct a basic evaluation context from a pipeline PolicyEvaluationRequest.
    pub fn from_request(req: &PolicyEvaluationRequest) -> Self {
        let mut target_paths = Vec::new();
        let mut args = serde_json::Value::Null;
        let mut workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Parse optional "ws=<path>;" prefix from context_digest
        if let Some(ws_start) = req.context_digest.find("ws=") {
            let remainder = &req.context_digest[ws_start + 3..];
            if let Some(ws_end) = remainder.find(';') {
                let ws_str = &remainder[..ws_end];
                workspace_root = PathBuf::from(ws_str);
            }
        }

        // Parse optional ";args=" suffix from context_digest
        if let Some(idx) = req.context_digest.find(";args=") {
            let args_str = &req.context_digest[idx + 6..];
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_str) {
                if let Some(path_str) = val.get("path").and_then(|p| p.as_str()) {
                    target_paths.push(PathBuf::from(path_str));
                }
                if let Some(path_str) = val.get("target_path").and_then(|p| p.as_str()) {
                    target_paths.push(PathBuf::from(path_str));
                }
                if let Some(path_str) = val.get("file_path").and_then(|p| p.as_str()) {
                    target_paths.push(PathBuf::from(path_str));
                }
                if let Some(path_str) = val.get("destination_path").and_then(|p| p.as_str()) {
                    target_paths.push(PathBuf::from(path_str));
                }
                if let Some(path_str) = val.get("file").and_then(|p| p.as_str()) {
                    target_paths.push(PathBuf::from(path_str));
                }
                if let Some(path_str) = val.get("dir").and_then(|p| p.as_str()) {
                    target_paths.push(PathBuf::from(path_str));
                }
                if let Some(paths_arr) = val.get("paths").and_then(|p| p.as_array()) {
                    for item in paths_arr {
                        if let Some(p_str) = item.as_str() {
                            target_paths.push(PathBuf::from(p_str));
                        }
                    }
                }
                if let Some(cmd_str) = val.get("command").and_then(|c| c.as_str()) {
                    for token in cmd_str.split_whitespace() {
                        let clean = token.trim_matches(|c| {
                            c == '\'' || c == '"' || c == ';' || c == '&' || c == '|' || c == '`'
                        });
                        if crate::capability::providers::local_fs::contains_protected_component(
                            Path::new(clean),
                        ) {
                            target_paths.push(PathBuf::from(clean));
                        }
                    }
                }
                if let Some(args_arr) = val.get("args").and_then(|a| a.as_array()) {
                    for item in args_arr {
                        if let Some(s) = item.as_str() {
                            for token in s.split_whitespace() {
                                let clean = token.trim_matches(|c| {
                                    c == '\''
                                        || c == '"'
                                        || c == ';'
                                        || c == '&'
                                        || c == '|'
                                        || c == '`'
                                });
                                if crate::capability::providers::local_fs::contains_protected_component(Path::new(clean)) {
                                    target_paths.push(PathBuf::from(clean));
                                }
                            }
                        }
                    }
                }
                args = val;
            }
        }

        Self {
            tool_id: req.tool_or_action.clone(),
            target_paths,
            args,
            role: None,
            mode: AutonomyMode::Autonomous,
            workspace_root,
        }
    }
}

/// Lexically normalize a path resolving '.' and '..' components without filesystem access.
pub fn lexical_normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut parts = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    parts.iter().collect()
}

/// Resolves path relative to workspace root and verifies it does not escape via traversal or symlinks.
pub fn canonicalize_and_validate_path(
    path: &Path,
    workspace_root: &Path,
) -> Result<PathBuf, PolicyError> {
    let canon_ws = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| lexical_normalize(workspace_root));

    let full_path = if path.is_relative() {
        canon_ws.join(path)
    } else {
        path.to_path_buf()
    };

    let normalized = lexical_normalize(&full_path);

    if !normalized.starts_with(&canon_ws) {
        return Err(PolicyError::EvaluationFailed(format!(
            "path '{}' escapes workspace root '{}'",
            path.display(),
            canon_ws.display()
        )));
    }

    if normalized.exists() {
        let canon = normalized.canonicalize().map_err(|e| {
            PolicyError::EvaluationFailed(format!(
                "failed to canonicalize path '{}': {e}",
                path.display()
            ))
        })?;
        if !canon.starts_with(&canon_ws) {
            return Err(PolicyError::EvaluationFailed(format!(
                "path '{}' symlink escapes workspace root '{}'",
                path.display(),
                canon_ws.display()
            )));
        }
        Ok(canon)
    } else {
        let mut ancestor = normalized.clone();
        while !ancestor.exists() {
            if let Some(parent) = ancestor.parent() {
                ancestor = parent.to_path_buf();
            } else {
                break;
            }
        }
        if ancestor.exists() {
            let canon_ancestor = ancestor.canonicalize().map_err(|e| {
                PolicyError::EvaluationFailed(format!(
                    "failed to canonicalize ancestor '{}': {e}",
                    ancestor.display()
                ))
            })?;
            if !canon_ancestor.starts_with(&canon_ws) {
                return Err(PolicyError::EvaluationFailed(format!(
                    "ancestor of path '{}' escapes workspace root '{}'",
                    path.display(),
                    canon_ws.display()
                )));
            }
        }
        Ok(normalized)
    }
}

/// Convert a glob pattern string to an equivalent regular expression pattern.
pub fn glob_to_regex(glob: &str, is_path: bool) -> String {
    let mut regex = String::from("^");
    let mut chars = glob.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if is_path && chars.peek() == Some(&'*') {
                    chars.next(); // consume second '*'
                    if chars.peek() == Some(&'/') {
                        chars.next(); // consume '/'
                        regex.push_str("(?:.*/)?");
                    } else {
                        regex.push_str(".*");
                    }
                } else if is_path {
                    regex.push_str("[^/]*");
                } else {
                    regex.push_str(".*");
                }
            }
            '?' => {
                if is_path {
                    regex.push_str("[^/]");
                } else {
                    regex.push('.');
                }
            }
            '.' | '(' | ')' | '+' | '|' | '^' | '$' | '@' | '%' | '{' | '}' | '[' | ']' | '\\' => {
                regex.push('\\');
                regex.push(c);
            }
            _ => {
                regex.push(c);
            }
        }
    }
    regex.push('$');
    regex
}

const MAX_REGEX_CACHE_ENTRIES: usize = 1024;

static TOOL_REGEX_CACHE: std::sync::LazyLock<
    std::sync::RwLock<std::collections::HashMap<String, Option<Regex>>>,
> = std::sync::LazyLock::new(|| std::sync::RwLock::new(std::collections::HashMap::new()));

static PATH_REGEX_CACHE: std::sync::LazyLock<
    std::sync::RwLock<std::collections::HashMap<String, Option<Regex>>>,
> = std::sync::LazyLock::new(|| std::sync::RwLock::new(std::collections::HashMap::new()));

fn get_or_compile_tool_regex(pattern: &str) -> Option<Regex> {
    if let Ok(cache) = TOOL_REGEX_CACHE.read() {
        if let Some(opt_re) = cache.get(pattern) {
            return opt_re.clone();
        }
    }

    let regex_str = glob_to_regex(pattern, false);
    let compiled = Regex::new(&regex_str).ok();

    if let Ok(mut cache) = TOOL_REGEX_CACHE.write() {
        if cache.len() >= MAX_REGEX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(pattern.to_string(), compiled.clone());
    }

    compiled
}

fn get_or_compile_path_regex(pattern: &str) -> Option<Regex> {
    if let Ok(cache) = PATH_REGEX_CACHE.read() {
        if let Some(opt_re) = cache.get(pattern) {
            return opt_re.clone();
        }
    }

    let regex_str = glob_to_regex(pattern, true);
    let compiled = Regex::new(&regex_str).ok();

    if let Ok(mut cache) = PATH_REGEX_CACHE.write() {
        if cache.len() >= MAX_REGEX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(pattern.to_string(), compiled.clone());
    }

    compiled
}

/// Policy evaluation matching engine.
pub struct PolicyMatcher;

impl PolicyMatcher {
    /// Evaluates if a tool identifier matches a pattern.
    /// Supports exact matches (`write_file`), prefixes (`git_*`, `fs:*`), or catch-all (`*`).
    pub fn matches_tool(pattern: &str, tool_id: &str) -> bool {
        if pattern == "*" || pattern == tool_id {
            return true;
        }

        get_or_compile_tool_regex(pattern).is_some_and(|re| re.is_match(tool_id))
    }

    /// Evaluates if a target path matches a pattern relative to workspace root or globally.
    pub fn matches_path(pattern: &str, target_path: &Path, workspace_root: &Path) -> bool {
        let is_workspace_relative =
            pattern.starts_with("./") || (!pattern.starts_with('/') && !pattern.starts_with("**"));

        if is_workspace_relative {
            // Target MUST reside within workspace root without escaping
            match canonicalize_and_validate_path(target_path, workspace_root) {
                Ok(valid_path) => {
                    let canon_ws = workspace_root
                        .canonicalize()
                        .unwrap_or_else(|_| lexical_normalize(workspace_root));
                    if let Ok(rel) = valid_path.strip_prefix(&canon_ws) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        let pat_clean = pattern.trim_start_matches("./");
                        if pat_clean == "**" || pat_clean == "*" {
                            return true;
                        }
                        if let Some(re) = get_or_compile_path_regex(pat_clean) {
                            return re.is_match(&rel_str);
                        }
                    }
                    false
                }
                Err(_) => false, // Path traversal or symlink escape fails closed immediately
            }
        } else {
            // Absolute or sensitive global pattern (e.g. **/.ssh/**, /etc/**, **/.env*, /etc/sudoers*)
            let norm_target = lexical_normalize(target_path);
            let target_str = norm_target.to_string_lossy().replace('\\', "/");

            let canon_str = target_path
                .canonicalize()
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"));

            if let Some(re) = get_or_compile_path_regex(pattern) {
                if re.is_match(&target_str) {
                    return true;
                }
                if canon_str.as_ref().is_some_and(|cs| re.is_match(cs)) {
                    return true;
                }

                // Check ancestors: if an ancestor directory matches pattern, any child path is matched
                let mut current = norm_target.parent();
                while let Some(p) = current {
                    let p_str = p.to_string_lossy().replace('\\', "/");
                    if re.is_match(&p_str) {
                        return true;
                    }
                    current = p.parent();
                }
            }
            false
        }
    }

    /// Evaluates typed argument predicates against actual tool invocation arguments.
    pub fn matches_args(
        pattern_args: Option<&serde_json::Value>,
        actual_args: &serde_json::Value,
    ) -> bool {
        let pattern = match pattern_args {
            None => return true,
            Some(serde_json::Value::Null) => return true,
            Some(p) => p,
        };

        match (pattern, actual_args) {
            (serde_json::Value::Object(pat_map), serde_json::Value::Object(act_map)) => {
                for (key, exp_val) in pat_map {
                    let act_val = match act_map.get(key) {
                        Some(v) => v,
                        None => return false,
                    };

                    if !Self::matches_single_arg(exp_val, act_val) {
                        return false;
                    }
                }
                true
            }
            (serde_json::Value::Null, serde_json::Value::Null) => true,
            (a, b) => a == b,
        }
    }

    fn matches_single_arg(expected: &serde_json::Value, actual: &serde_json::Value) -> bool {
        match (expected, actual) {
            (serde_json::Value::String(exp_str), serde_json::Value::String(act_str)) => {
                Self::matches_wildcard(exp_str, act_str)
            }
            (serde_json::Value::String(exp_str), serde_json::Value::Array(act_arr)) => {
                // If actual is an array (e.g. argv), check if any element matches or joined matches
                act_arr.iter().any(|item| {
                    item.as_str()
                        .is_some_and(|s| Self::matches_wildcard(exp_str, s))
                }) || {
                    let joined = act_arr
                        .iter()
                        .filter_map(|i| i.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    Self::matches_wildcard(exp_str, &joined)
                }
            }
            (serde_json::Value::Array(exp_arr), actual_val) => {
                // Any pattern in expected array matching satisfies
                exp_arr
                    .iter()
                    .any(|exp_item| Self::matches_single_arg(exp_item, actual_val))
            }
            (serde_json::Value::Object(exp_obj), serde_json::Value::Object(act_obj)) => {
                for (k, v) in exp_obj {
                    if let Some(act_v) = act_obj.get(k) {
                        if !Self::matches_single_arg(v, act_v) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (a, b) => a == b,
        }
    }

    fn matches_wildcard(pattern: &str, text: &str) -> bool {
        Self::matches_tool(pattern, text)
    }

    /// Evaluates agent role and autonomy mode match.
    pub fn matches_role_and_mode(
        rule: &PolicyRule,
        role: Option<AgentRole>,
        mode: AutonomyMode,
    ) -> bool {
        if !rule.roles.is_empty() {
            let role_matches = match role {
                Some(r) => rule.roles.contains(&r),
                None => false,
            };
            if !role_matches {
                return false;
            }
        }

        if !rule.modes.is_empty() && !rule.modes.contains(&mode) {
            return false;
        }

        true
    }

    /// Complete rule match against an evaluation context.
    pub fn matches_rule(rule: &PolicyRule, ctx: &PolicyEvaluationContext) -> bool {
        // Tool check
        if !rule.tools.is_empty()
            && !rule
                .tools
                .iter()
                .any(|t| Self::matches_tool(t, &ctx.tool_id))
        {
            return false;
        }

        // Role & mode check
        if !Self::matches_role_and_mode(rule, ctx.role.clone(), ctx.mode) {
            return false;
        }

        // Args check
        if !Self::matches_args(rule.args.as_ref(), &ctx.args) {
            return false;
        }

        // Paths check
        if !rule.paths.is_empty() {
            if ctx.target_paths.is_empty() {
                return false;
            }
            let path_matches = ctx.target_paths.iter().any(|target| {
                rule.paths
                    .iter()
                    .any(|p| Self::matches_path(p, target, &ctx.workspace_root))
            });
            if !path_matches {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_matches_tool() {
        let patterns = ["git_*", "fs:*", "shell:*", "write_file", "edit_*"];
        let tool_ids = [
            "git_commit",
            "fs:read",
            "shell:run",
            "write_file",
            "edit_file",
        ];

        let start = Instant::now();
        let iterations = 1_000;
        for i in 0..iterations {
            let pattern = patterns[i % patterns.len()];
            let tool_id = tool_ids[i % tool_ids.len()];
            let _ = PolicyMatcher::matches_tool(pattern, tool_id);
        }
        let elapsed = start.elapsed();
        println!("BASELINE_BENCHMARK: 1,000 iterations took {:?}", elapsed);
    }

    #[test]
    fn test_matches_tool_functional() {
        assert!(PolicyMatcher::matches_tool("git_*", "git_commit"));
        assert!(PolicyMatcher::matches_tool("git_*", "git_push"));
        assert!(!PolicyMatcher::matches_tool("git_*", "shell_run"));

        assert!(PolicyMatcher::matches_tool("*", "anything"));
        assert!(PolicyMatcher::matches_tool("write_file", "write_file"));
        assert!(!PolicyMatcher::matches_tool("write_file", "read_file"));

        assert!(PolicyMatcher::matches_tool("fs:*", "fs:read"));
        assert!(PolicyMatcher::matches_tool("fs:*", "fs:write"));
        assert!(!PolicyMatcher::matches_tool("fs:*", "net:connect"));
    }

    #[test]
    fn test_matches_path_cached() {
        let root = std::env::current_dir().unwrap();
        let path = root.join("src/lib.rs");

        assert!(PolicyMatcher::matches_path("src/*.rs", &path, &root));
        assert!(PolicyMatcher::matches_path("**/*.rs", &path, &root));
        assert!(!PolicyMatcher::matches_path("src/*.py", &path, &root));
    }

    #[test]
    fn test_concurrent_tool_matching() {
        let mut handles = vec![];
        for _ in 0..10 {
            let handle = std::thread::spawn(|| {
                for _ in 0..1_000 {
                    assert!(PolicyMatcher::matches_tool("git_*", "git_status"));
                    assert!(!PolicyMatcher::matches_tool("git_*", "fs_read"));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_regex_cache_capacity_eviction() {
        for i in 0..1200 {
            let pat = format!("tool_pattern_{}*", i);
            assert!(PolicyMatcher::matches_tool(
                &pat,
                &format!("tool_pattern_{}_run", i)
            ));
        }
    }
}
