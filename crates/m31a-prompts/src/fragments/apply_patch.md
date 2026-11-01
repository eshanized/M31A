# Apply Patch Guidance

## Critical: Use the FREEFORM Tool

**`apply_patch` is a FREEFORM tool.** Never invoke `applypatch` or `apply-patch` as a shell command. The model must call the `apply_patch` tool directly with the patch as a parameter.

## Patch Format

The patch follows the exact Codex specification:

```
*** Begin Patch
*** Add File: path/to/new_file.rs
@@
+fn new_function() {
+    println!("Hello");
+}
*** End of File
*** Update File: path/to/existing_file.rs
@@ fn existing_function() {
     println!("Old");
-    println!("Old");
+    println!("New");
 }
*** End of File
*** Delete File: path/to/obsolete_file.rs
*** End Patch
```

## Key Rules

1. **Always use `*** Begin Patch` / `*** End Patch`** as outer delimiters
2. **File operations**: `*** Add File:`, `*** Update File:`, `*** Delete File:`, `*** Move to:`
3. **Hunks**: `@@ context line` followed by `-old` / `+new` lines
4. **End of file**: `*** End of File` after each file's changes
5. **Context lines** must match exactly (whitespace-sensitive, but engine has 5-tier fuzzy matching)

## Fuzzy Matching (Engine Handles This)

The patch engine applies 5-tier matching automatically:
1. Exact match
2. Right-strip trailing whitespace
3. Full trim (both sides)
4. Unicode normalization (smart quotes → straight, em-dashes → hyphens, NBSP → space)
5. Prefix/substring for single-line context markers

**You don't need to worry about whitespace differences** — the engine handles them. Just provide context that uniquely identifies the location.

## Common Patterns

### Adding a function
```
*** Update File: src/lib.rs
@@ pub mod existing_module;
 
+pub mod new_module;
*** End of File
```

### Modifying a function body
```
*** Update File: src/main.rs
@@ fn process(data: &str) -> Result<String> {
     let cleaned = data.trim();
-    cleaned.to_uppercase()
+    Ok(cleaned.to_uppercase())
 }
*** End of File
```

### Adding a struct field
```
*** Update File: src/types.rs
@@ #[derive(Debug, Clone)]
 pub struct Config {
     pub name: String,
+    pub version: String,
 }
*** End of File
```

## When NOT to Use apply_patch

- Single-line changes in one file → use `write_file` after reading
- Simple search/replace → use `exec_command` with `sed` or similar
- Creating entirely new files → use `write_file` (simpler)

**Prefer `apply_patch` for multi-file changes or when you need atomic, reviewable diffs.**