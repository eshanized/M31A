//! Integration tests for Capability Registry, Service Seams, and Health/Permissions (CTL-01, CTL-02, CTL-03).

use async_trait::async_trait;
use m31a::capability::error::CapabilityError;
use m31a::capability::family::CapabilityFamily;
use m31a::capability::health::{CapabilityHealthState, HealthConfig};
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::{CapabilityOperation, CapabilityPermissions, RiskClass};
use m31a::capability::providers::LocalFileSystemProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::capability::traits::fs::{FileMetadata, FileSystemService};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_15_capability_families() {
    let registry = CapabilityRegistry::new();

    // 1. Verify exact 15 families enum variants exist
    let all_families = CapabilityFamily::all();
    assert_eq!(
        all_families.len(),
        15,
        "CTL-01 requires exactly 15 core capability families"
    );

    let expected_names = [
        "filesystem",
        "shell",
        "process",
        "terminal",
        "repository",
        "git",
        "web",
        "network",
        "jobs",
        "sandbox",
        "model",
        "memory",
        "verification",
        "artifacts",
        "telemetry",
    ];

    for name in &expected_names {
        let parsed: Result<CapabilityFamily, _> = name.parse();
        assert!(
            parsed.is_ok(),
            "Family name '{name}' must parse into a CapabilityFamily variant"
        );
        let family = parsed.unwrap();
        assert_eq!(family.as_str(), *name);
    }

    // 2. Register an instance for each of the 15 capability families
    for family in all_families {
        let instance_id = format!("{}.native", family.as_str());
        let instance = CapabilityInstance::new(
            instance_id.clone(),
            format!("Native {}", family.as_str()),
            "0.1.0",
            *family,
            "m31a-local",
            CapabilityPermissions::full_access(),
        );

        registry.register_instance(instance);
        assert!(
            registry.is_available(family),
            "CapabilityFamily {:?} should be available after registration",
            family
        );

        let retrieved = registry.get_instance(&instance_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().family, *family);
    }

    // 3. Verify list_capabilities returns all 15 instances
    let registered_capabilities = registry.list_capabilities();
    assert_eq!(
        registered_capabilities.len(),
        15,
        "CapabilityRegistry must contain 15 registered capability instances"
    );
}

/// In-memory mock filesystem provider to test provider replacement without consumer changes (CTL-02).
struct MockFileSystemProvider {
    memory: RwLock<HashMap<PathBuf, Vec<u8>>>,
}

