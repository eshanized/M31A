# M31A Testing Guide

## Test Commands

```bash
make test           # Race-enabled tests with coverage
make test-fast      # Tests without race detector
make test-specific TEST=TestFoo   # Run one test
make cover          # Generate HTML coverage report
```

## Coverage Targets

- **75%** overall
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

## Security Testing

### Command Injection
- Test obfuscation bypass attempts (double spaces, tabs, mixed case)
- Test variable expansion detection (`$VAR`, `${VAR}`, `$(cmd)`)
- Test newline and special character handling
- See `TestBash_ObfuscationDetection` in `internal/tools/bash_test.go`

### SSRF Protection
- Test private IP blocking (loopback, RFC1918, link-local)
- Test metadata endpoint blocking (169.254.169.254)
- Test DNS pinning via shared cache
- See `TestWebFetch_SSRF*` in `internal/tools/webfetch_test.go`

### Type Safety
- Test comma-ok type assertion guards on interface values
- Test graceful handling of invalid types in concurrent maps
- See `TestPermissions_InvalidType` in `internal/tools/permissions_test.go`

## Race Testing

### Required Race Tests
- All concurrent data structures (WorkflowCache, DNSCache)
- All goroutine lifecycle management
- All shared mutable state access

### Running Race Tests
```bash
go test -race ./...
```

### High-Contention Tests
- `TestWorkflowCache_ConcurrentDynamicContext` - 100 goroutines, 1000 iterations
- `TestDNSCache_HighContention` - 50 goroutines, 1000 iterations
- `TestDNSCache_ConcurrentEviction` - Low eviction threshold stress test

## Test Organization

- Test files are co-located with source files in the same package
- Table-driven tests are preferred for parameterized cases
- `t.Parallel()` is used where safe (avoid for tests with shared state)
- Mock tools are defined in `dispatcher_test.go` (`mockTool` struct)

## E2E Tests

`e2e_test.go` compiles and runs the binary. Real API tests (`TestBinary_Prompt_*RealAPI`) require env vars (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`) -- they skip when unset.
