//! Windows: reparse-point / junction / symlink containment.
//!
//! Security-critical. The lexical component comparison defeats prefix,
//! case, traversal, volume, UNC, reserved-name, and separator attacks on
//! every host. Full reparse-point resolution needs privileged native APIs;
//! that residual is recorded as a GAP in the security parity audit, not
//! hidden by these tests.

use std::path::Path;

use m31a::platform::windows::path as winpath;

#[test]
fn parity_windows_reparse_lexical_barrier() {
    assert!(!winpath::is_within_workspace(
        "C:\\work",
        "C:\\work-evil\\file.txt"
    ));
    assert!(!winpath::is_within_workspace(
        "C:\\work",
        "C:\\work\\..\\evil\\file.txt"
    ));
    assert!(winpath::is_within_workspace(
        "C:\\work",
        "C:\\work\\sub\\..\\sub\\file.txt"
    ));
    assert!(!winpath::is_within_workspace(
        "C:\\work",
        "D:\\work\\file.txt"
    ));
    assert!(!winpath::is_within_workspace(
        "\\\\srv\\share\\work",
        "\\\\srv\\share\\other\\f"
    ));
    assert!(winpath::is_within_workspace(
        "C:\\Work",
        "c:\\WORK\\Sub\\F.TXT"
    ));
    assert!(winpath::is_reserved_name(Path::new("NUL")));
    assert!(winpath::is_reserved_name(Path::new("aux.rs")));
    assert_eq!(
        winpath::lexical_normalize_windows("C:/a/./b/../c"),
        "C:\\a\\c"
    );
    assert!(winpath::is_absolute_windows_path("C:\\x"));
    assert!(winpath::is_unc_path("\\\\srv\\share"));
    assert_eq!(winpath::drive_letter("d:\\x"), Some('D'));
    assert_eq!(winpath::strip_extended_prefix("\\\\?\\C:\\x"), "C:\\x");
}

#[test]
fn parity_windows_junction_escape_flagged_for_canonicalization() {
    // A junction target cannot be resolved lexically; the module must flag
    // such paths so callers canonicalize with privilege instead of trusting
    // the lexical answer for reparse points.
    assert!(winpath::needs_canonicalization(Path::new(
        "C:\\work\\link-to-elsewhere\\file.txt"
    )));
    assert!(winpath::needs_canonicalization(Path::new(
        "\\\\srv\\share\\work\\f"
    )));
}
