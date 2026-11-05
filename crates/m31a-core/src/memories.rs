//! Long-term memory storage, semantic indexing, and knowledge consolidation.
//!
//! Manages persistent memory records across sessions, indexing insights and project guidelines.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use uuid::Uuid;

static MEMORY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Method used to retrieve memory items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalMethod {
    Semantic,
    Lexical,
    Hybrid,
}

impl std::fmt::Display for RetrievalMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Semantic => write!(f, "semantic"),
            Self::Lexical => write!(f, "lexical"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Abstract generator for dense vector text embeddings.
pub trait EmbeddingGenerator: Send + Sync {
    /// Computes dense embedding vector for the given input text.
    fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;
    /// Returns the embedding vector dimensionality.
    fn dimension(&self) -> usize;
}

/// Mock embedding generator for tests and deterministic offline use.
#[derive(Debug, Clone)]
pub struct MockEmbeddingGenerator {
    dim: usize,
}

impl MockEmbeddingGenerator {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl Default for MockEmbeddingGenerator {
    fn default() -> Self {
        Self { dim: 8 }
    }
}

impl EmbeddingGenerator for MockEmbeddingGenerator {
    fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        if self.dim == 0 {
            anyhow::bail!("Embedding dimension cannot be 0");
        }
        let mut vec = vec![0.0f32; self.dim];
        for (i, b) in text.as_bytes().iter().enumerate() {
            vec[i % self.dim] += (*b as f32) / 255.0 + 0.01;
        }
        normalize_vector(&vec)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// HTTP client embedding generator for remote providers (NVIDIA NIM, OpenAI).
#[derive(Clone)]
pub struct HttpEmbeddingGenerator {
    endpoint: String,
    api_key: String,
    model: String,
    dim: usize,
    client: reqwest::Client,
}

impl HttpEmbeddingGenerator {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        dim: usize,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            dim,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl EmbeddingGenerator for HttpEmbeddingGenerator {
    fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let endpoint = self.endpoint.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let text_owned = text.to_string();
        let client = self.client.clone();
        let dim = self.dim;

        let res = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(async move {
                let body = serde_json::json!({
                    "input": text_owned,
                    "model": model,
                });
                let resp = client
                    .post(&endpoint)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await?;
                if !resp.status().is_success() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    anyhow::bail!("Embedding endpoint error {}: {}", status, text);
                }
                let parsed: serde_json::Value = resp.json().await?;
                let embedding_val = parsed
                    .get("data")
                    .and_then(|d| d.get(0))
                    .and_then(|d| d.get("embedding"))
                    .context("Invalid embedding response structure: missing data[0].embedding")?;
                let raw_vec: Vec<f32> = serde_json::from_value(embedding_val.clone())?;
                if dim > 0 && raw_vec.len() != dim {
                    anyhow::bail!(
                        "Dimension mismatch from provider: expected {}, got {}",
                        dim,
                        raw_vec.len()
                    );
                }
                normalize_vector(&raw_vec)
            })
        })
        .join()
        .map_err(|_| anyhow::anyhow!("Embedding generation thread panicked"))??;

        Ok(res)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// Normalizes a float vector to unit Euclidean norm.
pub fn normalize_vector(v: &[f32]) -> Result<Vec<f32>> {
    if v.is_empty() {
        anyhow::bail!("Cannot normalize an empty vector");
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-12 {
        anyhow::bail!("Cannot normalize a zero-norm vector");
    }
    Ok(v.iter().map(|x| x / norm).collect())
}

/// Computes cosine similarity between two dense embedding vectors, validating dimensional consistency.
pub fn calculate_cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        anyhow::bail!("Vector dimension mismatch: {} vs {}", a.len(), b.len());
    }
    if a.is_empty() {
        anyhow::bail!("Cannot calculate cosine similarity for empty vectors");
    }
    let norm_a = normalize_vector(a)?;
    let norm_b = normalize_vector(b)?;
    let dot: f32 = norm_a.iter().zip(norm_b.iter()).map(|(x, y)| x * y).sum();
    Ok(dot.clamp(-1.0, 1.0))
}

/// Computes non-negative cosine similarity clamped to [0.0, 1.0], returning 0.0 on error or mismatch.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    calculate_cosine_similarity(a, b).unwrap_or(0.0).max(0.0)
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryItem {
    pub id: String,
    pub topic: String,
    pub detail: String,
    pub citation_count: usize,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoredMemoryItem {
    pub item: MemoryItem,
    pub lexical_score: f32,
    pub semantic_score: f32,
    pub hybrid_score: f32,
}

fn compute_lexical_score(query: &str, topic: &str, detail: &str) -> f32 {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return 0.0;
    }
    let topic_lower = topic.to_ascii_lowercase();
    let detail_lower = detail.to_ascii_lowercase();

    let mut score = 0.0f32;
    if topic_lower == q {
        score += 1.0;
    } else if topic_lower.contains(&q) {
        score += 0.8;
    } else if detail_lower.contains(&q) {
        score += 0.5;
    }

    let query_tokens: Vec<&str> = q.split_whitespace().filter(|w| !w.is_empty()).collect();
    if !query_tokens.is_empty() {
        let mut matched = 0usize;
        for token in &query_tokens {
            if topic_lower.contains(token) || detail_lower.contains(token) {
                matched += 1;
            }
        }
        let ratio = matched as f32 / query_tokens.len() as f32;
        score += ratio * 0.4;
    }

    score.clamp(0.0, 1.0)
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
    embedding_generator: Option<Arc<dyn EmbeddingGenerator>>,
}

impl MemoryManager {
    pub fn default_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".m31a").join("memories")
    }

    fn detect_embedding_generator() -> Option<Arc<dyn EmbeddingGenerator>> {
        if let Ok(key) = std::env::var("NVIDIA_API_KEY") {
            let key = key.trim();
            if !key.is_empty() {
                return Some(Arc::new(HttpEmbeddingGenerator::new(
                    "https://integrate.api.nvidia.com/v1/embeddings",
                    key,
                    "nvidia/nv-embedqa-e5-v5",
                    1024,
                )));
            }
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            let key = key.trim();
            if !key.is_empty() {
                return Some(Arc::new(HttpEmbeddingGenerator::new(
                    "https://api.openai.com/v1/embeddings",
                    key,
                    "text-embedding-3-small",
                    1536,
                )));
            }
        }
        None
    }

    pub fn new() -> Result<Self> {
        let dir = Self::default_dir();
        fs::create_dir_all(&dir)?;
        fs::create_dir_all(dir.join("rollout_summaries"))?;
        let generator = Self::detect_embedding_generator();
        Ok(Self {
            memories_dir: dir,
            embedding_generator: generator,
        })
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::create_dir_all(dir.join("rollout_summaries"));
        let generator = Self::detect_embedding_generator();
        Self {
            memories_dir: dir,
            embedding_generator: generator,
        }
    }

    pub fn with_embedding_generator(mut self, generator: Arc<dyn EmbeddingGenerator>) -> Self {
        self.embedding_generator = Some(generator);
        self
    }

    pub fn set_embedding_generator(&mut self, generator: Option<Arc<dyn EmbeddingGenerator>>) {
        self.embedding_generator = generator;
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
    /// If an embedding generator is configured, dense vectors are calculated and normalized automatically.
    pub fn consolidate_fact(&self, topic: &str, detail: &str) -> Result<MemoryItem> {
        let embedding = if let Some(ref generator) = self.embedding_generator {
            let text = format!("{}: {}", topic.trim(), detail.trim());
            match generator.generate_embedding(&text) {
                Ok(emb) => Some(emb),
                Err(err) => {
                    tracing::warn!(
                        "Failed to generate embedding for fact consolidation: {}",
                        err
                    );
                    None
                }
            }
        } else {
            None
        };
        self.consolidate_fact_with_embedding(topic, detail, embedding)
    }

    /// Consolidates a fact with an optional dense vector embedding for hybrid retrieval.
    pub fn consolidate_fact_with_embedding(
        &self,
        topic: &str,
        detail: &str,
        embedding: Option<Vec<f32>>,
    ) -> Result<MemoryItem> {
        let normalized_embedding = embedding.and_then(|v| normalize_vector(&v).ok());
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
                if normalized_embedding.is_some() {
                    items[idx].embedding = normalized_embedding;
                }
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
                    embedding: normalized_embedding,
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

    /// Performs hybrid search combining lexical scoring and dense vector semantic similarity.
    /// If `query_embedding` is `None` (e.g. embedding service is offline or unconfigured),
    /// it seamlessly falls back to pure local lexical scoring without failing.
    pub fn search_memories_hybrid(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
    ) -> Vec<ScoredMemoryItem> {
        let items = self.list_memory_items();
        let mut scored: Vec<ScoredMemoryItem> = items
            .into_iter()
            .filter_map(|item| {
                let lex_score = compute_lexical_score(query, &item.topic, &item.detail);
                let sem_score = match (query_embedding, &item.embedding) {
                    (Some(q_emb), Some(item_emb)) => cosine_similarity(q_emb, item_emb),
                    _ => 0.0,
                };

                let hybrid_score = if query_embedding.is_some() && item.embedding.is_some() {
                    0.5 * lex_score + 0.5 * sem_score
                } else {
                    lex_score
                };

                if hybrid_score > 0.0 {
                    Some(ScoredMemoryItem {
                        item,
                        lexical_score: lex_score,
                        semantic_score: sem_score,
                        hybrid_score,
                    })
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.hybrid_score
                .partial_cmp(&a.hybrid_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }

    /// Searches memories returning both the retrieval method used and scored results.
    /// If an embedding generator is present and succeeds, hybrid retrieval is performed.
    /// Otherwise, it safely falls back to local lexical retrieval.
    pub fn search_memories_with_mode(
        &self,
        query: &str,
    ) -> (RetrievalMethod, Vec<ScoredMemoryItem>) {
        if let Some(ref generator) = self.embedding_generator {
            match generator.generate_embedding(query) {
                Ok(query_emb) => {
                    let hits = self.search_memories_hybrid(query, Some(&query_emb));
                    let has_semantic = hits.iter().any(|h| h.semantic_score > 0.0);
                    let mode = if has_semantic {
                        RetrievalMethod::Hybrid
                    } else {
                        RetrievalMethod::Lexical
                    };
                    (mode, hits)
                }
                Err(err) => {
                    tracing::warn!(
                        "Embedding generation failed for search query, falling back to lexical: {}",
                        err
                    );
                    let hits = self.search_memories_hybrid(query, None);
                    (RetrievalMethod::Lexical, hits)
                }
            }
        } else {
            let hits = self.search_memories_hybrid(query, None);
            (RetrievalMethod::Lexical, hits)
        }
    }

    /// Searches memories by keyword query in topic or detail using hybrid scoring with fallback.
    pub fn search_memories(&self, query: &str) -> Vec<MemoryItem> {
        self.search_memories_with_mode(query)
            .1
            .into_iter()
            .map(|s| s.item)
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

        // Consolidate into structured store, propagating any persistence error
        self.consolidate_fact(&memory.title, &memory.raw_memory)?;

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

    #[test]
    fn test_hybrid_search_and_offline_lexical_fallback() {
        let tmp = tempdir().unwrap();
        let mgr = MemoryManager::with_dir(tmp.path().to_path_buf());

        // Consolidate facts with vector embeddings
        let emb1 = vec![1.0, 0.0, 0.0];
        let emb2 = vec![0.0, 1.0, 0.0];

        mgr.consolidate_fact_with_embedding(
            "Rust Performance",
            "Use jemalloc and profile with flamegraphs",
            Some(emb1.clone()),
        )
        .unwrap();

        mgr.consolidate_fact_with_embedding(
            "Python Virtualenv",
            "Always activate .venv before running pip install",
            Some(emb2.clone()),
        )
        .unwrap();

        // 1. Hybrid search with query embedding matching Rust Performance
        let query_emb = vec![0.95, 0.05, 0.0];
        let scored = mgr.search_memories_hybrid("optimization", Some(&query_emb));
        assert!(!scored.is_empty());
        assert_eq!(scored[0].item.topic, "Rust Performance");
        assert!(scored[0].semantic_score > 0.9);

        // 2. Offline / None query embedding fallback to purely lexical search
        let lexical_hits = mgr.search_memories("virtualenv");
        assert_eq!(lexical_hits.len(), 1);
        assert_eq!(lexical_hits[0].topic, "Python Virtualenv");

        let rust_hits = mgr.search_memories("jemalloc");
        assert_eq!(rust_hits.len(), 1);
        assert_eq!(rust_hits[0].topic, "Rust Performance");
    }

    #[test]
    fn test_vector_normalization() {
        assert!(normalize_vector(&[]).is_err());
        assert!(normalize_vector(&[0.0, 0.0, 0.0]).is_err());

        let v = vec![3.0, 4.0];
        let norm_v = normalize_vector(&v).unwrap();
        assert!((norm_v[0] - 0.6).abs() < 1e-6);
        assert!((norm_v[1] - 0.8).abs() < 1e-6);
        let len_sq = norm_v[0] * norm_v[0] + norm_v[1] * norm_v[1];
        assert!((len_sq - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_dimension_validation() {
        // Dimension mismatch fails closed
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let err = calculate_cosine_similarity(&a, &b);
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("dimension mismatch"));

        // Cosine similarity between identical vectors is 1.0
        let sim_ident = calculate_cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).unwrap();
        assert!((sim_ident - 1.0).abs() < 1e-6);

        // Cosine similarity between orthogonal vectors is 0.0
        let sim_ortho = calculate_cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).unwrap();
        assert!(sim_ortho.abs() < 1e-6);
    }

    #[test]
    fn test_search_memories_with_mode_hybrid_and_lexical() {
        let tmp = tempdir().unwrap();
        let generator = Arc::new(MockEmbeddingGenerator::new(4));
        let mgr =
            MemoryManager::with_dir(tmp.path().to_path_buf()).with_embedding_generator(generator);

        // Consolidate fact with automatic embedding generation
        let item = mgr
            .consolidate_fact(
                "Asynchronous Rust",
                "Use tokio runtime with cooperative cancellation",
            )
            .unwrap();
        assert!(item.embedding.is_some());
        assert_eq!(item.embedding.as_ref().unwrap().len(), 4);

        // Search with embedding generator -> Hybrid retrieval
        let (mode, hits) = mgr.search_memories_with_mode("Asynchronous Rust");
        assert_eq!(mode, RetrievalMethod::Hybrid);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].item.topic, "Asynchronous Rust");
        assert!(hits[0].hybrid_score > 0.0);

        // Manager without generator -> Lexical retrieval
        let mgr_offline = MemoryManager::with_dir(tmp.path().to_path_buf());
        let (offline_mode, offline_hits) = mgr_offline.search_memories_with_mode("Asynchronous");
        assert_eq!(offline_mode, RetrievalMethod::Lexical);
        assert!(!offline_hits.is_empty());
    }
}
