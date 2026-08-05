package workflow

import (
	"fmt"
	"sync"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
)

// TestWorkflowState_ConcurrentPlanAccess verifies that concurrent reads and
// writes to plan state fields (planMarkdown, planVersion, refineFeedback,
// researchOutput) do not race under the -race detector.
func TestWorkflowState_ConcurrentPlanAccess(t *testing.T) {
	ws := &WorkflowState{
		decisionLog: decision.NewLogger(256),
	}

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 100

	// Concurrent writers: SetPlanContent
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetPlanContent(fmt.Sprintf("plan-g%d-i%d", id, j))
			}
		}(i)
	}

	// Concurrent readers: PlanContent
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				_ = ws.PlanContent()
			}
		}(i)
	}

	// Concurrent writers: SetRefineFeedback
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetRefineFeedback(fmt.Sprintf("feedback-g%d-i%d", id, j))
			}
		}(i)
	}

	// Concurrent readers: RefineFeedback
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				_ = ws.RefineFeedback()
			}
		}(i)
	}

	// Concurrent writers: SetResearchOutput
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetResearchOutput(fmt.Sprintf("research-g%d-i%d", id, j))
			}
		}(i)
	}

	// Concurrent readers: ResearchOutput
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				_ = ws.ResearchOutput()
			}
		}(i)
	}

	// Concurrent version increment and read
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				_ = ws.IncrementPlanVersion()
			}
		}(i)
	}

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				_ = ws.PlanVersion()
			}
		}(i)
	}

	wg.Wait()
}

// TestWorkflowState_ConcurrentMessageAccess verifies that concurrent message
// operations (AppendMessage, MessagesSnapshot, SetMessages, ClearMessages)
// do not race under the -race detector.
func TestWorkflowState_ConcurrentMessageAccess(t *testing.T) {
	ws := &WorkflowState{
		decisionLog: decision.NewLogger(256),
	}

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 100

	// Concurrent writers: AppendMessage
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.AppendMessage(m31types.Message{
					Role:    "user",
					Content: fmt.Sprintf("message from goroutine %d iteration %d", id, j),
				})
			}
		}(i)
	}

	// Concurrent readers: MessagesSnapshot
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				snapshot := ws.MessagesSnapshot()
				// Verify snapshot is a copy (modifying it shouldn't affect original)
				if len(snapshot) > 0 {
					snapshot[0] = m31types.Message{Role: "tampered", Content: "should not affect original"}
				}
			}
		}(i)
	}

	// Concurrent SetMessages
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetMessages([]m31types.Message{
					{Role: "system", Content: fmt.Sprintf("set-g%d-i%d", id, j)},
				})
			}
		}(i)
	}

	// Concurrent ClearMessages
	for i := 0; i < goroutines/2; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations/2; j++ {
				ws.ClearMessages()
			}
		}(i)
	}

	wg.Wait()
}

// TestWorkflowState_ConcurrentIntentAccess verifies that concurrent reads and
// writes to intent classification and heal report fields do not race.
func TestWorkflowState_ConcurrentIntentAccess(t *testing.T) {
	ws := &WorkflowState{
		decisionLog: decision.NewLogger(256),
	}

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 100

	intentTypes := []m31types.IntentType{
		m31types.IntentFeature,
		m31types.IntentBugfix,
		m31types.IntentRefactor,
	}

	// Concurrent writers: SetIntentResult
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetIntentResult(&m31types.IntentResult{
					Intent:     intentTypes[j%len(intentTypes)],
					Complexity: "medium",
					Confidence: 0.8,
					Summary:    fmt.Sprintf("summary-g%d-i%d", id, j),
				})
			}
		}(i)
	}

	// Concurrent readers: IntentResult
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ir := ws.IntentResult()
				if ir != nil {
					_ = ir.Intent
					_ = ir.Complexity
					_ = ir.Confidence
					_ = ir.Summary
					_ = ir.Scope
				}
			}
		}(i)
	}

	// Concurrent writers: SetLastHealReport
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetLastHealReport(&m31types.HealReport{
					TaskID:  id,
					Attempt: j,
					Success: true,
				})
			}
		}(i)
	}

	// Concurrent readers: LastHealReport
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				hr := ws.LastHealReport()
				if hr != nil {
					_ = hr.TaskID
					_ = hr.Attempt
					_ = hr.Success
				}
			}
		}(i)
	}

	wg.Wait()
}

