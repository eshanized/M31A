//! Linux: unprivileged user-namespace evidence.

#[test]
fn parity_linux_userns_probe_matches_kernel_fact() {
    #[cfg(target_os = "linux")]
    {
        use m31a::platform::filesystem::HostFilesystem;
        let gate = std::path::Path::new("/proc/sys/kernel/unprivileged_userns_clone");
        let disabled = std::fs::read_to_string(gate).is_ok_and(|c| c.trim() == "0");
        let ns_exists = std::path::Path::new("/proc/self/ns/user").exists();
        assert_eq!(
            HostFilesystem::has_user_namespaces(),
            ns_exists && !disabled,
            "userns probe matches the kernel fact"
        );
    }
    #[cfg(not(target_os = "linux"))]
    {
        assert!(
            !m31a::platform::filesystem::HostFilesystem::has_user_namespaces(),
            "user namespaces are Linux-only"
        );
    }
}
