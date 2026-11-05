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
async fn test_task_containing_cargo_in_natural_language_does_not_execute() {
    let client = LocalFallbackCloudTasksClient::new();
    let task = TaskSpec {
        id: "task-cargo-nl".to_string(),
        prompt: "Review the cargo dependencies in Cargo.toml and cargo check".to_string(),
        model: "local".to_string(),
        tools: vec![],
        working_dir: ".".to_string(),
        env_vars: HashMap::new(),
        timeout_seconds: 30,
        max_tokens: None,
        temperature: None,
    };

    let result = client.execute_task(task).await.unwrap();
    assert_eq!(result.status, TaskStatusEnum::Failed);
    assert!(result.output.is_empty());
    assert!(
        result
            .error
            .unwrap()
            .contains("requires a connected remote Cloud Tasks endpoint")
    );
}

#[tokio::test]
async fn test_tasks_mentioning_ls_or_echo_do_not_execute() {
    let client = LocalFallbackCloudTasksClient::new();

    // Natural language mentioning ls
    let task_ls = TaskSpec {
        id: "task-ls-nl".to_string(),
        prompt: "Run ls -la and summarize the directory".to_string(),
        model: "local".to_string(),
        tools: vec![],
        working_dir: ".".to_string(),
        env_vars: HashMap::new(),
        timeout_seconds: 30,
        max_tokens: None,
        temperature: None,
    };
    let res_ls = client.execute_task(task_ls).await.unwrap();
    assert_eq!(res_ls.status, TaskStatusEnum::Failed);
    assert!(res_ls.output.is_empty());

    // Natural language mentioning echo
    let task_echo = TaskSpec {
        id: "task-echo-nl".to_string(),
        prompt: "echo back the results of the analysis".to_string(),
        model: "local".to_string(),
        tools: vec![],
        working_dir: ".".to_string(),
        env_vars: HashMap::new(),
        timeout_seconds: 30,
        max_tokens: None,
        temperature: None,
    };
    let res_echo = client.execute_task(task_echo).await.unwrap();
    assert_eq!(res_echo.status, TaskStatusEnum::Failed);
    assert!(res_echo.output.is_empty());
}

#[tokio::test]
async fn test_send_agent_input_does_not_execute_shell() {
    let client = LocalFallbackCloudTasksClient::new();
    let res = client
        .send_agent_input("fake-agent-id", "echo malicious command".to_string())
        .await;
    assert!(
        res.is_err(),
        "send_agent_input must not execute shell commands"
    );
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("unsupported without an attached multi-agent engine")
    );

    let wait_res = client.wait_agent("fake-agent-id").await;
    assert!(wait_res.is_err());
    assert!(
        wait_res
            .unwrap_err()
            .to_string()
            .contains("unsupported without an attached multi-agent engine")
    );
}

#[tokio::test]
async fn test_local_agent_spawning_returns_unsupported() {
    let client = LocalFallbackCloudTasksClient::new();
    let spawn_req = SpawnAgentRequest {
        session_id: "sess-001".to_string(),
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        config: "{}".to_string(),
        capabilities: vec!["terminal".to_string()],
        working_dir: "/tmp".to_string(),
    };

    let res = client.spawn_agent(spawn_req).await;
    assert!(
        res.is_err(),
        "Local fallback must reject spawning without engine"
    );
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("unsupported without an attached multi-agent engine"),
        "Error must truthfully indicate unsupported agent runtime, got: {err_str}"
    );
}