// TestWorkflowState_ConcurrentCheckpointAccess verifies that concurrent reads
// and writes to checkpoint data and current goal do not race.
func TestWorkflowState_ConcurrentCheckpointAccess(t *testing.T) {
	ws := &WorkflowState{
		decisionLog: decision.NewLogger(256),
	}

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 100

	// Concurrent writers: SetCheckpointData
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetCheckpointData(&CheckpointData{
					Phase:       m31types.PhaseExecute,
					Goal:        fmt.Sprintf("goal-g%d-i%d", id, j),
					PlanVersion: j,
				})
			}
		}(i)
	}

	// Concurrent readers: CheckpointData
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				cp := ws.CheckpointData()
				if cp != nil {
					_ = cp.Phase
					_ = cp.Goal
					_ = cp.PlanVersion
				}
			}
		}(i)
	}

	// Concurrent writers: SetCurrentGoal
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetCurrentGoal(fmt.Sprintf("goal-g%d-i%d", id, j))
			}
		}(i)
	}

	// Concurrent readers: CurrentGoal
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				_ = ws.CurrentGoal()
			}
		}(i)
	}

	wg.Wait()
}

// TestWorkflowState_CrossFieldAccess verifies that goroutines accessing
// different field groups simultaneously (plan + messages + intent + checkpoint)
// do not race across field boundaries. This tests that the per-field locking
// strategy works correctly when multiple fields are accessed concurrently.
func TestWorkflowState_CrossFieldAccess(t *testing.T) {
	ws := &WorkflowState{
		decisionLog: decision.NewLogger(256),
	}

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 100

	// Group 1: Plan access
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetPlanContent(fmt.Sprintf("plan-%d-%d", id, j))
				_ = ws.PlanContent()
				ws.SetRefineFeedback(fmt.Sprintf("fb-%d-%d", id, j))
				_ = ws.RefineFeedback()
				_ = ws.IncrementPlanVersion()
			}
		}(i)
	}

	// Group 2: Message access
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.AppendMessage(m31types.Message{
					Role:    "user",
					Content: fmt.Sprintf("cross-field msg %d-%d", id, j),
				})
				snapshot := ws.MessagesSnapshot()
				_ = len(snapshot)
			}
		}(i)
	}

	// Group 3: Intent access
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetIntentResult(&m31types.IntentResult{
					Intent:     m31types.IntentFeature,
					Complexity: "medium",
					Confidence: 0.8,
					Summary:    fmt.Sprintf("summary-%d-%d", id, j),
				})
				ir := ws.IntentResult()
				if ir != nil {
					_ = ir.Intent
				}
			}
		}(i)
	}

	// Group 4: Checkpoint access
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetCheckpointData(&CheckpointData{
					Phase:       m31types.PhaseExecute,
					Goal:        fmt.Sprintf("cp-goal-%d-%d", id, j),
					PlanVersion: j,
				})
				cp := ws.CheckpointData()
				if cp != nil {
					_ = cp.Goal
				}
				ws.SetCurrentGoal(fmt.Sprintf("goal-%d-%d", id, j))
				_ = ws.CurrentGoal()
			}
		}(i)
	}

	// Group 5: Heal report access
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetLastHealReport(&m31types.HealReport{
					TaskID:  id,
					Attempt: j,
					Success: true,
				})
				hr := ws.LastHealReport()
				if hr != nil {
					_ = hr.TaskID
				}
			}
		}(i)
	}

	// Group 6: Research output access
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				ws.SetResearchOutput(fmt.Sprintf("research-%d-%d", id, j))
				_ = ws.ResearchOutput()
			}
		}(i)
	}

	wg.Wait()
}
