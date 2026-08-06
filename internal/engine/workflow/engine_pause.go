package workflow

// Pause/resume support for the execute phase. All methods use e.pauseMu for synchronization.

import "context"

// PauseExecution pauses the execute phase. While paused, task groups wait before executing.
// Returns true if the engine was paused, false if already paused or not in execute phase.
func (e *Engine) PauseExecution() bool {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.pauseCh != nil {
		return false // already paused
	}
	e.pauseCh = make(chan struct{})
	e.resumeCh = make(chan struct{})
	e.skipTaskCh = make(chan int, 1)
	e.cancelTaskCh = make(chan int, 1)
	e.cancelGroupCh = make(chan struct{})
	return true
}

// ResumeExecution resumes the execute phase after a pause.
// Returns true if the engine was resumed, false if not paused.
func (e *Engine) ResumeExecution() bool {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.pauseCh == nil {
		return false // not paused
	}
	close(e.resumeCh)
	e.pauseCh = nil
	e.resumeCh = nil
	e.skipTaskCh = nil
	e.cancelTaskCh = nil
	e.cancelGroupCh = nil
	return true
}

// IsPaused returns whether the execute phase is currently paused.
func (e *Engine) IsPaused() bool {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	return e.pauseCh != nil
}

// SkipCurrentTask sends a skip command for a specific task (must be called while paused).
func (e *Engine) SkipCurrentTask(taskID int) {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.skipTaskCh != nil {
		select {
		case e.skipTaskCh <- taskID:
		default:
		}
	}
}

// CancelCurrentTask sends a cancel command for a specific task (must be called while paused).
func (e *Engine) CancelCurrentTask(taskID int) {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.cancelTaskCh != nil {
		select {
		case e.cancelTaskCh <- taskID:
		default:
		}
	}
}

// CancelGroup cancels the entire current task group (must be called while paused).
func (e *Engine) CancelGroup() {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.cancelGroupCh != nil {
		close(e.cancelGroupCh)
		e.cancelGroupCh = nil
	}
}

// waitForPause blocks until the engine is resumed or context is cancelled.
// Returns true if resumed, false if context was cancelled.
func (e *Engine) waitForPause(ctx context.Context) bool {
	e.pauseMu.Lock()
	if e.pauseCh == nil {
		e.pauseMu.Unlock()
		return true // not paused
	}
	resumeCh := e.resumeCh
	e.pauseMu.Unlock()

	select {
	case <-resumeCh:
		return true
	case <-ctx.Done():
		return false
	}
}

// consumeSkipOrCancel reads skip/cancel commands while paused. Returns (skipID, cancelID, cancelledGroup, ok).
func (e *Engine) consumeSkipOrCancel(ctx context.Context) (skipID int, cancelID int, cancelledGroup bool, ok bool) {
	e.pauseMu.Lock()
	skipCh := e.skipTaskCh
	cancelCh := e.cancelTaskCh
	groupCh := e.cancelGroupCh
	resumeCh := e.resumeCh
	e.pauseMu.Unlock()

	select {
	case <-resumeCh:
		return 0, 0, false, true
	case id := <-skipCh:
		return id, 0, false, true
	case id := <-cancelCh:
		return 0, id, false, true
	case <-groupCh:
		return 0, 0, true, true
	case <-ctx.Done():
		return 0, 0, false, false
	}
}
