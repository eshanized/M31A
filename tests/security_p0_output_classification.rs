//! P0-03 / P1-03 / P1-04 regression: classified output evidence.
//!
//! Injects known secrets into tool output and proves:
//! - model-visible output does not expose the secret
//! - diagnostics do not expose the secret
//! - conversation persistence does not expose the secret
//! - event persistence does not expose the secret
//! - artifacts carry explicit classification
//! - audit digest remains deterministic

use m31a::ids::ArtifactId;
use m31a::persistence::artifacts::{ArtifactStore, EvidenceClassification, FsArtifactStore};
use m31a::pipeline::stages::{
    AuditDigest, DiagnosticOutput, ModelVisibleOutput, PipelineOutputEvidence, RawExecutionEvidence,
};

const SECRETS: &[&str] = &[
    "nvapi-abcdefghijklmnopqrstuvwxyz0123456789ABCD", // dummy test fixture, never a real key
    "ghp_1111222233334444555566667777888899990000",
    "glpat-abcdefghij1234567890XYZ",
    "AKIAIOSFODNN7EXAMPLE",
    "postgres://admin:super_secret_pw123@db.internal:5432/m31a_db",
    "password=MySecretPassword123",
];

fn tool_output_with_secrets() -> String {
    format!(
        "tool finished\nnvidia key {}\ngithub {}\ngitlab {}\naws {}\ndb {}\nlogin {}\n-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0X\n-----END RSA PRIVATE KEY-----",
        SECRETS[0], SECRETS[1], SECRETS[2], SECRETS[3], SECRETS[4], SECRETS[5]
    )
}

#[test]
fn model_visible_output_never_exposes_secrets() {
    let raw = tool_output_with_secrets();
    let evidence = PipelineOutputEvidence::new(raw.clone(), raw.clone());
    for secret in SECRETS {
        assert!(
            !evidence.model_visible_output.contains(secret),
            "model-visible output leaked a secret"
        );
        assert!(
            !evidence.diagnostic_output.contains(secret),
            "diagnostic output leaked a secret"
        );
    }
    assert!(evidence.model_visible_output.contains("[REDACTED"));
    // Typed accessors agree with the raw fields.
    let typed_visible: ModelVisibleOutput = evidence.model_visible();
    for secret in SECRETS {
        assert!(!typed_visible.as_str().contains(secret));
    }
    let typed_diag: DiagnosticOutput = evidence.diagnostic();
    for secret in SECRETS {
        assert!(!typed_diag.as_str().contains(secret));
    }
}

#[test]
fn raw_evidence_classification_requires_explicit_access() {
    let raw = tool_output_with_secrets();
    let evidence = RawExecutionEvidence::new(raw.clone());
    // Classification consumes the raw value; projections are scrubbed.
    let (visible, diagnostic, digest) = evidence.classify("preview bytes=123".to_string());
    for secret in SECRETS {
        assert!(!visible.as_str().contains(secret));
        assert!(!diagnostic.as_str().contains(secret));
    }
    assert_eq!(digest.as_str().len(), 64);
    // Privileged access is explicit and auditable at the call site.
    let privileged = RawExecutionEvidence::new(raw.clone());
    assert_eq!(privileged.expose_privileged(), &raw);
}

#[test]
fn audit_digest_is_deterministic() {
    let raw = tool_output_with_secrets();
    let a = PipelineOutputEvidence::new(raw.clone(), "preview".to_string());
    let b = PipelineOutputEvidence::new(raw.clone(), "different preview".to_string());
    // Digest covers the RAW evidence, not the projection.
    assert_eq!(a.audit_digest, b.audit_digest);
    assert_eq!(a.audit_digest.len(), 64);
    let empty = PipelineOutputEvidence::empty();
    assert_eq!(empty.digest(), AuditDigest::empty_digest());
    let _ = empty;
}

#[tokio::test]
async fn artifacts_carry_explicit_classification() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tmp/artifact-class-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let store = FsArtifactStore::new(&dir);
    let raw = tool_output_with_secrets();
    let id = ArtifactId::new();
    let path = store
        .store_classified(
            id,
            raw.as_bytes(),
            "txt",
            EvidenceClassification::RawPrivileged,
        )
        .await
        .expect("classified store must succeed");
    assert!(path.exists());
    // Sidecar records the classification explicitly.
    let sidecar = dir.join(format!("{id}.txt.classification"));
    assert!(sidecar.exists(), "classification sidecar must exist");
    let body = std::fs::read_to_string(&sidecar).expect("read sidecar");
    assert!(
        body.contains("raw_privileged"),
        "sidecar must label raw: {body}"
    );
    assert!(body.contains(&id.to_string()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn oversized_tool_output_externalizes_classified() {
    use m31a::pipeline::capture::OutputCaptureManager;
    use std::sync::Arc;
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tmp/capture-class-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(&dir));
    let mgr = OutputCaptureManager::new(64, 4);
    let big = format!(
        "{}\n{}",
        tool_output_with_secrets().repeat(20),
        "x".repeat(5000)
    );
    let preview = mgr
        .process_output(&big, Some(&store))
        .await
        .expect("externalization must succeed");
    assert!(
        preview.contains("Output truncated"),
        "must return preview: {preview}"
    );
    // The preview becomes model-visible evidence → scrubbed at Stage 10.
    let evidence = PipelineOutputEvidence::new(big.clone(), preview);
    for secret in SECRETS {
        assert!(!evidence.model_visible_output.contains(secret));
    }
    // The externalized artifact is labeled raw_privileged.
    let mut found_sidecar = false;
    for entry in std::fs::read_dir(&dir).expect("read dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".classification") {
            let body = std::fs::read_to_string(entry.path()).expect("sidecar");
            assert!(body.contains("raw_privileged"));
            found_sidecar = true;
        }
    }
    assert!(found_sidecar, "expected a classification sidecar");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn persistence_redaction_helpers_hold() {
    // Unit-level proof that the exact functions used at the session/event
    // persistence boundaries scrub hostile values.
    let redactor = m31a::telemetry::SecretRedactor::new();
    let hostile = serde_json::json!({
        "kind": "tool_result_message",
        "output": tool_output_with_secrets(),
        "arguments": {"api_key": "nvapi-should-be-scrubbed-0123456789abcdef"}, // dummy test fixture
    });
    let serialized = serde_json::to_string(&hostile).expect("serialize");
    let scrubbed = redactor.redact_text(&serialized);
    for secret in SECRETS {
        assert!(
            !scrubbed.contains(secret),
            "persistence payload leaked a secret"
        );
    }
    // Structure survives redaction (still valid JSON with the same keys).
    let reparsed: serde_json::Value = serde_json::from_str(&scrubbed).expect("still JSON");
    assert_eq!(reparsed["kind"], "tool_result_message");
}
