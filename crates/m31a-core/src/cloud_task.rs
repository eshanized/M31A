//! Remote cloud task management, polling interfaces, and background job coordination.
//!
//! Coordinates distributed subagent tasks running in remote execution environments.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Semaphore;

/// Status of an asynchronous background task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CloudTaskStatus {
    Queued,
    Running {
        started_at: u64,
    },
    Completed {
        exit_code: i32,
        stdout_len: usize,
        stderr_len: usize,
        duration_ms: u64,
    },
    Failed {
        error: String,
    },
    Cancelled,
}

/// Collected output of a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTaskOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// Serializable record for disk persistence across CLI commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTaskRecord {
    pub task_id: String,
    pub command: String,
    pub status: CloudTaskStatus,
    pub output: Option<CloudTaskOutput>,
    pub created_at: u64,
}

struct TaskInternal {
    command: String,
    status: CloudTaskStatus,
    output: Option<CloudTaskOutput>,
    abort_handle: Option<tokio::task::AbortHandle>,
    created_at: u64,
}

/// Asynchronous task coordinator managing distributed task execution, polling, and status reporting.
#[derive(Clone)]
pub struct CloudTaskManager {
    tasks: Arc<RwLock<HashMap<String, TaskInternal>>>,
    concurrency_sem: Arc<Semaphore>,
    next_counter: Arc<AtomicUsize>,
    persistence_path: Option<PathBuf>,
}

impl Default for CloudTaskManager {
    fn default() -> Self {
        Self::new(8)
    }
}

