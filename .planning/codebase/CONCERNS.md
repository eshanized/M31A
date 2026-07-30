# CONCERNS.md — M31A Technical Debt & Concerns

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## Technical Debt

### High Priority
1. **TUI Package Size** (`internal/ui/tui/`, ~180 files)
   - Largest package, potential for circular dependencies
   - Consider extracting sub-packages: `tui/screens/`, `tui/handlers/`
   - Risk: Slow compilation, difficult navigation

2. **Workflow Engine Complexity** (`internal/engine/workflow/`, ~100 files)
   - Many test files (~30) suggest complex logic
   - Phase handlers could benefit from stricter interfaces
   - Risk: Bug introduction during modifications

3. **Provider Abstraction** (`internal/integrations/provider/`)
   - Three providers with similar implementations
   - Potential code duplication in SSE parsing, model fetching
   - Risk: Inconsistent behavior across providers

### Medium Priority
4. **Permission System Complexity**
   - Multiple layers: rules, batch approval, persistent permissions
   - Risk: Subtle bugs in permission evaluation
   - Mitigation: Extensive test coverage exists

5. **Session Management**
   - Multiple session storage locations (project-local + global)
   - Checkpoint system adds complexity
   - Risk: Session corruption on crash

6. **Cross-Platform Code**
   - Platform-specific files: `keychain_*.go`, `ship_lock_*.go`, `runtime_proc_*.go`
   - Risk: Platform-specific bugs, testing gaps

## Security Concerns

### API Key Handling
- **Mitigated**: Keys stored in OS keychain, not plaintext
- **Mitigated**: `.env` files gitignored
- **Concern**: Keychain initialization failures logged but not blocking
- **Recommendation**: Graceful degradation documented

### Tool Execution
- **Mitigated**: Permission system with risk levels
- **Mitigated**: Rate limiting and concurrency control
- **Mitigated**: Bash command blocking (obfuscation patterns)
- **Concern**: `persistent_permissions` saves rules to disk
- **Recommendation**: Audit permission rule storage

### Web Fetch/Search
- **Mitigated**: DNS caching prevents DNS rebinding
- **Mitigated**: Retry limits configurable
- **Concern**: External API responses not validated
- **Recommendation**: Response schema validation

## Performance Concerns

### Token Estimation
- `tiktoken-go` used for LLM context estimation
- **Concern**: Large context windows may slow estimation
- **Mitigation**: Caching in place

### File Operations
- `fsnotify` for file watching
- **Concern**: Large directory trees may cause high CPU usage
- **Mitigation**: Watch limits in place

### Tool Output Bounding
- `OutputStore` limits output to prevent context exhaustion
- **Concern**: Truncated output may lose important information
- **Mitigation**: Configurable limits (`OutputMaxLines`, `OutputMaxBytes`)

## Testing Gaps

### E2E Test Coverage
- Binary compilation tests exist
- **Gap**: Limited user interaction simulation
- **Gap**: Headless workflow mode under-tested

### Integration Test Coverage
- Tool integration tests exist
- **Gap**: Cross-package integration limited
- **Gap**: Error recovery paths under-tested

### Platform Test Coverage
- Linux: Well-tested (primary development platform)
- **Gap**: macOS/Windows specific code paths
- **Gap**: Keychain implementations untested in CI

## Documentation Gaps

### API Documentation
- Provider interface documented
- **Gap**: Internal tool APIs undocumented
- **Gap**: Workflow phase contracts undocumented

### Architecture Documentation
- High-level architecture clear
- **Gap**: Data flow diagrams missing
- **Gap**: State machine documentation missing

## Dependencies

### Go Version
- Requires Go 1.25.12 (very recent)
- **Concern**: May limit contributor adoption
- **Mitigation**: Docker/CI builds use specific Go version

### External Dependencies
- Bubble Tea ecosystem (charmbracelet/*)
- **Concern**: Large dependency tree for TUI
- **Mitigation**: No cgo dependencies (static binary)

## Recommendations

1. **Refactor TUI package** — extract sub-packages for screens, handlers
2. **Add integration tests** — cross-package error paths
3. **Document workflow phases** — phase contracts and state machine
4. **Add data flow diagrams** — visualize tool execution and session management
5. **Platform CI testing** — add macOS/Windows CI runners
6. **Audit permission storage** — review persistent permission security
