package workflow

// Lock ordering hierarchy
//
// WorkflowState uses multiple mutexes to protect different groups of state.
// To prevent deadlocks, ALL lock acquisitions MUST follow this total order:
//
//	transitionMu > planMu > messagesMu > intentResultMu > cachedFullPromptsMu > checkpointMu
//
// Rules:
//   - When acquiring multiple locks, always acquire in the documented order.
//   - Never acquire a lock that is earlier in the order while holding a later lock.
//   - Each accessor method in WorkflowState acquires exactly one lock,
//     so single-method calls are always safe.
//   - Code that needs multiple fields must acquire locks in order or use
//     snapshot methods that return copies.
//
// Example (from RunPhase):
//
//	e.state.transitionMu.Lock()    // 1st (phase transition)
//	e.state.messagesMu.Lock()      // 2nd (message update)
//	e.state.messagesMu.Unlock()
//	e.state.transitionMu.Unlock()
//
// This ordering is documented here as the single authoritative source.
// All code in the workflow package must respect this contract.

// VerifyLockOrder is a development-time helper that documents the expected
// lock ordering. It is not called at runtime but serves as executable
// documentation of the locking contract.
//
// Usage (in tests or code review):
//
//	// Correct ordering:
//	VerifyLockOrder("transitionMu", "planMu") // OK
//	VerifyLockOrder("planMu", "messagesMu")   // OK
//
//	// Incorrect ordering (would deadlock):
//	VerifyLockOrder("messagesMu", "planMu")   // VIOLATION
func VerifyLockOrder(first, second string) bool {
	order := map[string]int{
		"transitionMu":      0,
		"planMu":            1,
		"messagesMu":        2,
		"intentResultMu":    3,
		"cachedFullPromptsMu": 4,
		"checkpointMu":      5,
	}
	o1, ok1 := order[first]
	o2, ok2 := order[second]
	if !ok1 || !ok2 {
		return false
	}
	return o1 < o2
}
