# User-Defined Slash Commands

M31A supports personal slash commands declared as TOML prompt contracts. A definition is a reusable, model-driven procedure, not a program: the TOML file cannot directly execute shell code or grant itself capabilities. The command runs through M31A's canonical prompt compiler, model invocation, policy checks, approval coordinator, and tool pipeline.

## Where commands live

Create one TOML file for each command under the global configuration directory:

```text
<global-config-dir>/prompts/commands/<command-name>.toml
```

M31A resolves the global configuration root through its platform-aware path abstraction. Development builds use the isolated development configuration root. The loader scans TOML files in nested directories with a bounded discovery depth; it rejects files that are oversized, malformed, unsafe through symlinks/path traversal, or stored under protected Git internals. A bad definition is reported instead of being partially registered.

After creating or editing files, restart M31A to ensure the active runtime reloads them. The runtime has an explicit reload API, but automatic filesystem watching is not a guaranteed user-facing feature.

## Minimal command definition

```toml
id = "command.review_notes"
version = 1
kind = "command"

[command]
name = "review-notes"
aliases = ["rn"]
description = "Review the supplied change notes and summarize risks."

[execution]
role = "reviewer"
side_effect = "read_only"
max_steps = 12

[inputs]
required = [
  { name = "target", type = "string", description = "File or change to inspect." },
]
optional = [
  { name = "focus", type = "string", default = "correctness", description = "Review focus." },
]

[verification]
required = false
checks = []

[template]
body = """
Review {{ target }} with a focus on {{ focus }}.
Use only read-only tools. Summarize findings with file/line context
where available, and distinguish confirmed defects from questions.
"""
```

Invoke it from the interactive composer with:

```text
/review-notes --target src/runtime.rs --focus security
/rn --target "docs/architecture notes.md"
/review-notes --help
```

## Supported definition fields

- `id`, `version`, and `kind = "command"` identify the versioned prompt contract. The contract ID must use the reserved `command.*` namespace.
- `[command]` requires a name and description. Optional aliases must be unique and cannot collide with built-in commands. Names use ASCII letters, digits, hyphens, and underscores, and are normalized to lowercase.
- `[execution]` requires a registered agent role. `side_effect` accepts read-only or mutating values. Command-level approval defaults to required for mutating commands; runtime tool policy and approvals still apply regardless.
- `execution.max_steps` is bounded to 1–100 and defaults to 20. It limits model/tool iterations; it does not guarantee that the model will follow every instruction in the template.
- `[inputs]` supports required and optional parameters of type `boolean`, `string`, or `integer`. Optional inputs can have validated defaults. Required inputs cannot declare defaults.
- `[capabilities].required` documents requested capabilities. It is advisory, not a grant; the active capability registry, role envelope, policy, and tool pipeline decide what can actually execute.
- `[verification]` accepts only `working_tree_status`, `commit_contains_single_file`, and `conventional_commit_message` today. When `required = true`, at least one check must be configured. Check names are trimmed, validated, and deduplicated.
- `[template].body` is a MiniJinja template. Every referenced variable must correspond to a declared input; unknown variables and malformed templates are rejected when the command loads.

Unknown fields are rejected so misspelled keys fail early instead of being silently ignored. Command names, aliases, prompt IDs, and parameter names are validated before registration.

## Arguments

User-defined commands take named flags rather than positional arguments:

```text
/review-notes --target src/main.rs
/review-notes --target=src/main.rs --focus security
/review-notes --target "path with spaces.rs" --focus "API compatibility"
```

A boolean flag can be supplied as `--dry-run` or with an explicit value such as `--dry-run=false`, if that input is declared. String and integer flags require values. Unknown flags, duplicate flags, invalid types, and missing required inputs are rejected deterministically. Use `--help` or `-h` to show the definition's generated help.

## Safety behavior

### Dry-run is enforced

When a declared boolean input named `dry_run` is true, M31A only allows proposed tools whose registered base risk is `ReadOnly`. File mutations, commits, process execution, external communication, and unregistered tools are blocked before the tool execution pipeline. A prompt saying “do not make changes” is not the enforcement mechanism.

Dry-run still invokes the configured model provider and persists normal runtime/session/audit records. It means proposed tool actions cannot perform non-read-only operations; it does not mean the entire application performs zero internal state writes or zero network requests.

### Read-only commands are enforced

A command declared with `side_effect = "read_only"` has the same pre-execution restriction. The runtime checks the tool's registered risk class rather than trusting the prompt. This covers model-proposed tool calls and complements, rather than replaces, the canonical runtime policy and capability checks. M31A still persists its run/task/agent and audit records; the declaration does not imply a zero-write process.

### Other boundaries

- User command files cannot grant capabilities or replace protected runtime/behavioral prompt contracts.
- Mutating tool calls remain subject to M31A's existing policy and approval pipeline.
- Command-level approval is an extra authorization gate; disabling it in a user-authored definition does not disable tool-level policy checks.
- Unknown tool risks and invalid definitions fail closed for restricted invocations.

## Current limitations

User-defined commands are model-driven orchestration. They do not yet provide deterministic multi-step workflow graphs, user-authored verifier plugins, arbitrary scripts, rich schemas such as enums or arrays, or a dedicated command editor/management screen. Verification is limited to the three checks listed above. For safety-critical behavior, do not rely on prompt wording alone; use enforced policy boundaries and explicit verifiers.
