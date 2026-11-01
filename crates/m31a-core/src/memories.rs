//! Long-term memory storage, semantic indexing, and knowledge consolidation.
//!
//! Manages persistent memory records across sessions, indexing insights and project guidelines.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use uuid::Uuid;

static MEMORY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Advisory cross-process file lock guard for memory operations.
struct FileLock {
    #[cfg(unix)]
    fd: std::os::unix::io::RawFd,
    _file: fs::File,
}

impl FileLock {
    fn acquire(lock_path: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let ret = unsafe { libc::flock(fd, libc::LOCK_EX) };
            if ret != 0 {
                return Err(anyhow::anyhow!(
                    "Failed to acquire file lock: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(Self { fd, _file: file })
        }

        #[cfg(not(unix))]
        {
            Ok(Self { _file: file })
        }
    }
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.fd, libc::LOCK_UN);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryItem {
    pub id: String,
    pub topic: String,
    pub detail: String,
    pub citation_count: usize,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub thread_id: String,
    pub title: String,
    pub raw_memory: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub created_at: i64,
}

pub struct MemoryManager {
    memories_dir: PathBuf,
}

impl MemoryManager {
    pub fn default_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".m31a").join("memories")
    }

    pub fn new() -> Result<Self> {
        let dir = Self::default_dir();
        fs::create_dir_all(&dir)?;
        fs::create_dir_all(dir.join("rollout_summaries"))?;
        Ok(Self { memories_dir: dir })
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::create_dir_all(dir.join("rollout_summaries"));
        Self { memories_dir: dir }
    }

    fn items_file(&self) -> PathBuf {
        self.memories_dir.join("memories.json")
    }

    fn lock_file(&self) -> PathBuf {
        self.memories_dir.join(".memories.lock")
    }

    pub fn list_memory_items(&self) -> Vec<MemoryItem> {
        let _guard = MEMORY_LOCK.lock().unwrap();
        let path = self.items_file();
        if let Ok(bytes) = fs::read(&path) {
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Atomically persists memory items using a crash-safe write-and-rename pattern.
    pub fn save_memory_items(&self, items: &[MemoryItem]) -> Result<()> {
        let _guard = MEMORY_LOCK.lock().unwrap();
        let _file_lock = FileLock::acquire(&self.lock_file())?;

        let json = serde_json::to_string_pretty(items)?;
        let tmp_file = self
            .memories_dir
            .join(format!(".memories.json.tmp.{}", Uuid::new_v4()));

        let mut file = fs::File::create(&tmp_file)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);

        fs::rename(&tmp_file, self.items_file())?;
        Ok(())
    }

    /// Consolidates a new fact or learning into persistent memory store with strict transactional isolation.
    pub fn consolidate_fact(&self, topic: &str, detail: &str) -> Result<MemoryItem> {
        let _guard = MEMORY_LOCK.lock().unwrap();
        let _file_lock = FileLock::acquire(&self.lock_file())?;

        let mut items: Vec<MemoryItem> = if let Ok(bytes) = fs::read(self.items_file()) {
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            Vec::new()
        };

        let topic_clean = topic.trim();
        let now = chrono::Utc::now().timestamp();

        let pos = items
            .iter()
            .position(|i| i.topic.eq_ignore_ascii_case(topic_clean));

        let item = match pos {
            Some(idx) => {
                items[idx].detail = detail.to_string();
                items[idx].citation_count += 1;
                items[idx].last_used_at = Some(now);
                items[idx].clone()
            }
            None => {
                let id = format!("mem_{}", &Uuid::new_v4().simple().to_string()[..8]);
                let new_item = MemoryItem {
                    id,
                    topic: topic_clean.to_string(),
                    detail: detail.to_string(),
                    citation_count: 1,
                    last_used_at: Some(now),
                    created_at: now,
                };
                items.push(new_item.clone());
                new_item
            }
        };

        let json = serde_json::to_string_pretty(&items)?;
        let tmp_file = self
            .memories_dir
            .join(format!(".memories.json.tmp.{}", Uuid::new_v4()));
        let mut file = fs::File::create(&tmp_file)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_file, self.items_file())?;

        Ok(item)
    }

