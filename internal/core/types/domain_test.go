package types

import (
	"encoding/json"
	"reflect"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDomainTypes(t *testing.T) {
	// Test that all 18 domain types can be instantiated
	project := Project{
		ID:        uuid.New(),
		RootPath:  "/test",
		Name:      "TestProject",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, project.ID)

	workspace := Workspace{
		ID:        uuid.New(),
		ProjectID: uuid.New(),
		Name:      "TestWorkspace",
		RootPath:  "/test/ws",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, workspace.ID)

	session := Session{
		ID:            uuid.New(),
		ProjectID:     uuid.New(),
		Model:         "test-model",
		Provider:      "test-provider",
		StartedAt:     time.Now(),
		WorkflowPhase: PhaseInitialize,
	}
	assert.NotEqual(t, uuid.Nil, session.ID)

	run := Run{
		ID:        uuid.New(),
		SessionID: uuid.New(),
		Status:    RunStatusPlanned,
		StartedAt: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, run.ID)

	requirement := Requirement{
		ID:          uuid.New(),
		Title:       "Test Requirement",
		Description: "Test description",
		Phase:       1,
		Status:      RequirementStatusPending,
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, requirement.ID)

	decision := Decision{
		ID:        uuid.New(),
		Title:     "Test Decision",
		Status:    DecisionStatusProposed,
		Rationale: "Test rationale",
		Timestamp: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, decision.ID)

	plan := Plan{
		ID:        uuid.New(),
		Title:     "Test Plan",
		Objective: "Test objective",
		RiskLevel: RiskSafe,
		Status:    PlanStatusDraft,
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, plan.ID)

	task := Task{
		ID:                 1,
		Description:        "Test task",
		Action:             "implement",
		Status:             TaskStatusPending,
		AcceptanceCriteria: []string{"criteria 1"},
	}
	assert.Equal(t, 1, task.ID)

	taskGraph := TaskGraph{
		ID:        uuid.New(),
		PlanID:    uuid.New(),
		Status:    TaskGraphStatusPlanned,
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, taskGraph.ID)

	intent := Intent{
		Type:       IntentFeature,
		Complexity: ComplexitySimple,
		Summary:    "Test intent",
		Confidence: 0.9,
	}
	assert.Equal(t, IntentFeature, intent.Type)

	agent := Agent{
		ID:        uuid.New(),
		Role:      AgentRoleImplementer,
		Status:    AgentStatusIdle,
		StartedAt: time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, agent.ID)

	toolSpec := ToolSpec{
		Name:          "test_tool",
		Description:   "Test tool",
		RiskLevel:     RiskSafe,
		ResourceScope: ResourceScopeWorkspace,
	}
	assert.Equal(t, "test_tool", toolSpec.Name)

	artifact := Artifact{
		ID:        uuid.New(),
		Name:      "test.md",
		Type:      ArtifactTypeDocument,
		Path:      "/test/test.md",
		CreatedAt: time.Now(),
		CreatedBy: uuid.New(),
	}
	assert.NotEqual(t, uuid.Nil, artifact.ID)

	verification := Verification{
		ID:      uuid.New(),
		Level:   VerificationLevelUnit,
		Verdict: VerificationVerdictPending,
	}
	assert.NotEqual(t, uuid.Nil, verification.ID)

	checkpoint := Checkpoint{
		ID:          uuid.New(),
		Type:        CheckpointTypeHumanVerify,
		Description: "Test checkpoint",
		Risk:        "low",
		Status:      CheckpointStatusPending,
		CreatedAt:   time.Now(),
	}
	assert.NotEqual(t, uuid.Nil, checkpoint.ID)

	research := Research{
		ID:         uuid.New(),
		Question:   "Test question",
		Findings:   "Test findings",
		Confidence: 0.8,
		CreatedAt:  time.Now(),
		CreatedBy:  uuid.New(),
	}
	assert.NotEqual(t, uuid.Nil, research.ID)

	repository := Repository{
		ID:            uuid.New(),
		RootPath:      "/test/repo",
		DefaultBranch: "main",
	}
	assert.NotEqual(t, uuid.Nil, repository.ID)

	remote := Remote{
		Name: "origin",
		URL:  "https://github.com/test/repo.git",
	}
	assert.Equal(t, "origin", remote.Name)

	detectedStack := DetectedStack{
		Language:   "go",
		Framework:  "std",
		BuildTool:  "go",
		TestRunner: "go test",
	}
	assert.Equal(t, "go", detectedStack.Language)

	projectConfig := ProjectConfig{
		Provider:    "test",
		Model:       "test-model",
		MaxParallel: 4,
	}
	assert.Equal(t, "test", projectConfig.Provider)
}

func TestOwnership(t *testing.T) {
	// Verify each type is defined in exactly one file by checking package
	// We can't easily check file location at runtime, but we can verify
	// the types exist in this package
	types := []reflect.Type{
		reflect.TypeOf(Project{}),
		reflect.TypeOf(Repository{}),
		reflect.TypeOf(Workspace{}),
		reflect.TypeOf(Session{}),
		reflect.TypeOf(Run{}),
		reflect.TypeOf(Intent{}),
		reflect.TypeOf(Requirement{}),
		reflect.TypeOf(Decision{}),
		reflect.TypeOf(Plan{}),
		reflect.TypeOf(Task{}),
		reflect.TypeOf(TaskGraph{}),
		reflect.TypeOf(Agent{}),
		reflect.TypeOf(ToolSpec{}),
		reflect.TypeOf(PermissionPolicy("")),
		reflect.TypeOf(Artifact{}),
		reflect.TypeOf(Verification{}),
		reflect.TypeOf(Checkpoint{}),
		reflect.TypeOf(Research{}),
	}

	// All types should be in the same package (github.com/eshanized/M31A/internal/core/types)
	for _, typ := range types {
		assert.Equal(t, "github.com/eshanized/M31A/internal/core/types", typ.PkgPath(), "Type %s should be in types package", typ.Name())
	}
}

func TestSerialization(t *testing.T) {
	// Test JSON marshal/unmarshal round-trip for each type

	// Project
	project := Project{
		ID:        uuid.New(),
		RootPath:  "/test",
		Name:      "TestProject",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	data, err := json.Marshal(project)
	require.NoError(t, err)
	var project2 Project
	err = json.Unmarshal(data, &project2)
	require.NoError(t, err)
	assert.Equal(t, project.ID, project2.ID)
	assert.Equal(t, project.Name, project2.Name)
	// Verify nil slices serialize as empty arrays
	assert.NotNil(t, project2.Repositories)
	assert.Equal(t, 0, len(project2.Repositories))

	// Session
	session := Session{
		ID:            uuid.New(),
		ProjectID:     uuid.New(),
		Model:         "test-model",
		Provider:      "test-provider",
		StartedAt:     time.Now(),
		WorkflowPhase: PhaseInitialize,
	}
	data, err = json.Marshal(session)
	require.NoError(t, err)
	var session2 Session
	err = json.Unmarshal(data, &session2)
	require.NoError(t, err)
	assert.Equal(t, session.ID, session2.ID)
	// Tags may be nil after unmarshal if omitted in JSON (omitempty)
	if session2.Tags != nil {
		assert.Equal(t, 0, len(session2.Tags))
	}

	// Requirement
	req := Requirement{
		ID:          uuid.New(),
		Title:       "Test",
		Description: "Desc",
		Phase:       1,
		Status:      RequirementStatusActive,
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}
	data, err = json.Marshal(req)
	require.NoError(t, err)
	var req2 Requirement
	err = json.Unmarshal(data, &req2)
	require.NoError(t, err)
	assert.Equal(t, req.ID, req2.ID)
	// Traceability may be nil after unmarshal if omitted in JSON
	if req2.Traceability != nil {
		assert.Equal(t, 0, len(req2.Traceability))
	}

	// Decision
	dec := Decision{
		ID:        uuid.New(),
		Title:     "Test",
		Status:    DecisionStatusAccepted,
		Rationale: "Reason",
		Timestamp: time.Now(),
	}
	data, err = json.Marshal(dec)
	require.NoError(t, err)
	var dec2 Decision
	err = json.Unmarshal(data, &dec2)
	require.NoError(t, err)
	assert.Equal(t, dec.ID, dec2.ID)
	// Alternatives may be nil after unmarshal if omitted in JSON
	if dec2.Alternatives != nil {
		assert.Equal(t, 0, len(dec2.Alternatives))
	}

	// Plan
	plan := Plan{
		ID:        uuid.New(),
		Title:     "Test Plan",
		Objective: "Objective",
		RiskLevel: RiskMedium,
		Status:    PlanStatusApproved,
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	data, err = json.Marshal(plan)
	require.NoError(t, err)
	var plan2 Plan
	err = json.Unmarshal(data, &plan2)
	require.NoError(t, err)
	assert.Equal(t, plan.ID, plan2.ID)
	// Requirements and Tasks may be nil after unmarshal if omitted in JSON
	if plan2.Requirements != nil {
		assert.Equal(t, 0, len(plan2.Requirements))
	}
	if plan2.Tasks != nil {
		assert.Equal(t, 0, len(plan2.Tasks))
	}

	// Task
	task := Task{
		ID:                 1,
		Description:        "Test",
		Action:             "implement",
		Status:             TaskStatusDone,
		AcceptanceCriteria: []string{"ac1"},
	}
	data, err = json.Marshal(task)
	require.NoError(t, err)
	var task2 Task
	err = json.Unmarshal(data, &task2)
	require.NoError(t, err)
	assert.Equal(t, task.ID, task2.ID)
	// Dependencies and Files may be nil after unmarshal if omitted in JSON
	if task2.Dependencies != nil {
		assert.Equal(t, 0, len(task2.Dependencies))
	}
	if task2.Files != nil {
		assert.Equal(t, 0, len(task2.Files))
	}
	assert.NotNil(t, task2.AcceptanceCriteria)
	assert.Equal(t, 1, len(task2.AcceptanceCriteria))

	// TaskGraph
	tg := TaskGraph{
		ID:        uuid.New(),
		PlanID:    uuid.New(),
		Status:    TaskGraphStatusCompleted,
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	data, err = json.Marshal(tg)
	require.NoError(t, err)
	var tg2 TaskGraph
	err = json.Unmarshal(data, &tg2)
	require.NoError(t, err)
	assert.Equal(t, tg.ID, tg2.ID)
	if tg2.Tasks != nil {
		assert.Equal(t, 0, len(tg2.Tasks))
	}

	// Agent
	agent := Agent{
		ID:        uuid.New(),
		Role:      AgentRoleTester,
		Status:    AgentStatusCompleted,
		StartedAt: time.Now(),
	}
	data, err = json.Marshal(agent)
	require.NoError(t, err)
	var agent2 Agent
	err = json.Unmarshal(data, &agent2)
	require.NoError(t, err)
	assert.Equal(t, agent.ID, agent2.ID)
	if agent2.Capabilities != nil {
		assert.Equal(t, 0, len(agent2.Capabilities))
	}

	// ToolSpec
	tool := ToolSpec{
		Name:          "test",
		Description:   "Test tool",
		RiskLevel:     RiskDangerous,
		ResourceScope: ResourceScopeProject,
	}
	data, err = json.Marshal(tool)
	require.NoError(t, err)
	var tool2 ToolSpec
	err = json.Unmarshal(data, &tool2)
	require.NoError(t, err)
	assert.Equal(t, tool.Name, tool2.Name)
	if tool2.Capabilities != nil {
		assert.Equal(t, 0, len(tool2.Capabilities))
	}
	if tool2.SideEffects != nil {
		assert.Equal(t, 0, len(tool2.SideEffects))
	}
	assert.NotNil(t, tool2.SideEffects)
	assert.Equal(t, 0, len(tool2.SideEffects))

	// Artifact
	art := Artifact{
		ID:        uuid.New(),
		Name:      "test.md",
		Type:      ArtifactTypeCode,
		Path:      "/test/test.md",
		CreatedAt: time.Now(),
		CreatedBy: uuid.New(),
	}
	data, err = json.Marshal(art)
	require.NoError(t, err)
	var art2 Artifact
	err = json.Unmarshal(data, &art2)
	require.NoError(t, err)
	assert.Equal(t, art.ID, art2.ID)

	// Verification
	ver := Verification{
		ID:      uuid.New(),
		Level:   VerificationLevelIntegration,
		Verdict: VerificationVerdictPassed,
	}
	data, err = json.Marshal(ver)
	require.NoError(t, err)
	var ver2 Verification
	err = json.Unmarshal(data, &ver2)
	require.NoError(t, err)
	assert.Equal(t, ver.ID, ver2.ID)
	if ver2.Criteria != nil {
		assert.Equal(t, 0, len(ver2.Criteria))
	}
	if ver2.Evidence != nil {
		assert.Equal(t, 0, len(ver2.Evidence))
	}

	// Checkpoint
	cp := Checkpoint{
		ID:          uuid.New(),
		Type:        CheckpointTypeDecision,
		Description: "Test",
		Risk:        "medium",
		Status:      CheckpointStatusResolved,
		CreatedAt:   time.Now(),
	}
	data, err = json.Marshal(cp)
	require.NoError(t, err)
	var cp2 Checkpoint
	err = json.Unmarshal(data, &cp2)
	require.NoError(t, err)
	assert.Equal(t, cp.ID, cp2.ID)
	if cp2.Options != nil {
		assert.Equal(t, 0, len(cp2.Options))
	}

	// Research
	res := Research{
		ID:         uuid.New(),
		Question:   "Test?",
		Findings:   "Answer",
		Confidence: 0.9,
		CreatedAt:  time.Now(),
		CreatedBy:  uuid.New(),
	}
	data, err = json.Marshal(res)
	require.NoError(t, err)
	var res2 Research
	err = json.Unmarshal(data, &res2)
	require.NoError(t, err)
	assert.Equal(t, res.ID, res2.ID)
	if res2.Sources != nil {
		assert.Equal(t, 0, len(res2.Sources))
	}
}

func TestEnums(t *testing.T) {
	// Test all enum constants have correct string values
	assert.Equal(t, "trivial", string(ComplexityTrivial))
	assert.Equal(t, "simple", string(ComplexitySimple))
	assert.Equal(t, "moderate", string(ComplexityModerate))
	assert.Equal(t, "complex", string(ComplexityComplex))

	assert.Equal(t, "feature", string(IntentFeature))
	assert.Equal(t, "bugfix", string(IntentBugfix))
	assert.Equal(t, "refactor", string(IntentRefactor))
	assert.Equal(t, "question", string(IntentQuestion))
	assert.Equal(t, "explanation", string(IntentExplanation))
	assert.Equal(t, "exploration", string(IntentExploration))
	assert.Equal(t, "chore", string(IntentChore))

	assert.Equal(t, "planned", string(RunStatusPlanned))
	assert.Equal(t, "running", string(RunStatusRunning))
	assert.Equal(t, "paused", string(RunStatusPaused))
	assert.Equal(t, "completed", string(RunStatusCompleted))
	assert.Equal(t, "failed", string(RunStatusFailed))

	assert.Equal(t, "pending", string(RequirementStatusPending))
	assert.Equal(t, "active", string(RequirementStatusActive))
	assert.Equal(t, "completed", string(RequirementStatusCompleted))
	assert.Equal(t, "deferred", string(RequirementStatusDeferred))
	assert.Equal(t, "dropped", string(RequirementStatusDropped))

	assert.Equal(t, "proposed", string(DecisionStatusProposed))
	assert.Equal(t, "accepted", string(DecisionStatusAccepted))
	assert.Equal(t, "rejected", string(DecisionStatusRejected))
	assert.Equal(t, "superseded", string(DecisionStatusSuperseded))

	assert.Equal(t, "draft", string(PlanStatusDraft))
	assert.Equal(t, "approved", string(PlanStatusApproved))
	assert.Equal(t, "executing", string(PlanStatusExecuting))
	assert.Equal(t, "completed", string(PlanStatusCompleted))
	assert.Equal(t, "failed", string(PlanStatusFailed))

	assert.Equal(t, "pending", string(TaskStatusPending))
	assert.Equal(t, "running", string(TaskStatusRunning))
	assert.Equal(t, "done", string(TaskStatusDone))
	assert.Equal(t, "failed", string(TaskStatusFailed))
	assert.Equal(t, "skipped", string(TaskStatusSkipped))
	assert.Equal(t, "unrecoverable", string(TaskStatusUnrecoverable))

	assert.Equal(t, "planned", string(TaskGraphStatusPlanned))
	assert.Equal(t, "validated", string(TaskGraphStatusValidated))
	assert.Equal(t, "executing", string(TaskGraphStatusExecuting))
	assert.Equal(t, "completed", string(TaskGraphStatusCompleted))
	assert.Equal(t, "failed", string(TaskGraphStatusFailed))

	assert.Equal(t, "supervisor", string(AgentRoleSupervisor))
	assert.Equal(t, "implementer", string(AgentRoleImplementer))
	assert.Equal(t, "tester", string(AgentRoleTester))

	assert.Equal(t, "filesystem.read", string(CapabilityFilesystemRead))
	assert.Equal(t, "shell.execute", string(CapabilityShellExecute))

	assert.Equal(t, "interactive", string(PermissionPolicyInteractive))
	assert.Equal(t, "deny", string(PermissionPolicyDeny))
	assert.Equal(t, "allow", string(PermissionPolicyAllow))
	assert.Equal(t, "ci", string(PermissionPolicyCI))

	assert.Equal(t, "document", string(ArtifactTypeDocument))
	assert.Equal(t, "code", string(ArtifactTypeCode))

	assert.Equal(t, "structural", string(VerificationLevelStructural))
	assert.Equal(t, "unit", string(VerificationLevelUnit))
	assert.Equal(t, "integration", string(VerificationLevelIntegration))
	assert.Equal(t, "behavioral", string(VerificationLevelBehavioral))
	assert.Equal(t, "architectural", string(VerificationLevelArchitectural))

	assert.Equal(t, "passed", string(VerificationVerdictPassed))
	assert.Equal(t, "failed", string(VerificationVerdictFailed))
	assert.Equal(t, "pending", string(VerificationVerdictPending))
	assert.Equal(t, "skipped", string(VerificationVerdictSkipped))

	assert.Equal(t, "human_verify", string(CheckpointTypeHumanVerify))
	assert.Equal(t, "decision", string(CheckpointTypeDecision))
	assert.Equal(t, "human_action", string(CheckpointTypeHumanAction))
	assert.Equal(t, "blocking_human", string(CheckpointTypeBlockingHuman))

	assert.Equal(t, "pending", string(CheckpointStatusPending))
	assert.Equal(t, "active", string(CheckpointStatusActive))
	assert.Equal(t, "resolved", string(CheckpointStatusResolved))
	assert.Equal(t, "skipped", string(CheckpointStatusSkipped))
}
