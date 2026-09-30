//! Windows: cmd.exe / PowerShell quoting parity.

#[test]
fn parity_windows_shell_quoting_matrix() {
    use m31a::platform::windows::shell as wshell;
    assert_eq!(wshell::preferred_backend(), wshell::ShellBackend::Cmd);
    assert_eq!(wshell::ShellBackend::Cmd.program(), "cmd.exe");
    assert_eq!(wshell::ShellBackend::PowerShell.program(), "powershell.exe");
    assert_eq!(
        wshell::ShellBackend::Cmd.wrapper_args(),
        &["/D", "/S", "/C"]
    );
    assert_eq!(wshell::quote_cmd_arg("simple"), "simple");
    assert_eq!(wshell::quote_cmd_arg("a b"), "\"a b\"");
    assert_eq!(wshell::quote_cmd_arg(""), "\"\"");
    assert_eq!(wshell::quote_powershell_arg("safe-name"), "safe-name");
    assert_eq!(wshell::quote_powershell_arg(""), "''");
    assert_eq!(wshell::quote_powershell_arg("it's"), "'it''s'");
    assert!(wshell::is_windows_shell_program("cmd"));
    assert!(wshell::is_windows_shell_program("pwsh.exe"));
    assert!(!wshell::is_windows_shell_program("python.exe"));
    assert!(wshell::contains_cmd_metachars("a|b"));
    assert!(wshell::contains_cmd_metachars("%VAR%"));
    assert!(wshell::contains_powershell_metachars("`$x"));
    let cwd = std::env::temp_dir();
    let cmd = wshell::build_cmd_command("echo hi", &cwd);
    let _ = format!("{cmd:?}");
    let ps = wshell::build_powershell_command("Get-Location", &cwd);
    let _ = format!("{ps:?}");
}
