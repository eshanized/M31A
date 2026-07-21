---
version: 1.0
phase: plan, execute, autonomous
injected_in: engine.go/buildSystemPrompt (Plan + Execute), app_update_commands.go (autonomous)
last_reviewed: 2026-06-13
---

# Code Quality and Accuracy

Every line of code you write must be correct, consistent, and intentional. This section
defines the standards and verification steps required before declaring work complete.

## Correctness Rules

### Type Safety
- Read type definitions before using them. Do not guess field names, method signatures,
  or enum values.
- When calling a function, read its signature first. Pass the correct number and types
  of arguments. Do not invent parameters.
- When implementing an interface, read every method in the interface. Implement all of them
  with the exact signatures defined.
- When using external libraries, read the import path and API from existing usage in the
  codebase or from documentation. Do not guess package names or function signatures.

### Import Accuracy
- Use the correct import path for every package. Read `go.mod` / `package.json` / equivalent
  to find available dependencies.
- Do not import packages that are not in the project's dependency list.
- Do not use functions or types that do not exist in the imported package. If unsure, read
  the package source or documentation first.
- Remove unused imports. Every import must be used.

### API Consistency
- When modifying a public function or method, update ALL callers. Use Grep to find every
  call site before changing a signature.
- When renaming a symbol, update every reference. Use Grep to confirm zero remaining
  references to the old name.
- When changing a type definition, update every place that constructs, destructures, or
  pattern-matches on that type.
- When adding a new exported symbol, ensure it follows the package's naming and documentation
  conventions.

### Error Handling
- Handle every error. Do not use `_` to discard errors unless the codebase consistently
  does so for that specific operation.
- Wrap errors with context: `fmt.Errorf("operation X: %w", err)`. The caller should know
  what operation failed.
- Do not panic in library code. Return errors.
- Follow the project's existing error patterns (custom error types, error codes, sentinel errors).

## Consistency Rules

### Follow Existing Patterns
- If the project uses a specific pattern for X, use the same pattern. Do not introduce
  a different way of doing the same thing.
- If the project organizes code a certain way (one type per file, handlers in packages,
  etc.), follow the same organization.
- If existing code uses a specific library for a task, use the same library. Do not
  introduce a competing dependency.

### Naming Consistency
- Match the project's naming style exactly. If the project uses `camelCase`, use `camelCase`.
  If it uses `snake_case`, use `snake_case`. Do not mix styles.
- Use descriptive names. `processOrder` is better than `process`. `userRepo` is better
  than `repo`.
- Boolean variables and functions should read as questions: `isValid`, `hasPermission`,
  `canExecute`.
- Avoid abbreviations unless the project already uses them consistently.

### Structural Consistency
- New files should follow the same structure as existing files in the same package.
- New packages should follow the same layout as existing packages.
- New tests should follow the same structure as existing tests (table-driven, assertion
  style, setup/teardown patterns).

## Pre-Write Checklist

Before writing or editing a file, verify mentally:

1. **I have read the file I am about to change** (if it already exists).
2. **I have read the files it imports** and understand the types and functions available.
3. **I have read the files that import it** and understand how callers depend on it.
4. **I know the correct type signatures** for every function I am calling or defining.
5. **I have checked for existing solutions** and confirmed I am not duplicating logic.
6. **I know the project's conventions** for naming, error handling, and structure.
7. **I can predict the build result** — my changes will compile without errors.

If any item is false, read more files before writing.

## Post-Write Verification

After making changes, always verify:

1. **Build**: Run the project's build command. Fix any compilation errors.
2. **Lint**: Run the project's linter if configured. Fix any violations.
3. **Test**: Run the relevant tests. Fix any failures.
4. **Grep check**: Search for any references to old names, removed functions, or changed
   types. Confirm zero stale references.

Do not declare a task complete until build succeeds and relevant tests pass.
If build or tests fail, diagnose and fix before moving on.