impl MockFileSystemProvider {
    fn new() -> Self {
        Self {
            memory: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl FileSystemService for MockFileSystemProvider {
    async fn read_file(
        &self,
        path: &Path,
        _offset: Option<u64>,
        _limit: Option<usize>,
    ) -> Result<Vec<u8>, CapabilityError> {
        let mem = self.memory.read().unwrap();
        mem.get(path)
            .cloned()
            .ok_or_else(|| CapabilityError::NotFound(path.display().to_string()))
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<usize, CapabilityError> {
        let mut mem = self.memory.write().unwrap();
        mem.insert(path.to_path_buf(), content.to_vec());
        Ok(content.len())
    }

    async fn edit_file(
        &self,
        path: &Path,
        old_content: &str,
        new_content: &str,
    ) -> Result<(), CapabilityError> {
        let mut mem = self.memory.write().unwrap();
        let bytes = mem
            .get_mut(path)
            .ok_or_else(|| CapabilityError::NotFound(path.display().to_string()))?;
        let text = String::from_utf8(bytes.clone())
            .map_err(|e| CapabilityError::InvalidArgument(e.to_string()))?;
        if !text.contains(old_content) {
            return Err(CapabilityError::NotFound(
                "old content not found".to_string(),
            ));
        }
        *bytes = text.replacen(old_content, new_content, 1).into_bytes();
        Ok(())
    }

    async fn list_files(
        &self,
        _path: &Path,
        _recursive: bool,
    ) -> Result<Vec<PathBuf>, CapabilityError> {
        let mem = self.memory.read().unwrap();
        Ok(mem.keys().cloned().collect())
    }

    async fn file_metadata(&self, path: &Path) -> Result<FileMetadata, CapabilityError> {
        let mem = self.memory.read().unwrap();
        let bytes = mem
            .get(path)
            .ok_or_else(|| CapabilityError::NotFound(path.display().to_string()))?;
        Ok(FileMetadata {
            size_bytes: bytes.len() as u64,
            is_file: true,
            is_dir: false,
            is_readonly: false,
            modified_ms: None,
        })
    }

    async fn delete_file(&self, path: &Path) -> Result<(), CapabilityError> {
        let mut mem = self.memory.write().unwrap();
        mem.remove(path);
        Ok(())
    }
}

/// Consumer function demonstrating that consumers depend strictly on Arc<dyn FileSystemService> (CTL-02).
async fn consumer_read_file(
    fs: Arc<dyn FileSystemService>,
    path: &Path,
) -> Result<Vec<u8>, CapabilityError> {
    fs.read_file(path, None, None).await
}

#[tokio::test]
async fn test_provider_replacement() {
    let dir = tempdir().unwrap();
    let registry = CapabilityRegistry::new();

    // 1. Register LocalFileSystemProvider
    let local_fs = Arc::new(LocalFileSystemProvider::new(dir.path()).unwrap());
    registry.register_filesystem(local_fs.clone());

    // Write file using the local provider
    local_fs
        .write_file(Path::new("hello.txt"), b"Hello from local FS!")
        .await
        .unwrap();

    // Consumer reads using Arc<dyn FileSystemService> from registry
    let consumer_fs = registry.filesystem().expect("fs capability registered");
    let content = consumer_read_file(consumer_fs, Path::new("hello.txt"))
        .await
        .unwrap();
    assert_eq!(content, b"Hello from local FS!");

    // Verify workspace containment on LocalFileSystemProvider
    let traversal_err =
        consumer_read_file(registry.filesystem().unwrap(), Path::new("../escape.txt")).await;
    assert!(
        matches!(traversal_err, Err(CapabilityError::PathOutOfBounds { .. })),
        "Path traversal outside workspace must fail closed"
    );

    // 2. Hot-swap provider to MockFileSystemProvider in CapabilityRegistry
    let mock_fs = Arc::new(MockFileSystemProvider::new());
    mock_fs
        .write_file(Path::new("virtual.txt"), b"Hello from mock memory FS!")
        .await
        .unwrap();

    registry.register_filesystem(mock_fs);

    // 3. Consumer code is 100% identical, retrieves swapped provider without modification
    let swapped_fs = registry.filesystem().expect("fs capability registered");
    let swapped_content = consumer_read_file(swapped_fs, Path::new("virtual.txt"))
        .await
        .unwrap();
    assert_eq!(swapped_content, b"Hello from mock memory FS!");
}

#[test]
fn test_capability_health_state_transitions() {
    let health_config = HealthConfig {
        degraded_threshold: 1,
        unavailable_threshold: 3,
        cooldown_duration: Duration::from_millis(50),
    };
    let registry = CapabilityRegistry::with_health_config(health_config);

    let instance = CapabilityInstance::new(
        "fs.test",
        "Test FS",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "local",
        CapabilityPermissions::full_access(),
    );
    registry.register_instance(instance);

    // Initial state: Healthy and available
    assert_eq!(
        registry.get_health("fs.test"),
        Some(CapabilityHealthState::Healthy)
    );
    assert!(registry.is_available(&CapabilityFamily::Filesystem));

    // First infra fault -> Degraded
    registry.record_failure("fs.test", true, "socket timeout");
    assert_eq!(
        registry.get_health("fs.test"),
        Some(CapabilityHealthState::Degraded)
    );
    assert!(registry.is_available(&CapabilityFamily::Filesystem));

    // Second infra fault -> Degraded
    registry.record_failure("fs.test", true, "connection reset");
    assert_eq!(
        registry.get_health("fs.test"),
        Some(CapabilityHealthState::Degraded)
    );

    // Third infra fault -> Unavailable
    registry.record_failure("fs.test", true, "host down");
    assert_eq!(
        registry.get_health("fs.test"),
        Some(CapabilityHealthState::Unavailable)
    );
    assert!(!registry.is_available(&CapabilityFamily::Filesystem));

    // Cooldown expiry: transition to HalfOpen
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(
        registry.get_health("fs.test"),
        Some(CapabilityHealthState::HalfOpen)
    );
    assert!(registry.is_available(&CapabilityFamily::Filesystem));

    // Recovery probe succeeds -> Healthy
    registry.record_success("fs.test");
    assert_eq!(
        registry.get_health("fs.test"),
        Some(CapabilityHealthState::Healthy)
    );
    assert!(registry.is_available(&CapabilityFamily::Filesystem));
}

#[test]
fn test_semantic_vs_infrastructure_health_impact() {
    let health_config = HealthConfig {
        degraded_threshold: 1,
        unavailable_threshold: 2,
        cooldown_duration: Duration::from_millis(50),
    };
    let registry = CapabilityRegistry::with_health_config(health_config);

    let instance = CapabilityInstance::new(
        "git.test",
        "Test Git",
        "1.0.0",
        CapabilityFamily::Git,
        "cli",
        CapabilityPermissions::full_access(),
    );
    registry.register_instance(instance);

    // Repeated semantic task errors (e.g. file not found, bad user syntax, non-zero test exit)
    for _ in 0..10 {
        registry.record_failure("git.test", false, "branch not found");
    }

    // Health MUST remain Healthy
    assert_eq!(
        registry.get_health("git.test"),
        Some(CapabilityHealthState::Healthy)
    );
    assert!(registry.is_available(&CapabilityFamily::Git));

    // Infrastructure fault immediately degrades health
    registry.record_failure("git.test", true, "git binary not found / spawn failed");
    assert_eq!(
        registry.get_health("git.test"),
        Some(CapabilityHealthState::Degraded)
    );
}

#[test]
fn test_capability_permissions_fail_closed() {
    let registry = CapabilityRegistry::new();

    let perms = CapabilityPermissions::new([CapabilityOperation::Read])
        .with_path_scope("/workspace/safe_zone")
        .with_domains(vec!["api.m31a.dev".to_string()])
        .with_max_risk(RiskClass::ReadOnly);

    let instance = CapabilityInstance::new(
        "secure.reader",
        "Secure Reader",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "sandboxed",
        perms,
    );
    registry.register_instance(instance);

    // 1. Authorized Read in safe zone -> OK
    let ok_res = registry.validate_access(
        "secure.reader",
        CapabilityOperation::Read,
        Some(Path::new("/workspace/safe_zone/data.json")),
        None,
        Some(RiskClass::ReadOnly),
    );
    assert!(ok_res.is_ok());

    // 2. Unauthorized Write -> Fails closed
    let write_err = registry.validate_access(
        "secure.reader",
        CapabilityOperation::Write,
        Some(Path::new("/workspace/safe_zone/data.json")),
        None,
        Some(RiskClass::LowRiskMutation),
    );
    assert!(matches!(
        write_err,
        Err(CapabilityError::PermissionDenied(_))
    ));

    // 3. Out-of-boundary path -> Fails closed with PathOutOfBounds
    let path_err = registry.validate_access(
        "secure.reader",
        CapabilityOperation::Read,
        Some(Path::new("/etc/passwd")),
        None,
        Some(RiskClass::ReadOnly),
    );
    assert!(matches!(
        path_err,
        Err(CapabilityError::PathOutOfBounds { .. })
    ));

    // 4. Excessive risk -> Fails closed
    let risk_err = registry.validate_access(
        "secure.reader",
        CapabilityOperation::Read,
        Some(Path::new("/workspace/safe_zone/data.json")),
        None,
        Some(RiskClass::HighRiskMutation),
    );
    assert!(matches!(
        risk_err,
        Err(CapabilityError::PermissionDenied(_))
    ));

    // 5. Unauthorized network domain -> Fails closed
    let domain_err = registry.validate_access(
        "secure.reader",
        CapabilityOperation::Read,
        None,
        Some("untrusted.com"),
        Some(RiskClass::ReadOnly),
    );
    assert!(matches!(
        domain_err,
        Err(CapabilityError::PermissionDenied(_))
    ));
}
