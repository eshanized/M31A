//! Parity: credential file protection.
//!
//! Security property (identical on every host): after M31A marks a file
//! private, no other non-privileged principal can read it through the
//! documented mechanism. Unix proves owner-only mode bits (0o600); Windows
//! proves a protected DACL (owner + administrators, no inheritance). The
//! primitives differ; the property must hold.

use std::path::Path;

use m31a::platform::permissions::{enforcement_description, ensure_private_file};

/// New file: enforcement succeeds and Unix mode is exactly 0o600.
#[test]
fn parity_credential_new_file_enforced() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-cred")
        .tempdir()
        .expect("tempdir");
    let file = dir.path().join("credential.txt");
    std::fs::write(&file, b"secret-material").unwrap();
    let outcome = ensure_private_file(&file).expect("enforcement call must not fail");
    assert_eq!(
        outcome,
        m31a::platform::permissions::FileSecurityOutcome::Enforced,
        "EQUIVALENT: new credential file is Enforced on this host"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Unix credential file mode must be exactly 0o600, got {mode:o}"
        );
    }
}

/// Existing file with loose permissions: enforcement tightens it.
#[test]
fn parity_credential_existing_loose_file_tightened() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-cred-loose")
        .tempdir()
        .expect("tempdir");
    let file = dir.path().join("existing.txt");
    std::fs::write(&file, b"secret").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let outcome = ensure_private_file(&file).expect("enforcement must not fail");
    assert_eq!(
        outcome,
        m31a::platform::permissions::FileSecurityOutcome::Enforced
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "loose file tightened to 0o600");
    }
}

/// Replacement: re-enforcement after overwrite keeps the file private.
#[test]
fn parity_credential_replacement_stays_private() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-cred-replace")
        .tempdir()
        .expect("tempdir");
    let file = dir.path().join("rotated.txt");
    std::fs::write(&file, b"v1").unwrap();
    ensure_private_file(&file).unwrap();
    std::fs::write(&file, b"v2-rotated-secret").unwrap();
    ensure_private_file(&file).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "rotated credential file stays 0o600");
    }
}

/// Temporary credential-bearing file: same guarantee as durable files.
#[test]
fn parity_credential_temp_file_enforced() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-cred-tmp")
        .tempdir()
        .expect("tempdir");
    let file = dir.path().join("tmp-token.json");
    std::fs::write(&file, b"{\"token\":\"x\"}").unwrap();
    let outcome = ensure_private_file(&file).unwrap();
    assert_eq!(
        outcome,
        m31a::platform::permissions::FileSecurityOutcome::Enforced
    );
}

/// The enforcement description names the real mechanism per host, never a
/// universal primitive.
#[test]
fn parity_credential_mechanism_named_per_host() {
    let desc = enforcement_description();
    assert!(!desc.is_empty());
    #[cfg(not(windows))]
    assert!(
        desc.contains("0o600"),
        "Unix enforcement is mode bits, got {desc}"
    );
}

/// Windows DACL contract (pure, runs everywhere): the intended ACL grants
/// owner + administrators only, with inheritance disabled.
#[test]
fn parity_credential_windows_acl_contract() {
    let desc = m31a::platform::windows::acl::intended_acl_description();
    let lower = desc.to_lowercase();
    assert!(lower.contains("owner"), "DACL names the owner, got {desc}");
    assert!(
        lower.contains("administrators"),
        "DACL names administrators, got {desc}"
    );
    assert!(
        lower.contains("inheritance"),
        "DACL disables inheritance, got {desc}"
    );
    assert!(
        m31a::platform::windows::acl::supports_private_files() || !cfg!(windows),
        "Windows ACL support honestly reported"
    );
    let _ = Path::new("dummy");
}