impl CloudTaskManager {
    pub fn new(max_concurrency: usize) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            concurrency_sem: Arc::new(Semaphore::new(max_concurrency)),
            next_counter: Arc::new(AtomicUsize::new(1)),
            persistence_path: None,
        }
    }

    /// Opens the persistent manager storing task states under `~/.m31a/cloud_tasks.json`.
    pub fn open_default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let m31a_dir = home.join(".m31a");
        let _ = std::fs::create_dir_all(&m31a_dir);
        let path = m31a_dir.join("cloud_tasks.json");
        Self::with_persistence(path, 8)
    }

    /// Initializes manager loading existing records from `persistence_path` if present.
    pub fn with_persistence(path: PathBuf, max_concurrency: usize) -> Self {
        let mut map = HashMap::new();
        let mut max_id = 0;
        if path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(records) = serde_json::from_str::<Vec<PersistedTaskRecord>>(&content)
        {
            for r in records {
                if let Some(num_str) = r.task_id.strip_prefix("task-")
                    && let Ok(n) = num_str.parse::<usize>()
                    && n > max_id
                {
                    max_id = n;
                }
                map.insert(
                    r.task_id,
                    TaskInternal {
                        command: r.command,
                        status: r.status,
                        output: r.output,
                        abort_handle: None,
                        created_at: r.created_at,
                    },
                );
            }
        }

        Self {
            tasks: Arc::new(RwLock::new(map)),
            concurrency_sem: Arc::new(Semaphore::new(max_concurrency)),
            next_counter: Arc::new(AtomicUsize::new(max_id + 1)),
            persistence_path: Some(path),
        }
    }

    fn flush_to_disk(&self) {
        if let Some(ref path) = self.persistence_path {
            let map = match self.tasks.read() {
                Ok(m) => m,
                Err(_) => return,
            };
            let records: Vec<PersistedTaskRecord> = map
                .iter()
                .map(|(k, v)| PersistedTaskRecord {
                    task_id: k.clone(),
                    command: v.command.clone(),
                    status: v.status.clone(),
                    output: v.output.clone(),
                    created_at: v.created_at,
                })
                .collect();
            if let Ok(json) = serde_json::to_string_pretty(&records) {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Submit a command for asynchronous background execution.
    pub fn submit(
        &self,
        command_str: &str,
        cwd: &Path,
        env: HashMap<String, String>,
        timeout_secs: u64,
    ) -> String {
        let count = self.next_counter.fetch_add(1, Ordering::SeqCst);
        let task_id = format!("task-{}", count);
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        {
            let mut map = self.tasks.write().unwrap();
            map.insert(
                task_id.clone(),
                TaskInternal {
                    command: command_str.to_string(),
                    status: CloudTaskStatus::Queued,
                    output: None,
                    abort_handle: None,
                    created_at: now_unix,
                },
            );
        }
        self.flush_to_disk();

        let task_id_clone = task_id.clone();
        let cmd_string = command_str.to_string();
        let cwd_buf = cwd.to_path_buf();
        let tasks_map = Arc::clone(&self.tasks);
        let sem = Arc::clone(&self.concurrency_sem);
        let persist_path_clone = self.persistence_path.clone();

        let flush_helper = move |tasks: &Arc<RwLock<HashMap<String, TaskInternal>>>| {
            if let Some(ref path) = persist_path_clone
                && let Ok(map) = tasks.read()
            {
                let records: Vec<PersistedTaskRecord> = map
                    .iter()
                    .map(|(k, v)| PersistedTaskRecord {
                        task_id: k.clone(),
                        command: v.command.clone(),
                        status: v.status.clone(),
                        output: v.output.clone(),
                        created_at: v.created_at,
                    })
                    .collect();
                if let Ok(json) = serde_json::to_string_pretty(&records) {
                    let _ = std::fs::write(path, json);
                }
            }
        };

        let join_handle = tokio::spawn(async move {
            let _permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    let mut map = tasks_map.write().unwrap();
                    if let Some(t) = map.get_mut(&task_id_clone) {
                        t.status = CloudTaskStatus::Failed {
                            error: "Semaphore acquisition failed".to_string(),
                        };
                    }
                    drop(map);
                    flush_helper(&tasks_map);
                    return;
                }
            };

            let now_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            {
                let mut map = tasks_map.write().unwrap();
                if let Some(t) = map.get_mut(&task_id_clone) {
                    t.status = CloudTaskStatus::Running {
                        started_at: now_unix,
                    };
                }
            }
            flush_helper(&tasks_map);

            let start_instant = Instant::now();
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(&cmd_string);
            cmd.current_dir(&cwd_buf);
            cmd.envs(&env);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let mut map = tasks_map.write().unwrap();
                    if let Some(t) = map.get_mut(&task_id_clone) {
                        t.status = CloudTaskStatus::Failed {
                            error: format!("Failed to spawn process: {e}"),
                        };
                    }
                    drop(map);
                    flush_helper(&tasks_map);
                    return;
                }
            };

            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            let timeout_duration =
                Duration::from_secs(if timeout_secs == 0 { 300 } else { timeout_secs });
            let status_res = tokio::time::timeout(timeout_duration, child.wait()).await;
            let duration_ms = start_instant.elapsed().as_millis() as u64;

            match status_res {
                Ok(Ok(exit_status)) => {
                    let mut stdout = String::new();
                    if let Some(mut r) = stdout_pipe {
                        let mut buf = Vec::new();
                        let _ = r.read_to_end(&mut buf).await;
                        stdout = String::from_utf8_lossy(&buf).to_string();
                    }

                    let mut stderr = String::new();
                    if let Some(mut r) = stderr_pipe {
                        let mut buf = Vec::new();
                        let _ = r.read_to_end(&mut buf).await;
                        stderr = String::from_utf8_lossy(&buf).to_string();
                    }

                    let exit_code = exit_status.code().unwrap_or(-1);
                    let mut map = tasks_map.write().unwrap();
                    if let Some(t) = map.get_mut(&task_id_clone) {
                        t.status = CloudTaskStatus::Completed {
                            exit_code,
                            stdout_len: stdout.len(),
                            stderr_len: stderr.len(),
                            duration_ms,
                        };
                        t.output = Some(CloudTaskOutput {
                            exit_code,
                            stdout,
                            stderr,
                            duration_ms,
                        });
                    }
                }
                Ok(Err(e)) => {
                    let mut map = tasks_map.write().unwrap();
                    if let Some(t) = map.get_mut(&task_id_clone) {
                        t.status = CloudTaskStatus::Failed {
                            error: format!("Process wait failed: {e}"),
                        };
                    }
                }
                Err(_) => {
                    let _ = child.kill().await;
                    let mut map = tasks_map.write().unwrap();
                    if let Some(t) = map.get_mut(&task_id_clone) {
                        t.status = CloudTaskStatus::Failed {
                            error: format!("Task timed out after {}s", timeout_duration.as_secs()),
                        };
                    }
                }
            }
            flush_helper(&tasks_map);
        });

        {
            let mut map = self.tasks.write().unwrap();
            if let Some(t) = map.get_mut(&task_id) {
                t.abort_handle = Some(join_handle.abort_handle());
            }
        }

        task_id
    }

    /// Query the status of an ongoing or completed task.
    pub fn status(&self, task_id: &str) -> Option<CloudTaskStatus> {
        let map = self.tasks.read().unwrap();
        map.get(task_id).map(|t| t.status.clone())
    }

    /// Retrieve task outputs if the task has completed.
    pub fn get_output(&self, task_id: &str) -> Option<CloudTaskOutput> {
        let map = self.tasks.read().unwrap();
        map.get(task_id).and_then(|t| t.output.clone())
    }

    /// Abort / cancel an in-flight background task.
    pub fn cancel(&self, task_id: &str) -> Result<bool> {
        let mut map = self.tasks.write().unwrap();
        if let Some(t) = map.get_mut(task_id) {
            if let Some(ref handle) = t.abort_handle {
                handle.abort();
            }
            t.status = CloudTaskStatus::Cancelled;
            drop(map);
            self.flush_to_disk();
            return Ok(true);
        }
        Ok(false)
    }

    /// List all registered tasks and their current statuses.
    pub fn list_tasks(&self) -> Vec<(String, CloudTaskStatus)> {
        let map = self.tasks.read().unwrap();
        map.iter()
            .map(|(k, v)| (k.clone(), v.status.clone()))
            .collect()
    }

    /// Runs best-of-N attempts (1–4) for a delegated command (§7.14).
    /// Returns a list of spawned sub-task IDs.
    pub fn submit_best_of_n(
        &self,
        command_str: &str,
        cwd: &Path,
        env: HashMap<String, String>,
        timeout_secs: u64,
        attempts: usize,
    ) -> Vec<String> {
        let n = attempts.clamp(1, 4);
        let mut ids = Vec::new();
        for _ in 0..n {
            ids.push(self.submit(command_str, cwd, env.clone(), timeout_secs));
        }
        ids
    }

    /// Selects the best completed attempt among a set of task IDs (§7.14).
    /// Best attempt has exit_code 0 and lowest duration_ms.
    pub fn select_best_attempt(&self, task_ids: &[String]) -> Option<(String, CloudTaskOutput)> {
        let mut best: Option<(String, CloudTaskOutput)> = None;
        for id in task_ids {
            if let Some(output) = self.get_output(id) {
                if output.exit_code == 0 {
                    match &best {
                        None => best = Some((id.clone(), output)),
                        Some((_, current_best)) => {
                            if output.duration_ms < current_best.duration_ms {
                                best = Some((id.clone(), output));
                            }
                        }
                    }
                } else if best.is_none() {
                    best = Some((id.clone(), output));
                }
            }
        }
        best
    }

    /// Computes diff of task stdout/output against local target file or workspace (§7.14).
    pub fn diff_task_output(&self, task_id: &str, target_path: &Path) -> Result<String> {
        let output = self
            .get_output(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not completed or output not found"))?;
        let existing = if target_path.exists() {
            std::fs::read_to_string(target_path)?
        } else {
            String::new()
        };
        let text_diff = similar::TextDiff::from_lines(&existing, &output.stdout);
        let mut diff_str = String::new();
        for change in text_diff.iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            diff_str.push_str(&format!("{}{}", sign, change));
        }
        Ok(diff_str)
    }

    /// Applies task stdout output to a designated local file (§7.14).
    pub fn apply_task_output(&self, task_id: &str, target_path: &Path) -> Result<usize> {
        let output = self
            .get_output(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not completed or output not found"))?;
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target_path, &output.stdout)?;
        Ok(output.stdout.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cloud_task_manager_lifecycle() {
        let manager = CloudTaskManager::new(4);
        let cwd = std::env::current_dir().unwrap();

        let task_id = manager.submit("echo 'hello from cloud task'", &cwd, HashMap::new(), 10);
        assert!(task_id.starts_with("task-"));

        // Wait for completion
        let mut completed = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if let Some(CloudTaskStatus::Completed { exit_code, .. }) = manager.status(&task_id) {
                assert_eq!(exit_code, 0);
                completed = true;
                break;
            }
        }
        assert!(completed);

        let output = manager.get_output(&task_id).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("hello from cloud task"));
    }

    #[tokio::test]
    async fn test_cloud_task_persistence() {
        let temp = tempfile::tempdir().unwrap();
        let persist_file = temp.path().join("cloud_tasks.json");
        let cwd = std::env::current_dir().unwrap();

        // Create manager with persistence and submit task
        let manager = CloudTaskManager::with_persistence(persist_file.clone(), 2);
        let task_id = manager.submit("echo 'persisted output'", &cwd, HashMap::new(), 10);

        // Wait for task completion
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if let Some(CloudTaskStatus::Completed { .. }) = manager.status(&task_id) {
                break;
            }
        }

        // Re-open manager with same file
        let reloaded = CloudTaskManager::with_persistence(persist_file, 2);
        let status = reloaded.status(&task_id);
        assert!(matches!(status, Some(CloudTaskStatus::Completed { .. })));

        let output = reloaded.get_output(&task_id).unwrap();
        assert!(output.stdout.contains("persisted output"));
    }

    #[tokio::test]
    async fn test_cloud_task_cancellation() {
        let manager = CloudTaskManager::new(4);
        let cwd = std::env::current_dir().unwrap();

        let task_id = manager.submit("sleep 10", &cwd, HashMap::new(), 15);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let cancelled = manager.cancel(&task_id).unwrap();
        assert!(cancelled);
        assert_eq!(manager.status(&task_id), Some(CloudTaskStatus::Cancelled));
    }

    #[tokio::test]
    async fn test_best_of_n_and_diff_apply() {
        let manager = CloudTaskManager::new(4);
        let cwd = std::env::current_dir().unwrap();

        let task_ids =
            manager.submit_best_of_n("echo 'best attempt output'", &cwd, HashMap::new(), 10, 2);
        assert_eq!(task_ids.len(), 2);

        // Wait for tasks to complete
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if task_ids
                .iter()
                .all(|id| matches!(manager.status(id), Some(CloudTaskStatus::Completed { .. })))
            {
                break;
            }
        }

        let best = manager.select_best_attempt(&task_ids);
        assert!(best.is_some());
        let (best_id, best_output) = best.unwrap();
        assert_eq!(best_output.exit_code, 0);

        let temp = tempfile::tempdir().unwrap();
        let target_file = temp.path().join("out.txt");
        let diff = manager.diff_task_output(&best_id, &target_file).unwrap();
        assert!(diff.contains("best attempt output"));

        let bytes_applied = manager.apply_task_output(&best_id, &target_file).unwrap();
        assert!(bytes_applied > 0);
        let read_back = std::fs::read_to_string(&target_file).unwrap();
        assert!(read_back.contains("best attempt output"));
    }
}
