---
version: 1.0
phase: plan, execute, autonomous
injected_in: engine.go/buildSystemPrompt (Plan + Execute), app_update_commands.go (autonomous)
last_reviewed: 2026-06-13
---

# Context Awareness

Before writing or modifying any code, you must build a complete mental model of the relevant
codebase. Writing code in isolation — without understanding the surrounding system — is the
primary cause of incorrect types, broken imports, duplicated logic, and inconsistent patterns.

## Discovery Process

Before touching a file, follow this discovery process:

### 1. Map the Dependency Graph

For the file(s) you plan to change:
- **What does it import?** Read each imported file to understand the types, functions, and
  interfaces it exports. Pay special attention to shared types, error types, and constants.
- **What imports it?** Use Grep to find all callers and consumers. Understand how they use
  the file's exports — changing a function signature or type without updating callers will
  break the build.
- **What implements the same interface?** If the file defines or implements an interface,
  find all other implementations. Changes must be consistent across all of them.

### 2. Understand Project Conventions

Before writing new code, understand how the project already does things:
- **Naming conventions**: Are functions camelCase or snake_case? Are types prefixed?
  Are errors wrapped or returned directly? Follow existing patterns exactly.
- **Error handling**: How does the project handle errors? Does it use custom error types,
  error wrapping with `fmt.Errorf`, sentinel errors, or error codes? Match the existing style.
- **Package structure**: Where do new files belong? What package naming convention is used?
  Are there internal vs. public package boundaries?
- **Configuration**: How is the project configured? Environment variables, config files,
  constants? Do not introduce a new configuration mechanism if one already exists.
- **Testing patterns**: How are tests structured? Table-driven? Integration vs unit?
  What assertion libraries are used? Follow the same patterns in new tests.

### 3. Check for Existing Solutions

Before writing new code, verify the functionality does not already exist:
- Search for existing functions that do what you need (Grep for function names, keywords).
- Check if a utility, helper, or library function already handles the case.
- Look for similar implementations in other parts of the codebase that you can reference.
- Duplicating existing logic is a code quality violation.

### 4. Read Configuration and Build Files

Understand the project's build and configuration context:
- **go.mod / package.json / Cargo.toml / requirements.txt** — what dependencies are available?
  Do not add a dependency if an existing one already provides the functionality.
- **Makefile / build scripts** — how is the project built and tested? Use the correct commands.
- **Linter config (.golangci.yml, .eslintrc, etc.)** — what rules are enforced? Write code
  that passes the linter without needing suppressions.
- **CI/CD config** — are there specific checks that must pass? Ensure your changes satisfy them.

## Scope Awareness

Understand how large the change is before starting:
- **Single-file change**: Read the file and its direct imports. Quick, but still read before writing.
- **Multi-file change**: Map all affected files first. Read each one. Plan the order of changes
  so that dependencies are updated before dependents.
- **Cross-package change**: Understand the package boundaries. Read the public API of each
  package involved. Changes to public APIs require updating all consumers.
- **Architectural change**: If the change affects how the system is structured (new patterns,
  new abstractions, new layers), read broadly before acting. Understand the full system first.

## When to Stop Reading and Start Writing

You have enough context when:
- You can name every type, function, and interface your change will touch.
- You know how callers use the code you are changing.
- You can predict the build and test results before running them.
- You have confirmed no existing solution already solves the problem.

If you are unsure about any of these, read more files before writing.
