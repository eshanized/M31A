# Approval Mode Behavior

## Three Approval Policies

| Policy | Behavior | When to Use |
|--------|----------|-------------|
| `OnRequest` | Prompt user for each privileged action | Default; safe interactive work |
| `Never` | Auto-deny all privileged actions | Fully autonomous, read-only tasks |
| `ApproveForMe` | Guardian reviews automatically | Autonomous with safety net |

## Guardian (ApproveForMe)

When `approval = "ApproveForMe"`, the **Guardian** reviews every tool operation:
- Evaluates safety of shell commands, patches, file writes
- Sliding-window circuit breaker: **3 denials in 5 operations → trips to `Never`**
- Provides rationale for each decision
- Zero-latency local review (no external API calls)

## Per-Command Overrides

`exec_command` supports per-invocation sandbox overrides:
```json
{
  "command": "rm -rf build/",
  "sandbox_permissions": "require_escalated",
  "justification": "Clean build artifacts",
  "prefix_rule": ["rm", "-rf", "build/"]
}
```

- `sandbox_permissions`: `use_default` | `with_additional_permissions` | `require_escalated`
- `justification`: Required for `require_escalated`; shown to user/guardian
- `prefix_rule`: Reusable approval prefix for repeated commands

## Network Approval

Network access requests go through the same approval flow:
1. Tool requests network via `additional_permissions.network: true`
2. ExecPolicy checks network rules
3. If no rule matches → approval prompt (or Guardian review)

## Best Practices

- **Default to `OnRequest`** for interactive sessions
- **Use `ApproveForMe`** for long autonomous runs with Guardian safety net
- **Never use `Never`** unless explicitly requested by user
- **Provide clear justifications** — they improve Guardian accuracy and user trust