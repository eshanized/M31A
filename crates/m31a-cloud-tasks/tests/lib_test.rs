//! Dedicated integration tests for `m31a-cloud-tasks` (src/lib.rs).
//!
//! Validates:
//! - TaskSpec proto mappings and status enum conversions
//! - CloudTasksConfig default configuration and serialization
//! - LocalFallbackCloudTasksClient real local task execution (Zero-Mock)
//! - Agent lifecycle management (spawn, capability inspection, listing, termination)

use m31a_cloud_tasks::proto::SpawnAgentRequest;
use m31a_cloud_tasks::{
    AgentStatusEnum, CloudAuth, CloudTaskExecutor, CloudTasksConfig, LocalFallbackCloudTasksClient,
    TaskSpec, TaskStatusEnum,
};
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_task_spec_proto_conversion() {
    let mut env = HashMap::new();
    env.insert("TARGET_ENV".to_string(), "staging".to_string());

    let spec = TaskSpec {
        id: "task-001".to_string(),
        prompt: "Run benchmark suite".to_string(),
        model: "deepseek-r1".to_string(),
        tools: vec![],
        working_dir: "/workspace".to_string(),
        env_vars: env,
        timeout_seconds: 120,
        max_tokens: Some(4096),
        temperature: Some(0.2),
    };

    let proto_spec = spec.to_proto();
    assert_eq!(proto_spec.id, "task-001");
    assert_eq!(proto_spec.prompt, "Run benchmark suite");
    assert_eq!(proto_spec.model, "deepseek-r1");
    assert_eq!(proto_spec.timeout_seconds, 120);
    assert_eq!(proto_spec.max_tokens, 4096);
    assert_eq!(proto_spec.temperature, 0.2);
    assert_eq!(proto_spec.env_vars.get("TARGET_ENV").unwrap(), "staging");
}

#[test]
fn test_status_enum_mappings() {
    assert_eq!(TaskStatusEnum::from(0), TaskStatusEnum::Pending);
    assert_eq!(TaskStatusEnum::from(1), TaskStatusEnum::Running);
    assert_eq!(TaskStatusEnum::from(2), TaskStatusEnum::Completed);
    assert_eq!(TaskStatusEnum::from(3), TaskStatusEnum::Failed);
    assert_eq!(TaskStatusEnum::from(4), TaskStatusEnum::Cancelled);
    assert_eq!(TaskStatusEnum::from(999), TaskStatusEnum::Failed);

    assert_eq!(AgentStatusEnum::from(0), AgentStatusEnum::Starting);
    assert_eq!(AgentStatusEnum::from(1), AgentStatusEnum::Ready);
    assert_eq!(AgentStatusEnum::from(2), AgentStatusEnum::Busy);
    assert_eq!(AgentStatusEnum::from(3), AgentStatusEnum::Completed);
    assert_eq!(AgentStatusEnum::from(4), AgentStatusEnum::Failed);
    assert_eq!(AgentStatusEnum::from(5), AgentStatusEnum::Terminated);
    assert_eq!(AgentStatusEnum::from(999), AgentStatusEnum::Failed);
}

#[test]
fn test_cloud_tasks_config_serialization() {
    let config = CloudTasksConfig {
        endpoint: "https://tasks.us-east.m31a.dev:443".to_string(),
        auth: CloudAuth::ApiKey {
            key: "secret-key-123".to_string(),
        },
        tls_cert_path: Some("/etc/ssl/cert.pem".to_string()),
        tls_key_path: Some("/etc/ssl/key.pem".to_string()),
        ca_cert_path: None,
        timeout_seconds: 600,
        max_retries: 5,
    };

    let json_str = serde_json::to_string(&config).unwrap();
    assert!(json_str.contains("secret-key-123"));
    assert!(json_str.contains("tasks.us-east.m31a.dev"));

    let deserialized: CloudTasksConfig = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.timeout_seconds, 600);
    assert_eq!(deserialized.max_retries, 5);
}

#[tokio::test]
async fn test_local_fallback_client_execute_real_command() {
    let tmp = tempdir().unwrap();
    let client = LocalFallbackCloudTasksClient::new();

    let task = TaskSpec {
        id: "task-real-exec".to_string(),
        prompt: "!echo 'M31A_ZERO_MOCK_CLOUD_TASKS'".to_string(),
        model: "local".to_string(),
        tools: vec![],
        working_dir: tmp.path().to_str().unwrap().to_string(),
        env_vars: HashMap::new(),
        timeout_seconds: 30,
        max_tokens: None,
        temperature: None,
    };

    let result = client.execute_task(task).await.unwrap();
    assert_eq!(result.status, TaskStatusEnum::Completed);
    assert!(
        result.output.contains("M31A_ZERO_MOCK_CLOUD_TASKS"),
        "Real command output must match: {}",
        result.output
    );
    assert!(result.error.is_none());
}

#[tokio::test]
async fn test_local_fallback_client_agent_lifecycle() {
    let client = LocalFallbackCloudTasksClient::new();

    // 1. Check capabilities
    let caps = client.get_capabilities().await.unwrap();
    assert!(caps.supports_file_ops);
    assert!(caps.supports_process_exec);
    assert!(caps.max_memory_mb > 0);
    assert!(!caps.available_models.is_empty());

    // 2. Spawn agent
    let spawn_req = SpawnAgentRequest {
        session_id: "sess-fleet-001".to_string(),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        config: "{}".to_string(),
        capabilities: vec!["terminal".to_string(), "code-edit".to_string()],
        working_dir: "/tmp".to_string(),
    };

    let handle = client.spawn_agent(spawn_req).await.unwrap();
    assert_eq!(handle.session_id, "sess-fleet-001");
    assert_eq!(handle.status, AgentStatusEnum::Ready);
    assert!(handle.capabilities.contains(&"terminal".to_string()));

    // 3. List agents
    let agents = client.list_agents().await.unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_id, handle.agent_id);

    // 4. Send genuine command input & wait
    client
        .send_agent_input(&handle.agent_id, "echo status check ok".to_string())
        .await
        .unwrap();

    let wait_res = client.wait_agent(&handle.agent_id).await.unwrap();
    assert_eq!(wait_res.status, TaskStatusEnum::Completed);
    assert!(wait_res.output.contains("status check ok"));

    // 5. Test failure mode: command exit 42 MUST fail honestly, never fabricated success
    client
        .send_agent_input(&handle.agent_id, "exit 42".to_string())
        .await
        .unwrap();
    let fail_res = client.wait_agent(&handle.agent_id).await.unwrap();
    assert_eq!(fail_res.status, TaskStatusEnum::Failed);
    assert!(fail_res.error.unwrap().contains("42"));

    // 6. Close agent
    client.close_agent(&handle.agent_id).await.unwrap();
}
