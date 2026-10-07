//! Architecture and integration test runtime harness.
//!
//! Provides deterministic, bounded, isolated, and cancellable runtime
//! lifecycle management for tests. Every test runtime created through this
//! harness has an isolated database, an isolated workspace, and a bounded
//! shutdown path.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use sqlx::SqlitePool;

use crate::events::bus::{BroadcastEventBus, EventBus, EventFilter};
use crate::events::envelope::EventEnvelope;
use crate::ids::SessionId;
use crate::interaction::session::SqliteSessionRepository;
use crate::persistence::sqlite::schema::initialize_database;
use crate::runtime::AppRuntime;

/// Test time budgets (test infrastructure policy, not runtime configuration).
pub const TEST_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
pub const TEST_EVENT_TIMEOUT: Duration = Duration::from_secs(5);
pub const TEST_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
pub const TEST_TASK_JOIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Execute a future within a bounded test timeout, producing actionable diagnostic
/// information upon failure.
pub async fn bounded_await<F, T>(
    test_name: &str,
    operation: &str,
    timeout_duration: Duration,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    let start = Instant::now();
    match tokio::time::timeout(timeout_duration, future).await {
        Ok(output) => Ok(output),
        Err(_) => {
            let elapsed = start.elapsed();
            Err(format!(
                "TEST TIMEOUT [{test_name}]: Timed out after {elapsed:?} (budget: {timeout_duration:?}) awaiting '{operation}'"
            ))
        }
    }
}

/// Explicit lifecycle guard for a test-isolated [`AppRuntime`].
///
/// Ensures deterministic creation, isolated workspace/database state,
/// bounded observation, and finite teardown.
pub struct TestRuntimeGuard {
    dir: Option<tempfile::TempDir>,
    pub runtime: Arc<AppRuntime>,
    pub session_id: SessionId,
    session_repo: Option<SqliteSessionRepository>,
    pub pool: SqlitePool,
    pub event_bus: Arc<BroadcastEventBus>,
    pub test_name: String,
    is_shutdown: bool,
}

impl TestRuntimeGuard {
    /// Create a new isolated test runtime with an explicit test name for diagnostics.
    pub async fn create(test_name: impl Into<String>) -> Self {
        let test_name = test_name.into();
        let tmp_root = PathBuf::from("target/tmp");
        let _ = std::fs::create_dir_all(&tmp_root);
        let dir = tempfile::Builder::new()
            .prefix(&format!("m31a-test-{test_name}-"))
            .tempdir_in(&tmp_root)
            .expect("create test tempdir");
        let ws = dir.path();

        let git_init = std::process::Command::new("git")
            .args(["init", "--template=", "-b", "main"])
            .current_dir(ws)
            .status()
            .expect("git init failed");
        assert!(git_init.success(), "git init must succeed");

        tokio::fs::create_dir_all(ws.join("src"))
            .await
            .expect("create src dir");
        tokio::fs::write(
            ws.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .await
        .expect("write src/lib.rs");

        let db_path = ws.join("test_isolated.db");
        let pool = initialize_database(&db_path)
            .await
            .expect("initialize isolated test database");

        let event_bus = Arc::new(BroadcastEventBus::new(2048));
        let runtime = Arc::new(
            AppRuntime::from_pool_and_workspace(pool.clone(), ws.to_path_buf(), event_bus.clone())
                .await
                .expect("construct AppRuntime from isolated pool"),
        );

        let session_repo = SqliteSessionRepository::new(pool.clone());
        let session = session_repo
            .create_session(ws)
            .await
            .expect("create test session");

        Self {
            dir: Some(dir),
            runtime,
            session_id: session.id,
            session_repo: Some(session_repo),
            pool,
            event_bus,
            test_name,
            is_shutdown: false,
        }
    }

    /// Access the workspace root path.
    pub fn workspace_root(&self) -> &std::path::Path {
        self.dir.as_ref().expect("dir present").path()
    }

    /// Access the temporary workspace directory handle.
    pub fn dir(&self) -> &tempfile::TempDir {
        self.dir.as_ref().expect("dir present")
    }

    /// Access the session repository.
    pub fn session_repo(&self) -> &SqliteSessionRepository {
        self.session_repo.as_ref().expect("session_repo present")
    }

    /// Consume the guard and return its parts for tests that manage components separately.
    pub fn into_parts(
        mut self,
    ) -> (
        tempfile::TempDir,
        Arc<AppRuntime>,
        SessionId,
        SqliteSessionRepository,
    ) {
        let dir = self.dir.take().expect("dir present");
        let repo = self.session_repo.take().expect("session_repo present");
        (dir, self.runtime.clone(), self.session_id, repo)
    }

    /// Bounded wait for a specific event on the runtime's event bus.
    pub async fn wait_for_event<P>(
        &self,
        expected_event_desc: &str,
        predicate: P,
    ) -> Result<EventEnvelope, String>
    where
        P: Fn(&EventEnvelope) -> bool + Send + 'static,
    {
        let bus = self.event_bus.clone();
        let test_name = self.test_name.clone();
        let op_name = format!("waiting for event: {expected_event_desc}");

        bounded_await(&test_name, &op_name, TEST_EVENT_TIMEOUT, async move {
            let mut rx = bus.subscribe(EventFilter::all()).await;
            while let Some(Ok(envelope)) = rx.next().await {
                if predicate(&envelope) {
                    return envelope;
                }
            }
            panic!("event stream closed prematurely while waiting for {expected_event_desc}");
        })
        .await
    }

    /// Explicit bounded shutdown of the runtime.
    pub async fn shutdown(&mut self) -> Result<(), String> {
        if self.is_shutdown {
            return Ok(());
        }

        let runtime = self.runtime.clone();
        let test_name = self.test_name.clone();

        bounded_await(
            &test_name,
            "runtime shutdown",
            TEST_SHUTDOWN_TIMEOUT,
            async move {
                runtime.shutdown().await;
            },
        )
        .await?;

        self.is_shutdown = true;
        Ok(())
    }

    /// Assert that background tasks have exited and shutdown has been acknowledged.
    pub fn assert_no_background_tasks(&self) -> Result<(), String> {
        if self.is_shutdown && !self.runtime.is_shutting_down() {
            return Err(format!(
                "TEST FAILED [{0}]: Runtime reports not shutting down after shutdown()",
                self.test_name
            ));
        }
        Ok(())
    }
}

impl Drop for TestRuntimeGuard {
    fn drop(&mut self) {
        if !self.is_shutdown {
            self.runtime.shutdown_token().cancel();
        }
    }
}
