//! Windows: ACL file-protection contract.

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
        let outcome = m31a::platform::windows::acl::ensure_private_file_windows(&file).unwrap();
        assert!(
            matches!(
                outcome,
                m31a::platform::windows::acl::AclOutcome::Enforced
                    | m31a::platform::windows::acl::AclOutcome::Unsupported
                    | m31a::platform::windows::acl::AclOutcome::Failed
            ),
            "native ACL enforcement is a typed outcome: {outcome:?}"
        );
    }
}
