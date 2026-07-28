package tools

import (
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/integrations/metrics"
	"github.com/eshanized/M31A/internal/tools/ai"
	toolsExec "github.com/eshanized/M31A/internal/tools/exec"
	"github.com/eshanized/M31A/internal/tools/fileops"
	"github.com/eshanized/M31A/internal/tools/search"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/tools/todo"
)

func init() {
	ai.NewDispatcher = NewDispatcher
}

// Re-export SetVersion from search package
func SetVersion(v string) {
	search.SetVersion(v)
}

// Re-export NewAgent from ai package
func NewAgent(m *subagent.Manager, isChild bool, depth int, profiles map[string]config.SubagentProfileConfig) *ai.Agent {
	return ai.NewAgent(m, isChild, depth, profiles)
}

// Re-export NewDispatcherFactory from ai package
func NewDispatcherFactory(backupDir, sessionsDir string, permCfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig, manager *subagent.Manager, profiles map[string]config.SubagentProfileConfig) subagent.DispatcherFactory {
	return ai.NewDispatcherFactory(backupDir, sessionsDir, permCfg, toolsCfg, manager, profiles)
}

// Re-export tool constructors from sub-packages (only those NOT already in main tools package)
func NewBash(workDir string, maxTimeoutSecs int, additionalBlockedCommands []string, additionalObfuscationPatterns []string) *toolsExec.Bash {
	return toolsExec.NewBash(workDir, maxTimeoutSecs, additionalBlockedCommands, additionalObfuscationPatterns)
}

func NewFileRead(workDir string) *fileops.FileRead {
	return fileops.NewFileRead(workDir)
}

func NewFileWrite(workDir, backupDir string) *fileops.FileWrite {
	return fileops.NewFileWrite(workDir, backupDir)
}

func NewEdit(workDir, backupDir string) *fileops.Edit {
	return fileops.NewEdit(workDir, backupDir)
}

func NewGlob(workDir string) *search.Glob {
	return search.NewGlob(workDir)
}

func NewGrep(workDir string) *search.Grep {
	return search.NewGrep(workDir)
}

// Re-export utility functions from fileops for backward compatibility
func LevenshteinDistance(a, b string) int {
	return fileops.LevenshteinDistance(a, b)
}

func HumanSize(b int64) string {
	return fileops.HumanSize(b)
}

// Re-export NewMetricsTool from search package
func NewMetricsTool(collector *metrics.Collector) *search.MetricsTool {
	return search.NewMetricsTool(collector)
}

// Re-export TodoItem type from todo package
type TodoItem = todo.TodoItem
