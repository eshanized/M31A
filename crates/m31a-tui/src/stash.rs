//! Prompt draft stash management and JSONL persistence.
//!
//! Enables developers to stash half-typed prompts, switch tasks, and restore them.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Stashed prompt draft record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptStashEntry {
    pub id: String,
    pub timestamp: String,
    pub prompt: String,
}

pub struct PromptStash;

impl PromptStash {
    pub fn stash_file_path() -> PathBuf {
        let dir = m31a_core::Config::config_dir().join("stash");
        let _ = create_dir_all(&dir);
        dir.join("prompt-stash.jsonl")
    }

    /// Saves a prompt to the stash file, returning the new count of stashed items.
    pub fn save(prompt: &str) -> std::io::Result<usize> {
        let path = Self::stash_file_path();
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        let entry = PromptStashEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            prompt: prompt.to_string(),
        };

        let json = serde_json::to_string(&entry)?;
        writeln!(file, "{}", json)?;

        Self::count()
    }

    /// Pops and removes the most recently stashed prompt.
    pub fn pop() -> std::io::Result<Option<String>> {
        let mut entries = Self::list()?;
        if let Some(last) = entries.pop() {
            Self::write_all(&entries)?;
            Ok(Some(last.prompt))
        } else {
            Ok(None)
        }
    }

    /// Lists all stashed prompts from oldest to newest.
    pub fn list() -> std::io::Result<Vec<PromptStashEntry>> {
        let path = Self::stash_file_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let l = line?;
            if let Ok(entry) = serde_json::from_str::<PromptStashEntry>(&l) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Clears all stashed prompts.
    pub fn clear() -> std::io::Result<()> {
        let path = Self::stash_file_path();
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    /// Counts how many prompts are currently stashed.
    pub fn count() -> std::io::Result<usize> {
        Ok(Self::list()?.len())
    }

    fn write_all(entries: &[PromptStashEntry]) -> std::io::Result<()> {
        let path = Self::stash_file_path();
        let mut file = File::create(&path)?;
        for entry in entries {
            let json = serde_json::to_string(entry)?;
            writeln!(file, "{}", json)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_stash_roundtrip() {
        let stash_file =
            std::env::temp_dir().join(format!("test-stash-{}.jsonl", uuid::Uuid::new_v4()));

        let entry = PromptStashEntry {
            id: "1".into(),
            timestamp: "2026-10-31 01:00:00".into(),
            prompt: "Refactor virtualizer and add stash".into(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        std::fs::write(&stash_file, format!("{}\n", json)).unwrap();

        let read_entry: PromptStashEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(read_entry.prompt, "Refactor virtualizer and add stash");
        assert_eq!(read_entry.id, "1");

        let _ = std::fs::remove_file(stash_file);
    }
}
