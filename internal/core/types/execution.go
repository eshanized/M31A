package types

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type AgentRole string

const (
	AgentRoleSupervisor      AgentRole = "supervisor"
	AgentRoleExplorer        AgentRole = "explorer"
	AgentRoleResearcher      AgentRole = "researcher"
	AgentRoleArchitect       AgentRole = "architect"
	AgentRolePlanner         AgentRole = "planner"
	AgentRoleImplementer     AgentRole = "implementer"
	AgentRoleTester          AgentRole = "tester"
	AgentRoleDebugger        AgentRole = "debugger"
	AgentRoleVerifier        AgentRole = "verifier"
	AgentRoleReviewer        AgentRole = "reviewer"
	AgentRoleSecurityAuditor AgentRole = "security_auditor"
	AgentRoleGitSpecialist   AgentRole = "git_specialist"
	AgentRoleReleaseEngineer AgentRole = "release_engineer"
)

type Agent struct {
	ID           uuid.UUID    `json:"id"`
	Role         AgentRole    `json:"role"`
	Capabilities []Capability `json:"capabilities"`
	Status       AgentStatus  `json:"status"`
	StartedAt    time.Time    `json:"started_at"`
	CompletedAt  *time.Time   `json:"completed_at,omitempty"`
}

type AgentStatus string

const (
	AgentStatusIdle      AgentStatus = "idle"
	AgentStatusRunning   AgentStatus = "running"
	AgentStatusWaiting   AgentStatus = "waiting"
	AgentStatusCompleted AgentStatus = "completed"
	AgentStatusFailed    AgentStatus = "failed"
)

type Capability string

const (
	CapabilityFilesystemRead  Capability = "filesystem.read"
	CapabilityFilesystemWrite Capability = "filesystem.write"
	CapabilityShellExecute    Capability = "shell.execute"
	CapabilityGitRead         Capability = "git.read"
	CapabilityGitWrite        Capability = "git.write"
	CapabilityNetworkHTTP     Capability = "network.http"
	CapabilityProviderCall    Capability = "provider.call"
	CapabilityKeychainRead    Capability = "keychain.read"
	CapabilityKeychainWrite   Capability = "keychain.write"
)

type PermissionPolicy string

const (
	PermissionPolicyInteractive PermissionPolicy = "interactive"
	PermissionPolicyDeny        PermissionPolicy = "deny"
	PermissionPolicyAllow       PermissionPolicy = "allow"
	PermissionPolicyCI          PermissionPolicy = "ci"
)

type ResourceScope string

const (
	ResourceScopeWorkspace ResourceScope = "workspace"
	ResourceScopeProject   ResourceScope = "project"
	ResourceScopeGlobal    ResourceScope = "global"
)

type MutationClass string

const (
	MutationClassReadOnly MutationClass = "read_only"
	MutationClassLocal    MutationClass = "local"
	MutationClassRemote   MutationClass = "remote"
)

// ToolSpec represents a tool definition in the domain model (Execution plane).
// This is distinct from the Tool interface in types.go which is for execution.
type ToolSpec struct {
	Name          string        `json:"name"`
	Description   string        `json:"description"`
	InputSchema   string        `json:"input_schema"`
	OutputSchema  string        `json:"output_schema"`
	Capabilities  []Capability  `json:"capabilities"`
	RiskLevel     RiskLevel     `json:"risk_level"`
	ResourceScope ResourceScope `json:"resource_scope"`
	MutationClass MutationClass `json:"mutation_class"`
	SideEffects   []string      `json:"side_effects"`
	Idempotent    bool          `json:"idempotent"`
}

func (a Agent) MarshalJSON() ([]byte, error) {
	if a.Capabilities == nil {
		a.Capabilities = []Capability{}
	}
	type agentAlias Agent
	return json.Marshal(agentAlias(a))
}

func (t ToolSpec) MarshalJSON() ([]byte, error) {
	if t.Capabilities == nil {
		t.Capabilities = []Capability{}
	}
	if t.SideEffects == nil {
		t.SideEffects = []string{}
	}
	type toolSpecAlias ToolSpec
	return json.Marshal(toolSpecAlias(t))
}
