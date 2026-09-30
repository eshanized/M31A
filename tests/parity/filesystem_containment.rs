//! Parity: filesystem containment.
//!
//! Semantic contract: a candidate path that escapes the workspace must be
//! rejected on every host. Unix proves this by canonicalization (symlinks
//! resolved); Windows proves it by normalized component comparison plus
//! volume/UNC handling. Normal-path agreement alone is not parity; the
//! adversarial cases below are.

use std::path::Path;

use m31a::platform::filesystem::HostFilesystem;
use m31a::platform::windows::path as winpath;

fn fixture_workspace(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .expect("parity fixture workspace must be creatable")
}

/// Given: workspace root and a direct child. Expected: containment accepted.
#[test]
fn parity_containment_child_accepted() {
    let ws = fixture_workspace("m31a-p49-ws");
    std::fs::create_dir_all(ws.path().join("sub")).unwrap();
    let child = ws.path().join("sub").join("file.txt");
    std::fs::write(&child, b"x").unwrap();
    let ok = HostFilesystem::validate_containment(ws.path(), &child);
    assert!(
        ok.is_ok(),
        "EQUIVALENT: direct child stays contained, got {ok:?}"
    );
}

/// Given: a sibling of the workspace. Expected: rejected.
#[test]
fn parity_containment_sibling_rejected() {
    let parent = fixture_workspace("m31a-p49-parent");
    let ws = parent.path().join("work");
    let sibling = parent.path().join("evil.txt");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(&sibling, b"x").unwrap();
    let err = HostFilesystem::validate_containment(&ws, &sibling);
    assert!(
        err.is_err(),
        "EQUIVALENT: sibling escapes the workspace and must be rejected"
    );
}

/// Common-prefix attack (`work-evil` vs `work`): string prefix is not
/// containment. Rejected on every host.
#[test]
fn parity_containment_prefix_attack_rejected() {
    let parent = fixture_workspace("m31a-p49-prefix");
    let ws = parent.path().join("work");
    let evil = parent.path().join("work-evil").join("f.txt");
    std::fs::create_dir_all(evil.parent().unwrap()).unwrap();
    std::fs::write(&evil, b"x").unwrap();
    std::fs::create_dir_all(&ws).unwrap();
    assert!(
        HostFilesystem::validate_containment(&ws, &evil).is_err(),
        "EQUIVALENT: prefix trick work-evil must not pass as work/"
    );
    assert!(
        !winpath::is_within_workspace("C:\\work", "C:\\work-evil\\file.txt"),
        "EQUIVALENT: Windows component comparison also rejects the prefix trick"
    );
    assert!(winpath::is_within_workspace(
        "C:\\work",
        "C:\\work\\file.txt"
    ));
}

/// `..` traversal escapes: rejected.
#[test]
fn parity_containment_dotdot_rejected() {
    let parent = fixture_workspace("m31a-p49-dotdot");
    let ws = parent.path().join("work");
    std::fs::create_dir_all(&ws).unwrap();
    let esc = ws.join("..").join("evil.txt");
    let parent_evil = parent.path().join("evil.txt");
    std::fs::write(&parent_evil, b"x").unwrap();
    assert!(
        HostFilesystem::validate_containment(&ws, &esc).is_err(),
        "EQUIVALENT: .. traversal must be rejected"
    );
    assert!(!winpath::is_within_workspace(
        "C:\\work",
        "C:\\work\\..\\evil\\file.txt"
    ));
}

/// Absolute escape path: rejected.
#[test]
fn parity_containment_absolute_escape_rejected() {
    let ws = fixture_workspace("m31a-p49-abs");
    let outside = fixture_workspace("m31a-p49-outside");
    let target = outside.path().join("secret.txt");
    std::fs::write(&target, b"x").unwrap();
    assert!(
        HostFilesystem::validate_containment(ws.path(), &target).is_err(),
        "EQUIVALENT: absolute path outside the workspace is rejected"
    );
}

