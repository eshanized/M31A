use m31a_core::SandboxPolicy;
use m31a_sandbox::SandboxRunner;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_adversarial_env_secret_access_denied() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Write sensitive .env file
    let env_file = workspace.join(".env");
    fs::write(
        &env_file,
        "SECRET_DATABASE_PASSWORD=classified_production_secret",
    )
    .unwrap();

    // Attempt to read .env under WorkspaceWrite
    let out = runner
        .execute(
            "cat .env",
            workspace,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        !out.stdout.contains("classified_production_secret"),
        "Adversarial breach: Sandboxed process read .env secret: {}",
        out.stdout
    );
}

#[tokio::test]
async fn test_adversarial_env_variant_secret_access_denied() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Write sensitive .env.production file
    let env_prod = workspace.join(".env.production");
    fs::write(&env_prod, "AWS_ACCESS_KEY_ID=AKIA_SUPER_SECRET").unwrap();

    let out = runner
        .execute(
            "cat .env.production",
            workspace,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        !out.stdout.contains("AKIA_SUPER_SECRET"),
        "Adversarial breach: Sandboxed process read .env.production secret: {}",
        out.stdout
    );
}

#[tokio::test]
async fn test_adversarial_ssh_key_access_denied() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Create nested .ssh key in workspace
    let ssh_dir = workspace.join(".ssh");
    fs::create_dir_all(&ssh_dir).unwrap();
    let key_file = ssh_dir.join("id_rsa");
    fs::write(
        &key_file,
        "-----BEGIN RSA PRIVATE KEY-----\nsuper_secret_ssh_key\n-----END RSA PRIVATE KEY-----",
    )
    .unwrap();

    let out = runner
        .execute(
            "cat .ssh/id_rsa",
            workspace,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        !out.stdout.contains("super_secret_ssh_key"),
        "Adversarial breach: Sandboxed process read SSH private key: {}",
        out.stdout
    );
}

#[tokio::test]
async fn test_adversarial_read_only_filesystem_write_denied() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Attempt to write a file in ReadOnly mode
    let out = runner
        .execute(
            "touch breach.txt",
            workspace,
            SandboxPolicy::ReadOnly,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        out.exit_code != 0 || !workspace.join("breach.txt").exists(),
        "Adversarial breach: ReadOnly sandbox permitted writing file to disk"
    );
    assert!(!workspace.join("breach.txt").exists());
}

#[tokio::test]
async fn test_adversarial_symlink_traversal_to_masked_secret() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    let secret_file = workspace.join(".env");
    fs::write(&secret_file, "API_TOKEN=super_token_999").unwrap();

    // Create symlink to .env
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link_file = workspace.join("symlink_to_env");
        symlink(&secret_file, &link_file).unwrap();

        let out = runner
            .execute(
                "cat symlink_to_env",
                workspace,
                SandboxPolicy::WorkspaceWrite,
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        assert!(
            !out.stdout.contains("super_token_999"),
            "Adversarial breach: Symlink traversal bypassed .env masking: {}",
            out.stdout
        );
    }
}

#[tokio::test]
async fn test_adversarial_network_access_blocked_in_read_only() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Attempt outbound network connection in ReadOnly (unshare-net)
    let out = runner
        .execute(
            "python3 -c \"import socket; s = socket.socket(); s.connect(('1.1.1.1', 80))\"",
            workspace,
            SandboxPolicy::ReadOnly,
            Duration::from_secs(5),
        )
        .await;

    // Must fail or produce network error
    if let Ok(cmd_out) = out {
        assert!(
            cmd_out.exit_code != 0,
            "Network connection should fail under ReadOnly sandbox"
        );
    }
}

#[tokio::test]
async fn test_adversarial_extra_ro_mount_cannot_override_masked_env_secret() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Write secret files with naming variants
    let env_file = workspace.join(".env");
    fs::write(&env_file, "SECRET_KEY=adversarial_override_attempt_1").unwrap();
    let envrc_file = workspace.join(".envrc");
    fs::write(&envrc_file, "DIRENV_KEY=adversarial_override_attempt_2").unwrap();

    // Adversary adds the workspace itself as an extra read-only mount
    // to attempt to override the tmpfs/null mask
    runner.add_protected_path(workspace.to_path_buf());

    let out1 = runner
        .execute(
            "cat .env",
            workspace,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        !out1.stdout.contains("adversarial_override_attempt_1"),
        "Adversarial breach: extra_ro_paths overrode .env mask: {}",
        out1.stdout
    );

    let out2 = runner
        .execute(
            "cat .envrc",
            workspace,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        !out2.stdout.contains("adversarial_override_attempt_2"),
        "Adversarial breach: extra_ro_paths overrode .envrc mask: {}",
        out2.stdout
    );
}

#[tokio::test]
async fn test_adversarial_git_worktree_hooks_masked() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let main_repo = temp.path().join("main_repo");
    let worktree_dir = temp.path().join("worktree_wt");
    fs::create_dir_all(&main_repo).unwrap();
    fs::create_dir_all(&worktree_dir).unwrap();

    // Simulate git worktree setup
    let git_common = main_repo.join(".git");
    let hooks_dir = git_common.join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    let hook_file = hooks_dir.join("pre-commit");
    fs::write(&hook_file, "#!/bin/sh\necho malicious_worktree_hook\n").unwrap();

    let wt_gitdir = git_common.join("worktrees").join("wt");
    fs::create_dir_all(&wt_gitdir).unwrap();
    fs::write(wt_gitdir.join("commondir"), "../..\n").unwrap();

    // Worktree .git file pointer
    let dot_git_file = worktree_dir.join(".git");
    fs::write(&dot_git_file, format!("gitdir: {}\n", wt_gitdir.display())).unwrap();

    let out = runner
        .execute(
            &format!("cat {} 2>&1 || true", hook_file.display()),
            &worktree_dir,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    assert!(
        !out.stdout.contains("malicious_worktree_hook"),
        "Adversarial breach: Worktree failed to mask common git hooks: {}",
        out.stdout
    );
}

#[tokio::test]
async fn test_adversarial_synthetic_mount_prevents_uncreated_hook_injection() {
    let runner = SandboxRunner::new();
    if !runner.has_bwrap() {
        eprintln!("Skipping bwrap test: bwrap not available");
        return;
    }

    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Attempt to inject a new .git/hooks directory and malicious script inside WorkspaceWrite
    let out = runner
        .execute(
            "mkdir -p .git/hooks 2>/dev/null; echo 'echo injected' > .git/hooks/pre-commit 2>&1 || true",
            workspace,
            SandboxPolicy::WorkspaceWrite,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    let hook_path = workspace.join(".git").join("hooks").join("pre-commit");
    assert!(
        !hook_path.exists(),
        "Adversarial breach: Child injected git hook into workspace: {:?}",
        out
    );
}
