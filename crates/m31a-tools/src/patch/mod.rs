//! Multi-file patch management and diff application module.
//!
//! Exports patch execution algorithms, parser interfaces, and file mutation transactions.

pub mod engine;
pub mod parser;
pub mod seek_sequence;
pub mod text_file;

pub use engine::{PatchResult, apply_patch_to_workspace};
pub use parser::{ApplyPatchArgs, Hunk, ParseError, UpdateFileChunk, parse_patch};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_and_apply_patch_add_update_delete() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("hello.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();

        let patch_text = r#"*** Begin Patch
*** Update File: hello.rs
@@ fn main()
-    println!("Hello");
+    println!("Hello, World!");
+    println!("M31A Patching Engine");
*** Add File: notes.txt
+First note
+Second note
*** End Patch"#;

        let args = parse_patch(patch_text).expect("Failed to parse valid patch");
        assert_eq!(args.hunks.len(), 2);

        let res = apply_patch_to_workspace(dir.path(), &args).expect("Failed to apply patch");
        assert_eq!(res.files_modified.len(), 1);
        assert_eq!(res.files_created.len(), 1);

        let updated_contents = std::fs::read_to_string(&file_path).unwrap();
        assert!(updated_contents.contains("println!(\"Hello, World!\");"));
        assert!(updated_contents.contains("println!(\"M31A Patching Engine\");"));

        let notes_contents = std::fs::read_to_string(dir.path().join("notes.txt")).unwrap();
        assert_eq!(notes_contents, "First note\nSecond note\n");
    }

    #[test]
    fn test_fuzzy_seek_sequence_normalisation() {
        let lines = vec![
            "fn compute() -> i32 {".to_string(),
            "    // Hyphen: – and quote: “test”".to_string(),
            "    42".to_string(),
            "}".to_string(),
        ];

        // Pattern using ASCII '-' and '"' instead of Unicode typographic marks
        let pattern = vec![
            "// Hyphen: - and quote: \"test\"".to_string(),
            "42".to_string(),
        ];

        let idx = seek_sequence::seek_sequence(&lines, &pattern, 0, false);
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn test_parse_patch_with_preamble_and_heredoc() {
        let patch_text = r#"Sure! Here is the patch:
apply_patch <<'EOF'
*** Begin Patch
*** Add File: test_heredoc.txt
+Hello from heredoc
*** End Patch
EOF
Let me know if this helps!"#;

        let args = parse_patch(patch_text).expect("Failed to parse patch with heredoc & preamble");
        assert_eq!(args.hunks.len(), 1);
        if let Hunk::AddFile { path, contents } = &args.hunks[0] {
            assert_eq!(path.to_str().unwrap(), "test_heredoc.txt");
            assert_eq!(contents, "Hello from heredoc\n");
        } else {
            panic!("Expected AddFile hunk");
        }
    }
}