/// Symlink inside the workspace pointing outside: Unix canonicalization
/// must reject; the Windows lexical layer flags it for canonicalization.
#[test]
fn parity_containment_symlink_escape_rejected() {
    #[cfg(unix)]
    {
        let ws = fixture_workspace("m31a-p49-link");
        let outside = fixture_workspace("m31a-p49-link-out");
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, b"secret").unwrap();
        let link = ws.path().join("evil-link.txt");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        assert!(
            HostFilesystem::validate_containment(ws.path(), &link).is_err(),
            "EQUIVALENT(unix): symlink escape resolves outside and is rejected"
        );
    }
    let probe = Path::new("c:\\work\\sub\\..\\evil");
    assert!(
        winpath::needs_canonicalization(probe),
        "Windows flags dotdot paths for canonicalization before deciding"
    );
}

/// Nested symlink (link -> dir containing link -> outside): rejected.
#[test]
fn parity_containment_nested_symlink_rejected() {
    #[cfg(unix)]
    {
        let ws = fixture_workspace("m31a-p49-nlink");
        let outside = fixture_workspace("m31a-p49-nlink-out");
        std::fs::write(outside.path().join("s.txt"), b"s").unwrap();
        let inner = ws.path().join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::os::unix::fs::symlink(outside.path(), inner.join("hop")).unwrap();
        let target = inner.join("hop").join("s.txt");
        assert!(
            HostFilesystem::validate_containment(ws.path(), &target).is_err(),
            "nested symlink hops resolving outside are rejected"
        );
    }
}

/// Missing path inside the workspace: accepted-or-typed (no panic); missing
/// path outside: rejected where resolvable.
#[test]
fn parity_containment_missing_path_typed() {
    let ws = fixture_workspace("m31a-p49-missing");
    let missing_inside = ws.path().join("not-yet-created.txt");
    let _ = HostFilesystem::validate_containment(ws.path(), &missing_inside);
    let outside_missing = std::env::temp_dir().join("m31a-p49-definitely-outside-xyz.txt");
    if outside_missing.exists() {
        assert!(HostFilesystem::validate_containment(ws.path(), &outside_missing).is_err());
    }
}

/// Case differences: Unix is case-sensitive, Windows is not. The difference
/// is intentional and typed per host — SEMANTICALLY DIFFERENT by design.
#[test]
fn parity_containment_case_rules_differ_by_host() {
    assert!(
        !HostFilesystem::paths_identical(Path::new("A"), Path::new("a")) || cfg!(windows),
        "Unix identity is case-sensitive"
    );
    assert!(winpath::paths_identical_windows(
        Path::new("A"),
        Path::new("a")
    ));
    assert!(winpath::is_within_workspace(
        "C:\\Work",
        "c:\\work\\SUB\\file.txt"
    ));
}

/// Windows volume/UNC rules: drive mismatch and host mismatch never contain.
#[test]
fn parity_windows_volume_unc_rules() {
    assert!(!winpath::is_within_workspace(
        "C:\\work",
        "D:\\work\\file.txt"
    ));
    assert!(!winpath::is_within_workspace(
        "\\\\srv\\share\\work",
        "\\\\other\\share\\work\\f"
    ));
    assert!(winpath::is_within_workspace(
        "\\\\srv\\share\\work",
        "\\\\srv\\share\\work\\sub\\f"
    ));
}

/// Windows reserved device names can never name ordinary workspace files.
#[test]
fn parity_windows_reserved_names_rejected() {
    assert!(winpath::is_reserved_name(Path::new("NUL")));
    assert!(winpath::is_reserved_name(Path::new("com1.txt")));
    assert!(!winpath::is_reserved_name(Path::new("normal.txt")));
    let ws = Path::new("C:\\work");
    assert!(
        winpath::validate_containment(ws, Path::new("NUL")).is_err(),
        "reserved names fail closed"
    );
}

/// Separator mixing (`/` vs `\`) is unified before comparison.
#[test]
fn parity_windows_separator_mixing_unified() {
    assert!(winpath::is_within_workspace(
        "C:\\work",
        "C:/work/sub/file.txt"
    ));
    assert_eq!(winpath::normalize_separators("a/b\\c"), "a\\b\\c");
}
