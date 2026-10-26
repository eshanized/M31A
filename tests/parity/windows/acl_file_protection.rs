//! Windows: ACL file-protection contract.

use std::path::Path;

#[test]
fn parity_windows_acl_contract_and_native() {
    let desc = m31a::platform::windows::acl::intended_acl_description();
    assert!(!desc.is_empty(), "intended ACL is documented");

    #[cfg(windows)]
    {
        let dir = tempfile::Builder::new()
            .prefix("m31a-p49-acl")
            .tempdir()
            .unwrap();
        let file = dir.path().join("secret.txt");
        std::fs::write(&file, b"secret").unwrap();

        // 1. Positive enforcement
        let outcome = m31a::platform::windows::acl::ensure_private_file_windows(&file)
            .expect("Windows ACL enforcement must succeed on valid file");
        assert_eq!(
            outcome,
            m31a::platform::windows::acl::AclOutcome::Enforced,
            "Windows ACL enforcement must strictly yield Enforced, never Unsupported or Failed"
        );

        // 2. Behavioral verification of applied DACL on disk
        let verified = m31a::platform::windows::acl::verify_private_file_acl_windows(&file)
            .expect("ACL verification must succeed");
        assert!(
            verified,
            "Applied ACL must be verified on disk with protected inheritance and owner/admin control"
        );

        // 3. Negative fail-closed on non-existent path
        let non_existent = dir.path().join("missing.txt");
        let err = m31a::platform::windows::acl::ensure_private_file_windows(&non_existent);
        assert!(
            err.is_err(),
            "Enforcement on non-existent file must fail closed with error"
        );
    }

    #[cfg(not(windows))]
    {
        assert!(
            !m31a::platform::windows::acl::supports_private_files(),
            "Non-Windows host must report supports_private_files == false"
        );
        let outcome = m31a::platform::windows::acl::ensure_private_file_windows(Path::new("dummy"))
            .expect("off-target call must return typed outcome");
        assert_eq!(
            outcome,
            m31a::platform::windows::acl::AclOutcome::Unsupported,
            "Off-target Windows ACL call must return Unsupported, never Enforced or Failed"
        );
        let verify_err = m31a::platform::windows::acl::verify_private_file_acl_windows(Path::new("dummy"));
        assert!(
            verify_err.is_err(),
            "Off-target verification must return error"
        );
    }
}
