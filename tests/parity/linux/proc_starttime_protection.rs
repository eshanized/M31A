//! Linux: /proc start-time identity protection.
//!
//! The strongest deterministic check available without forcing a kernel
//! PID-recycling event: the live process has a stable start time, a dead
//! PID loses it, and two distinct spawns never share (pid, starttime).

#[test]
fn parity_linux_starttime_stable_for_live_process() {
    #[cfg(target_os = "linux")]
    {
        let pid = std::process::id();
        let a = m31a::platform::process::read_process_starttime(pid);
        let b = m31a::platform::process::read_process_starttime(pid);
        assert!(a.is_some(), "live process has a start-time identity");
        assert_eq!(a, b, "identity is stable across reads");
        assert!(m31a::platform::process::supports_starttime_protection());
    }
    #[cfg(not(target_os = "linux"))]
    {
        assert!(
            m31a::platform::process::read_process_starttime(1).is_none()
                || cfg!(target_os = "macos"),
            "off-Linux the /proc identity is unprovable (None), never guessed"
        );
    }
}

#[test]
fn parity_linux_dead_pid_loses_identity_simulation() {
    #[cfg(target_os = "linux")]
    {
        let mut child = std::process::Command::new("true")
            .spawn()
            .expect("spawn true");
        let pid = child.id();
        let before = m31a::platform::process::read_process_starttime(pid);
        assert!(before.is_some(), "spawned child has an identity");
        child.wait().expect("reap");
        // After reap the PID is free: either None or a different identity.
        // Recorded as simulation because the kernel may not have recycled it.
        let after = m31a::platform::process::read_process_starttime(pid);
        assert!(
            after.is_none() || after != before,
            "SIMULATION: dead PID {pid} no longer carries the old identity"
        );
    }
}
