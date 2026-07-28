package mocks

// WorkflowDispatcher implements workflow.Dispatcher for testing.
// This is the minimal mock for phase coordinator tests that only need
// RevokeBatchApprovals().
type WorkflowDispatcher struct {
	Revoked bool
}

func (d *WorkflowDispatcher) RevokeBatchApprovals() {
	d.Revoked = true
}
