# Sandbox Policy Reference

## Sandbox Levels

| Level | Description | Use Case |
|-------|-------------|----------|
| `ReadOnly` | File reads only, safe commands (ls, cat, grep) | Exploration, analysis |
| `WorkspaceWrite` | Read/write workspace, run tests, build | Default for coding tasks |
| `DangerFullAccess` | Unrestricted system access | System administration (rare) |

## Protected Paths (Always Blocked)

The following paths are **always protected** regardless of sandbox level:
- `.git/` — Git internals
- `.m31a/` — Agent configuration and state
- `.codex/` — Codex compatibility layer
- `.ssh/` — SSH keys
- `.env*` — Environment files with secrets

Attempting to access these will be denied by the sandbox.

## Network Access

- **Default**: No network access
- **With additional permissions**: Can request network access via `request_permissions` tool
- **Exec command**: Can specify `additional_permissions.network: true` for per-command access

## ExecPolicy (Starlark)

Commands are evaluated against `ExecPolicy` rules before execution:
- Prefix rules: `["git", "pull"]` → auto-approve `git pull origin main`
- Network rules: Allow/deny specific hosts/ports
- Decision: `allow` | `prompt` | `deny` with justification

The policy is defined in `.m31a/execpolicy.star` or project config.

## Approval Flow

```
exec_command → ExecPolicy check → Sandbox check → User approval (if needed) → Execute
```

- `OnRequest`: Prompt for each privileged command
- `Never`: Deny all privileged commands
- `ApproveForMe`: Guardian reviews automatically (circuit breaker: 3 denials in 5 → trip)