    /// Records a citation / usage event for a specific memory item.
    pub fn record_citation(&self, id: &str) -> Result<bool> {
        let _guard = MEMORY_LOCK.lock().unwrap();
        let _file_lock = FileLock::acquire(&self.lock_file())?;

        let mut items: Vec<MemoryItem> = if let Ok(bytes) = fs::read(self.items_file()) {
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            Vec::new()
        };

        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.citation_count += 1;
            item.last_used_at = Some(chrono::Utc::now().timestamp());

            let json = serde_json::to_string_pretty(&items)?;
            let tmp_file = self
                .memories_dir
                .join(format!(".memories.json.tmp.{}", Uuid::new_v4()));
            let mut file = fs::File::create(&tmp_file)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp_file, self.items_file())?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Searches memories by keyword query in topic or detail.
    pub fn search_memories(&self, query: &str) -> Vec<MemoryItem> {
        let q = query.to_ascii_lowercase();
        self.list_memory_items()
            .into_iter()
            .filter(|i| {
                i.topic.to_ascii_lowercase().contains(&q)
                    || i.detail.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn record_memory(&self, memory: &MemoryRecord) -> Result<()> {
        let summaries_dir = self.memories_dir.join("rollout_summaries");
        let summary_file = summaries_dir.join(format!("{}.md", memory.thread_id));
        let summary_content = format!(
            "# Session Summary: {}\n\n- Thread ID: {}\n- Date: {}\n- Tags: {}\n\n## Takeaways\n{}\n",
            memory.title,
            memory.thread_id,
            chrono::DateTime::from_timestamp(memory.created_at, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default(),
            memory.tags.join(", "),
            memory.summary
        );
        fs::write(&summary_file, summary_content)?;

        // Consolidate into structured store
        let _ = self.consolidate_fact(&memory.title, &memory.raw_memory);

        // Also append to raw_memories.md for backward compatibility
        let raw_file = self.memories_dir.join("raw_memories.md");
        let entry = format!(
            "\n### Memory: {} ({})\n{}\n",
            memory.title, memory.thread_id, memory.raw_memory
        );
        let mut existing = fs::read_to_string(&raw_file).unwrap_or_default();
        if existing.is_empty() {
            existing = "# Consolidated Agent Memories & Project Learnings\n".to_string();
        }
        existing.push_str(&entry);
        fs::write(&raw_file, existing)?;

        Ok(())
    }

    pub fn load_consolidated_memories(&self) -> String {
        let raw_file = self.memories_dir.join("raw_memories.md");
        fs::read_to_string(&raw_file).unwrap_or_default()
    }

    /// Renders persistent memories block formatted with citation tags (`[memory:<id>]`).
    pub fn render_memories_prompt(&self) -> String {
        let items = self.list_memory_items();
        if items.is_empty() {
            let legacy = self.load_consolidated_memories();
            if legacy.trim().is_empty() {
                return String::new();
            }
            return format!(
                "\n## Persistent Knowledge & Past Memories\nBelow is accumulated cross-session context, learnings, and preferences:\n\n{}\n",
                legacy.trim()
            );
        }

        let mut out = String::from("\n## Persistent Knowledge & Past Memories\n");
        out.push_str("When applying these learnings, cite the corresponding memory tag (e.g. `[memory:<id>]`):\n\n");
        for item in items {
            out.push_str(&format!(
                "- **{}** `[memory:{}]`: {}\n",
                item.topic, item.id, item.detail
            ));
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_memory_recording_and_loading() {
        let tmp = tempdir().unwrap();
        let mgr = MemoryManager::with_dir(tmp.path().to_path_buf());

        let mem = MemoryRecord {
            id: "mem-1".to_string(),
            thread_id: "thread-abc".to_string(),
            title: "Cargo Clippy Preference".to_string(),
            raw_memory: "Always run cargo clippy --workspace before finishing any Rust change."
                .to_string(),
            summary: "Enforce cargo clippy on workspace".to_string(),
            tags: vec!["rust".to_string(), "linting".to_string()],
            created_at: 1700000000,
        };

        mgr.record_memory(&mem).unwrap();
        let prompt = mgr.render_memories_prompt();
        assert!(prompt.contains("Cargo Clippy Preference"));
        assert!(prompt.contains("Always run cargo clippy"));
    }

    #[test]
    fn test_fact_consolidation_and_citations() {
        let tmp = tempdir().unwrap();
        let mgr = MemoryManager::with_dir(tmp.path().to_path_buf());

        let item1 = mgr
            .consolidate_fact(
                "Docker Mounts",
                "Use --ro-bind /dev/null for single file masks",
            )
            .unwrap();
        assert_eq!(item1.citation_count, 1);
        assert!(item1.id.starts_with("mem_"));

        // Consolidate updated detail for same topic
        let item1_up = mgr
            .consolidate_fact(
                "Docker Mounts",
                "Use --ro-bind /dev/null for files and --tmpfs for dirs",
            )
            .unwrap();
        assert_eq!(item1_up.id, item1.id);
        assert_eq!(item1_up.citation_count, 2);
        assert!(item1_up.detail.contains("--tmpfs"));

        // Citation increment
        let cited = mgr.record_citation(&item1.id).unwrap();
        assert!(cited);
        let items = mgr.list_memory_items();
        assert_eq!(items[0].citation_count, 3);

        // Search
        let hits = mgr.search_memories("tmpfs");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, item1.id);

        let prompt = mgr.render_memories_prompt();
        assert!(prompt.contains(&format!("[memory:{}]", item1.id)));
    }

    #[test]
    fn test_concurrent_memory_persistence() {
        use std::sync::Arc;
        use std::thread;

        let tmp = tempdir().unwrap();
        let mgr = Arc::new(MemoryManager::with_dir(tmp.path().to_path_buf()));

        let mut handles = Vec::new();
        for i in 0..10 {
            let mgr_clone = Arc::clone(&mgr);
            handles.push(thread::spawn(move || {
                let topic = format!("Topic-{}", i);
                let detail = format!("Detail for topic {}", i);
                mgr_clone.consolidate_fact(&topic, &detail).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let items = mgr.list_memory_items();
        assert_eq!(
            items.len(),
            10,
            "All 10 concurrent writes must be persisted without corruption"
        );
        for i in 0..10 {
            let topic = format!("Topic-{}", i);
            assert!(
                items.iter().any(|item| item.topic == topic),
                "Missing persisted item for {}",
                topic
            );
        }
    }
}
