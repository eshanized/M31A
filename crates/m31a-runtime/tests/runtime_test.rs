//! Integration tests for `m31a-runtime`.

use m31a_core::Config;
use m31a_runtime::{RuntimeBuilder, RuntimeConfig, RuntimeEvent, RuntimeState};
use tempfile::tempdir;

#[tokio::test]
async fn test_runtime_initialization_and_services() {
    let temp = tempdir().unwrap();
    let temp_path = temp.path();

    let mut core = Config::default();
    core.api_key = Some("test-key".to_string());
    core.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
    core.model_provider = Some("nim".to_string());

    let db_path = temp_path.join("runtime_test.db");
    let storage = m31a_storage::Storage::open(&db_path).unwrap();

    let config = RuntimeConfig::for_test(core, temp_path);
    let runtime = RuntimeBuilder::new(config)
        .with_storage(storage)
        .build()
        .await
        .expect("Runtime build should succeed");

    assert_eq!(runtime.state(), RuntimeState::Ready);
    assert!(runtime.state().is_operational());
    assert!(!runtime.state().is_terminal());

    // Verify services are accessible and functional
    assert_eq!(runtime.config().cwd, temp_path);
    assert!(!runtime.tools().definitions().is_empty());
    assert_eq!(runtime.storage().list_threads(10).unwrap().len(), 0);
    assert!(runtime.watcher().is_none());

    let vault_dir = temp_path.join(".m31a-vault");
    assert!(vault_dir.is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&vault_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}

#[tokio::test]
async fn test_runtime_validation_rejects_empty_model() {
    let temp = tempdir().unwrap();
    let mut core = Config::default();
    core.model = "   ".to_string();

    let config = RuntimeConfig::new(core, temp.path().to_path_buf());
    let result = RuntimeBuilder::new(config).build().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_runtime_validation_rejects_nonexistent_directory() {
    let non_existent = std::path::PathBuf::from("/path/that/does/not/exist/987123");
    let core = Config::default();
    let config = RuntimeConfig::new(core, non_existent);
    let result = RuntimeBuilder::new(config).build().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_runtime_validation_rejects_workspace_file() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("not-a-workspace");
    std::fs::write(&file, "not a directory").unwrap();

    let config = RuntimeConfig::new(Config::default(), file);
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_runtime_lifecycle_graceful_shutdown_is_idempotent() {
    let temp = tempdir().unwrap();
    let temp_path = temp.path();

    let mut core = Config::default();
    core.api_key = Some("test-key".to_string());
    core.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
    core.model_provider = Some("nim".to_string());

    let db_path = temp_path.join("lifecycle_test.db");
    let storage = m31a_storage::Storage::open(&db_path).unwrap();

    let config = RuntimeConfig::for_test(core, temp_path);
    let runtime = RuntimeBuilder::new(config)
        .with_storage(storage)
        .build()
        .await
        .unwrap();

    let mut event_rx = runtime.subscribe_events();

    // Concurrent shutdown callers converge on one completed teardown report.
    let (first, second) = tokio::join!(runtime.shutdown(), runtime.shutdown());
    let report1 = first.expect("First shutdown succeeds");
    let report2 = second.expect("Concurrent shutdown succeeds");
    assert_eq!(report1, report2);
    assert!(report1.cleanly_stopped);
    assert_eq!(runtime.state(), RuntimeState::Stopped);
    assert!(runtime.state().is_terminal());

    // Verify shutdown events were broadcast
    let mut saw_shutting_down = false;
    let mut saw_shutdown_complete = false;
    while let Ok(event) = event_rx.try_recv() {
        match event {
            RuntimeEvent::ShuttingDown => saw_shutting_down = true,
            RuntimeEvent::ShutdownComplete { .. } => saw_shutdown_complete = true,
            _ => {}
        }
    }
    assert!(saw_shutting_down);
    assert!(saw_shutdown_complete);

    // Repeated shutdown returns the original report.
    let report3 = runtime
        .shutdown()
        .await
        .expect("Repeated shutdown succeeds");
    assert_eq!(report1, report3);
    assert!(report3.cleanly_stopped);
    assert_eq!(runtime.state(), RuntimeState::Stopped);
}

#[tokio::test]
async fn test_runtime_service_ownership_and_agent_runtime_wiring() {
    let temp = tempdir().unwrap();
    let temp_path = temp.path();

    let mut core = Config::default();
    core.api_key = Some("test-key".to_string());
    core.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
    core.model_provider = Some("nim".to_string());

    let db_path = temp_path.join("ownership_test.db");
    let storage = m31a_storage::Storage::open(&db_path).unwrap();

    let config = RuntimeConfig::for_test(core, temp_path);
    let runtime = RuntimeBuilder::new(config)
        .with_storage(storage)
        .build()
        .await
        .unwrap();

    // Verify ToolRegistry has agent runtime wired
    assert!(runtime.tools().is_allowed("spawn_agent"));

    // Verify trust store authorization evaluation
    let trust_res = m31a_runtime::services::workspace::verify_workspace_trust(
        &runtime.trust_store(),
        temp_path,
        true,
    );
    assert!(trust_res.is_ok());
}

#[tokio::test]
async fn test_multiple_independent_runtime_instances() {
    let temp1 = tempdir().unwrap();
    let temp2 = tempdir().unwrap();

    let mut core1 = Config::default();
    core1.api_key = Some("test-key-1".to_string());
    core1.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
    core1.model_provider = Some("nim".to_string());

    let mut core2 = Config::default();
    core2.api_key = Some("test-key-2".to_string());
    core2.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
    core2.model_provider = Some("nim".to_string());

    let db1 = temp1.path().join("db1.db");
    let db2 = temp2.path().join("db2.db");

    let s1 = m31a_storage::Storage::open(&db1).unwrap();
    let s2 = m31a_storage::Storage::open(&db2).unwrap();

    let rt1 = RuntimeBuilder::new(RuntimeConfig::for_test(core1, temp1.path()))
        .with_storage(s1)
        .build()
        .await
        .unwrap();

    let rt2 = RuntimeBuilder::new(RuntimeConfig::for_test(core2, temp2.path()))
        .with_storage(s2)
        .build()
        .await
        .unwrap();

    assert_eq!(rt1.state(), RuntimeState::Ready);
    assert_eq!(rt2.state(), RuntimeState::Ready);

    rt1.shutdown().await.unwrap();
    assert_eq!(rt1.state(), RuntimeState::Stopped);
    assert_eq!(rt2.state(), RuntimeState::Ready);

    rt2.shutdown().await.unwrap();
    assert_eq!(rt2.state(), RuntimeState::Stopped);
}
