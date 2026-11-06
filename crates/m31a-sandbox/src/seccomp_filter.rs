//! In-process Linux sandbox primitives: `no_new_privs` and seccomp filters.
//!
//! Provides granular filtering for socket domains (blocking VM sockets `AF_VSOCK`,
//! blocking direct network calls in restricted mode, restricting to AF_UNIX or proxy-routed IP).

use std::collections::BTreeMap;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkSeccompMode {
    /// Disallows raw external network calls (`connect`, `bind`, `listen`, etc.)
    /// and disallows any socket creation except `AF_UNIX`.
    Restricted,
    /// Routed via local proxy: allows `AF_INET` and `AF_INET6`, denies other socket families,
    /// and optionally allows `AF_UNIX`.
    ProxyRouted,
    /// General network allowed, but strictly blocks `AF_VSOCK` to prevent guest-to-host VM socket escape.
    VmSocketRestricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeccompCmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    MaskedEq(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeccompRuleCondition {
    pub arg_index: usize,
    pub op: SeccompCmpOp,
    pub value: u64,
}

impl SeccompRuleCondition {
    pub fn new(arg_index: usize, op: SeccompCmpOp, value: u64) -> Self {
        Self {
            arg_index,
            op,
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeccompAction {
    Allow,
    Errno(u32),
    KillProcess,
    KillThread,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeccompSyscallRule {
    pub syscall_nr: i64,
    pub action: SeccompAction,
    pub conditions: Vec<SeccompRuleCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeccompFilterPlan {
    pub mode: NetworkSeccompMode,
    pub default_action: SeccompAction,
    pub denied_action: SeccompAction,
    pub rules: BTreeMap<i64, Vec<SeccompSyscallRule>>,
}

impl SeccompFilterPlan {
    /// Constructs an architecture-specific seccomp filter plan for the given mode.
    pub fn build(mode: NetworkSeccompMode, allow_all_unix_sockets: bool) -> Self {
        let default_action = SeccompAction::Allow;
        let denied_action = SeccompAction::Errno(libc::EPERM as u32);
        let mut rules: BTreeMap<i64, Vec<SeccompSyscallRule>> = BTreeMap::new();

        let deny_syscall = |nr: i64, map: &mut BTreeMap<i64, Vec<SeccompSyscallRule>>| {
            map.insert(
                nr,
                vec![SeccompSyscallRule {
                    syscall_nr: nr,
                    action: denied_action,
                    conditions: Vec::new(),
                }],
            );
        };

        // Disallow process inspection: upstream always denies ptrace and
        // process_vm read/write in every seccomp mode (§3.2 "Always denies"),
        // including VmSocketRestricted.
        deny_syscall(libc::SYS_ptrace, &mut rules);
        deny_syscall(libc::SYS_process_vm_readv, &mut rules);
        deny_syscall(libc::SYS_process_vm_writev, &mut rules);

        // io_uring can bypass socket() family checks in Linux kernels, so always deny in restricted modes
        deny_syscall(libc::SYS_io_uring_setup, &mut rules);
        deny_syscall(libc::SYS_io_uring_enter, &mut rules);
        deny_syscall(libc::SYS_io_uring_register, &mut rules);

        match mode {
            NetworkSeccompMode::Restricted => {
                deny_syscall(libc::SYS_connect, &mut rules);
                deny_syscall(libc::SYS_accept, &mut rules);
                deny_syscall(libc::SYS_accept4, &mut rules);
                deny_syscall(libc::SYS_bind, &mut rules);
                deny_syscall(libc::SYS_listen, &mut rules);
                deny_syscall(libc::SYS_getpeername, &mut rules);
                deny_syscall(libc::SYS_getsockname, &mut rules);
                deny_syscall(libc::SYS_shutdown, &mut rules);
                deny_syscall(libc::SYS_sendto, &mut rules);
                deny_syscall(libc::SYS_sendmmsg, &mut rules);
                deny_syscall(libc::SYS_recvmmsg, &mut rules);
                deny_syscall(libc::SYS_getsockopt, &mut rules);
                deny_syscall(libc::SYS_setsockopt, &mut rules);

                // Deny socket and socketpair unless arg0 == AF_UNIX
                let condition_not_unix =
                    SeccompRuleCondition::new(0, SeccompCmpOp::Ne, libc::AF_UNIX as u64);
                rules.insert(
                    libc::SYS_socket,
                    vec![SeccompSyscallRule {
                        syscall_nr: libc::SYS_socket,
                        action: denied_action,
                        conditions: vec![condition_not_unix.clone()],
                    }],
                );
                rules.insert(
                    libc::SYS_socketpair,
                    vec![SeccompSyscallRule {
                        syscall_nr: libc::SYS_socketpair,
                        action: denied_action,
                        conditions: vec![condition_not_unix],
                    }],
                );
            }
            NetworkSeccompMode::ProxyRouted => {
                let mut socket_conditions = vec![
                    SeccompRuleCondition::new(0, SeccompCmpOp::Ne, libc::AF_INET as u64),
                    SeccompRuleCondition::new(0, SeccompCmpOp::Ne, libc::AF_INET6 as u64),
                ];
                if allow_all_unix_sockets {
                    socket_conditions.push(SeccompRuleCondition::new(
                        0,
                        SeccompCmpOp::Ne,
                        libc::AF_UNIX as u64,
                    ));
                }
                rules.insert(
                    libc::SYS_socket,
                    vec![SeccompSyscallRule {
                        syscall_nr: libc::SYS_socket,
                        action: denied_action,
                        conditions: socket_conditions,
                    }],
                );
                rules.insert(
                    libc::SYS_socketpair,
                    vec![SeccompSyscallRule {
                        syscall_nr: libc::SYS_socketpair,
                        action: denied_action,
                        conditions: vec![SeccompRuleCondition::new(
                            0,
                            SeccompCmpOp::Ne,
                            libc::AF_UNIX as u64,
                        )],
                    }],
                );
            }
            NetworkSeccompMode::VmSocketRestricted => {
                // Deny only AF_VSOCK (arg0 == AF_VSOCK)
                let condition_vsock =
                    SeccompRuleCondition::new(0, SeccompCmpOp::Eq, libc::AF_VSOCK as u64);
                rules.insert(
                    libc::SYS_socket,
                    vec![SeccompSyscallRule {
                        syscall_nr: libc::SYS_socket,
                        action: denied_action,
                        conditions: vec![condition_vsock.clone()],
                    }],
                );
                rules.insert(
                    libc::SYS_socketpair,
                    vec![SeccompSyscallRule {
                        syscall_nr: libc::SYS_socketpair,
                        action: denied_action,
                        conditions: vec![condition_vsock],
                    }],
                );
            }
        }

        Self {
            mode,
            default_action,
            denied_action,
            rules,
        }
    }
}

/// Sets `PR_SET_NO_NEW_PRIVS` on the current thread so seccomp cannot be bypassed.
#[cfg(target_os = "linux")]
pub fn set_no_new_privs() -> io::Result<()> {
    let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// No-op on non-Linux platforms where seccomp is unavailable.
#[cfg(not(target_os = "linux"))]
pub fn set_no_new_privs() -> io::Result<()> {
    Ok(())
}

/// Resolves the appropriate seccomp filter mode given network and filesystem policy flags.
pub fn determine_seccomp_mode(
    network_enabled: bool,
    proxy_routed: bool,
    has_full_disk_write: bool,
) -> Option<NetworkSeccompMode> {
    if !network_enabled {
        Some(NetworkSeccompMode::Restricted)
    } else if proxy_routed {
        Some(NetworkSeccompMode::ProxyRouted)
    } else if !has_full_disk_write {
        Some(NetworkSeccompMode::VmSocketRestricted)
    } else {
        None
    }
}

/// Resolves the seccomp mode for a sandbox policy on the non-bwrap fallback
/// path (report §3.2).
///
/// Enforces network isolation topology: `ReadOnly` already runs `--unshare-net` under
/// bwrap, so the fallback gets the strictly stronger `Restricted` filter
/// (deliberate hardening, aligned across paths). `WorkspaceWrite` keeps its
/// networking behavior and only adds the always-deny set plus `AF_VSOCK`
/// blocking (`VmSocketRestricted`). `DangerFullAccess` installs nothing.
pub fn seccomp_mode_for_policy(policy: m31a_core::SandboxPolicy) -> Option<NetworkSeccompMode> {
    match policy {
        m31a_core::SandboxPolicy::DangerFullAccess => None,
        m31a_core::SandboxPolicy::ReadOnly => Some(NetworkSeccompMode::Restricted),
        m31a_core::SandboxPolicy::WorkspaceWrite => Some(NetworkSeccompMode::VmSocketRestricted),
    }
}

#[cfg(target_os = "linux")]
fn seccomp_io_error(stage: &str, e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("seccomp {stage}: {e}"))
}

#[cfg(target_os = "linux")]
fn condition_to_seccompiler(c: &SeccompRuleCondition) -> io::Result<seccompiler::SeccompCondition> {
    let arg_index = u8::try_from(c.arg_index).map_err(|_| {
        io::Error::other(format!("seccomp arg index out of range: {}", c.arg_index))
    })?;
    let op = match c.op {
        SeccompCmpOp::Eq => seccompiler::SeccompCmpOp::Eq,
        SeccompCmpOp::Ne => seccompiler::SeccompCmpOp::Ne,
        SeccompCmpOp::Lt => seccompiler::SeccompCmpOp::Lt,
        SeccompCmpOp::Le => seccompiler::SeccompCmpOp::Le,
        SeccompCmpOp::Gt => seccompiler::SeccompCmpOp::Gt,
        SeccompCmpOp::Ge => seccompiler::SeccompCmpOp::Ge,
        SeccompCmpOp::MaskedEq(mask) => seccompiler::SeccompCmpOp::MaskedEq(mask),
    };
    seccompiler::SeccompCondition::new(arg_index, seccompiler::SeccompCmpArgLen::Qword, op, c.value)
        .map_err(|e| seccomp_io_error("condition", e))
}

/// Compiles a [`SeccompFilterPlan`] into an installable BPF program.
///
/// Runs in the PARENT process: compilation happens once per spawn, and the
/// resulting program is installed in the child between `fork` and `exec` via
/// [`pre_exec_seccomp_hook`]. The plan's deny-list shape maps onto
/// seccompiler as: conditions within a rule are ANDed (deny unless arg is in
/// the allowed set), rules for one syscall are OR'd, every rule must carry the
/// plan's `denied_action`, and the global default is the plan's `default_action`.
#[cfg(target_os = "linux")]
pub fn compile_plan_to_bpf(plan: &SeccompFilterPlan) -> io::Result<seccompiler::BpfProgram> {
    if plan.default_action != SeccompAction::Allow {
        return Err(io::Error::other(
            "seccomp plan: only Allow default is installable",
        ));
    }
    let match_action = match plan.denied_action {
        SeccompAction::Errno(code) => seccompiler::SeccompAction::Errno(code),
        SeccompAction::KillProcess => seccompiler::SeccompAction::KillProcess,
        SeccompAction::KillThread => seccompiler::SeccompAction::KillThread,
        SeccompAction::Allow => {
            return Err(io::Error::other(
                "seccomp plan: denied_action must deny; Allow would be a no-op",
            ));
        }
    };

    let mut rules: std::collections::BTreeMap<i64, Vec<seccompiler::SeccompRule>> =
        std::collections::BTreeMap::new();
    for (nr, chain) in &plan.rules {
        // Every rule must carry the plan's denied action before any
        // translation (a mixed plan has no single match_action).
        if chain.iter().any(|r| r.action != plan.denied_action) {
            return Err(io::Error::other(format!(
                "seccomp plan: mixed actions for syscall {nr}"
            )));
        }
        // An unconditional deny (no conditions) cannot be expressed as a
        // SeccompRule — seccompiler requires non-empty condition vectors.
        // Since chain rules are OR'd, any unconditional rule denies the
        // syscall outright: encode as an empty chain (match → match_action).
        if chain.iter().any(|r| r.conditions.is_empty()) {
            rules.insert(*nr, Vec::new());
            continue;
        }
        let mut compiled_chain = Vec::with_capacity(chain.len());
        for r in chain {
            let mut conditions = Vec::with_capacity(r.conditions.len());
            for c in &r.conditions {
                conditions.push(condition_to_seccompiler(c)?);
            }
            compiled_chain.push(
                seccompiler::SeccompRule::new(conditions)
                    .map_err(|e| seccomp_io_error("rule", e))?,
            );
        }
        rules.insert(*nr, compiled_chain);
    }

    let filter = seccompiler::SeccompFilter::new(
        rules,
        seccompiler::SeccompAction::Allow, // mismatch (default allow-list posture)
        match_action,
        seccompiler::TargetArch::try_from(std::env::consts::ARCH)
            .map_err(|e| seccomp_io_error("arch", e))?,
    )
    .map_err(|e| seccomp_io_error("filter", e))?;

    seccompiler::BpfProgram::try_from(filter).map_err(|e| seccomp_io_error("bpf compile", e))
}

/// Builds a `pre_exec` closure that installs the compiled BPF filter in the
/// child between fork and exec (the in-process equivalent of upstream's
/// two-stage inner launcher).
///
/// Compilation happens here in the PARENT; failures surface as hard spawn
/// errors before any process is created. The closure first re-asserts
/// `no_new_privs` (the Landlock hook may already have set it; harmless to
/// repeat), then installs the filter with `SYS_seccomp`.
#[cfg(target_os = "linux")]
pub fn pre_exec_seccomp_hook(
    plan: &SeccompFilterPlan,
) -> io::Result<impl FnMut() -> io::Result<()> + Send + 'static> {
    let bpf = compile_plan_to_bpf(plan)?;
    Ok(move || {
        set_no_new_privs()?;
        seccompiler::apply_filter(&bpf).map_err(|e| seccomp_io_error("install", e))?;
        Ok(())
    })
}

/// Non-Linux fallback for pre-exec seccomp hook.
#[cfg(not(target_os = "linux"))]
pub fn pre_exec_seccomp_hook(
    _plan: &SeccompFilterPlan,
) -> io::Result<impl FnMut() -> io::Result<()> + Send + 'static> {
    Ok(|| Ok(()))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    #[test]
    fn test_determine_seccomp_mode_logic() {
        assert_eq!(
            determine_seccomp_mode(false, false, true),
            Some(NetworkSeccompMode::Restricted)
        );
        assert_eq!(
            determine_seccomp_mode(true, true, true),
            Some(NetworkSeccompMode::ProxyRouted)
        );
        assert_eq!(
            determine_seccomp_mode(true, false, false),
            Some(NetworkSeccompMode::VmSocketRestricted)
        );
        assert_eq!(determine_seccomp_mode(true, false, true), None);
    }

    #[test]
    fn test_seccomp_filter_plan_restricted() {
        let plan = SeccompFilterPlan::build(NetworkSeccompMode::Restricted, false);
        assert!(plan.rules.contains_key(&libc::SYS_connect));
        assert!(plan.rules.contains_key(&libc::SYS_bind));
        assert!(plan.rules.contains_key(&libc::SYS_io_uring_setup));
        assert!(plan.rules.contains_key(&libc::SYS_ptrace));
        assert!(plan.rules.contains_key(&libc::SYS_socket));
        let socket_rules = &plan.rules[&libc::SYS_socket];
        assert_eq!(socket_rules.len(), 1);
        assert_eq!(socket_rules[0].conditions[0].op, SeccompCmpOp::Ne);
        assert_eq!(socket_rules[0].conditions[0].value, libc::AF_UNIX as u64);
    }

    #[test]
    fn test_seccomp_filter_plan_vsock_restricted() {
        let plan = SeccompFilterPlan::build(NetworkSeccompMode::VmSocketRestricted, false);
        assert!(!plan.rules.contains_key(&libc::SYS_connect));
        assert!(plan.rules.contains_key(&libc::SYS_io_uring_setup));
        assert!(plan.rules.contains_key(&libc::SYS_socket));
        let socket_rules = &plan.rules[&libc::SYS_socket];
        assert_eq!(socket_rules.len(), 1);
        assert_eq!(socket_rules[0].conditions[0].op, SeccompCmpOp::Eq);
        assert_eq!(socket_rules[0].conditions[0].value, libc::AF_VSOCK as u64);
    }

    #[test]
    fn test_always_deny_set_present_in_every_mode() {
        // §3.2: ptrace / process_vm_* / io_uring_* are denied in ALL modes
        // (upstream "Always denies"), including VmSocketRestricted.
        for mode in [
            NetworkSeccompMode::Restricted,
            NetworkSeccompMode::ProxyRouted,
            NetworkSeccompMode::VmSocketRestricted,
        ] {
            let plan = SeccompFilterPlan::build(mode, true);
            for nr in [
                libc::SYS_ptrace,
                libc::SYS_process_vm_readv,
                libc::SYS_process_vm_writev,
                libc::SYS_io_uring_setup,
                libc::SYS_io_uring_enter,
                libc::SYS_io_uring_register,
            ] {
                assert!(
                    plan.rules.contains_key(&nr),
                    "{mode:?} missing always-deny syscall {nr}"
                );
            }
        }
    }

    #[test]
    fn test_seccomp_mode_for_policy() {
        use m31a_core::SandboxPolicy;
        assert_eq!(
            seccomp_mode_for_policy(SandboxPolicy::DangerFullAccess),
            None
        );
        assert_eq!(
            seccomp_mode_for_policy(SandboxPolicy::ReadOnly),
            Some(NetworkSeccompMode::Restricted)
        );
        assert_eq!(
            seccomp_mode_for_policy(SandboxPolicy::WorkspaceWrite),
            Some(NetworkSeccompMode::VmSocketRestricted)
        );
    }

    #[test]
    fn test_compile_plan_to_bpf_all_modes() {
        for mode in [
            NetworkSeccompMode::Restricted,
            NetworkSeccompMode::ProxyRouted,
            NetworkSeccompMode::VmSocketRestricted,
        ] {
            let plan = SeccompFilterPlan::build(mode, true);
            let bpf = compile_plan_to_bpf(&plan)
                .unwrap_or_else(|e| panic!("compile failed for {mode:?}: {e}"));
            assert!(!bpf.is_empty(), "BPF program empty for {mode:?}");
        }
    }

    #[test]
    fn test_compile_plan_rejects_bad_shape() {
        // Allow denied_action would be a no-op deny-list → rejected.
        let mut plan = SeccompFilterPlan::build(NetworkSeccompMode::VmSocketRestricted, true);
        plan.denied_action = SeccompAction::Allow;
        assert!(compile_plan_to_bpf(&plan).is_err());

        // Mixed actions within a plan → rejected.
        let mut plan = SeccompFilterPlan::build(NetworkSeccompMode::VmSocketRestricted, true);
        let nr = libc::SYS_ptrace;
        plan.rules.get_mut(&nr).unwrap()[0].action = SeccompAction::KillProcess;
        assert!(compile_plan_to_bpf(&plan).is_err());

        // Non-Allow default → rejected.
        let mut plan = SeccompFilterPlan::build(NetworkSeccompMode::VmSocketRestricted, true);
        plan.default_action = SeccompAction::Errno(1);
        assert!(compile_plan_to_bpf(&plan).is_err());
    }

    #[test]
    fn test_set_no_new_privs_idempotent() {
        // Can safely be called on Linux
        let res = set_no_new_privs();
        assert!(res.is_ok());
    }

    // --- Real kernel enforcement ------------------------------------------
    //
    // The self-test child re-executes this test binary filtered by the probe
    // name. When M31A_SECCOMP_CHILD is set, the named test performs raw
    // syscalls and prints one `M31A_PROBE` line; the parent asserts the
    // expected errnos per mode.

    const PROBE_TEST_NAME: &str = "seccomp_selftest_child";

    #[test]
    fn seccomp_selftest_child() {
        let Ok(mode) = std::env::var("M31A_SECCOMP_CHILD") else {
            return; // no-op in normal test runs
        };

        // ptrace(TRACEME): EPERM under the always-deny set; harmless when
        // the filter is absent (control environments never set the env var).
        let ptrace_rc = unsafe {
            libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };

        let probe_socket = |domain: i32| {
            let fd = unsafe { libc::socket(domain, libc::SOCK_STREAM, 0) };
            let errno = if fd < 0 {
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
            } else {
                unsafe { libc::close(fd) };
                0
            };
            (fd, errno)
        };
        let (_unix_fd, unix_errno) = probe_socket(libc::AF_UNIX);
        let (_inet_fd, inet_errno) = probe_socket(libc::AF_INET);
        let (_vsock_fd, vsock_errno) = probe_socket(libc::AF_VSOCK);

        println!(
            "M31A_PROBE ptrace_rc={ptrace_rc} ptrace_errno={} unix_errno={unix_errno} inet_errno={inet_errno} vsock_errno={vsock_errno} mode={mode}",
            if ptrace_rc < 0 {
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
            } else {
                0
            }
        );
    }

    fn run_probe_child(plan: &SeccompFilterPlan) -> Vec<(String, i64)> {
        let hook = pre_exec_seccomp_hook(plan).expect("compile probe plan");
        let mut cmd = std::process::Command::new(std::env::current_exe().expect("current_exe"));
        cmd.args([PROBE_TEST_NAME, "--nocapture", "--test-threads=1"])
            .env("M31A_SECCOMP_CHILD", "1")
            .stdin(std::process::Stdio::null());
        unsafe {
            cmd.pre_exec(hook);
        }
        let out = cmd.output().expect("spawn probe child");
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout
            .lines()
            .filter(|l| l.contains("M31A_PROBE"))
            .flat_map(|l| {
                l.split_whitespace()
                    .filter_map(|tok| tok.split_once('='))
                    .filter_map(|(k, v)| v.parse::<i64>().ok().map(|n| (k.to_string(), n)))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn test_real_kernel_seccomp_enforcement() {
        let ex = std::env::current_exe();
        let exe = match ex {
            Ok(p) => p,
            Err(_) => return,
        };
        if !exe.exists() {
            eprintln!("test binary path unavailable — skipping enforcement test");
            return;
        }

        // VmSocketRestricted: vsock denied, ptrace denied, unix+inet allowed.
        let plan = SeccompFilterPlan::build(NetworkSeccompMode::VmSocketRestricted, true);
        let probe: std::collections::HashMap<String, i64> =
            run_probe_child(&plan).into_iter().collect();
        assert!(
            !probe.is_empty(),
            "no probe output from filtered child (harness died?)"
        );
        assert_eq!(probe["unix_errno"], 0, "AF_UNIX must stay allowed");
        assert_eq!(probe["inet_errno"], 0, "AF_INET must stay allowed");
        assert_eq!(
            probe["vsock_errno"],
            libc::EPERM as i64,
            "AF_VSOCK must get EPERM"
        );
        assert_eq!(
            probe["ptrace_errno"],
            libc::EPERM as i64,
            "ptrace must get EPERM"
        );

        // Restricted: inet denied too, unix allowed.
        let plan = SeccompFilterPlan::build(NetworkSeccompMode::Restricted, true);
        let probe: std::collections::HashMap<String, i64> =
            run_probe_child(&plan).into_iter().collect();
        assert!(!probe.is_empty(), "no probe output from filtered child");
        assert_eq!(probe["unix_errno"], 0, "AF_UNIX must stay allowed");
        assert_eq!(
            probe["inet_errno"],
            libc::EPERM as i64,
            "AF_INET must get EPERM"
        );
    }
}
