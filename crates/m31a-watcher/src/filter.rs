//! Filesystem event filtering and noise suppression rules.
//!
//! Filters out noise from build directories, lockfiles, temporary files, and hidden metadata paths.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct WatcherFilter {
    ignored_dir_names: Vec<String>,
    ignored_extensions: Vec<String>,
}

impl Default for WatcherFilter {
    fn default() -> Self {
        Self {
            ignored_dir_names: vec![
                ".git".into(),
                "target".into(),
                "node_modules".into(),
                ".m31a".into(),
                "dist".into(),
                "build".into(),
                "out".into(),
                ".cache".into(),
                "__pycache__".into(),
                ".pytest_cache".into(),
                ".venv".into(),
                "venv".into(),
            ],
            ignored_extensions: vec![
                "swp".into(),
                "tmp".into(),
                "bak".into(),
                "lock".into(),
                "pyc".into(),
            ],
        }
    }
}

impl WatcherFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_additional_ignore(mut self, dir_name: &str) -> Self {
        self.ignored_dir_names.push(dir_name.to_string());
        self
    }

    pub fn should_ignore_dir(&self, name: &str) -> bool {
        self.ignored_dir_names.iter().any(|d| d == name)
    }

    pub fn should_ignore_file(&self, path: &Path) -> bool {
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.') && file_name != ".env" {
                return true;
            }
            if file_name.ends_with('~') {
                return true;
            }
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_ascii_lowercase();
            if self.ignored_extensions.iter().any(|e| e == &ext_lower) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_filter() {
        let filter = WatcherFilter::default();

        assert!(filter.should_ignore_dir("target"));
        assert!(filter.should_ignore_dir("node_modules"));
        assert!(!filter.should_ignore_dir("src"));

        assert!(filter.should_ignore_file(Path::new("test.swp")));
        assert!(filter.should_ignore_file(Path::new(".hidden")));
        assert!(!filter.should_ignore_file(Path::new("main.rs")));
        assert!(!filter.should_ignore_file(Path::new("app.tsx")));
    }
}
