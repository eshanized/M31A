package tools

import (
	"os/exec"
	"sync"

	"github.com/eshanized/M31A/internal/tools/ai"
	"github.com/eshanized/M31A/internal/tools/exec"
	"github.com/eshanized/M31A/internal/tools/fileops"
	"github.com/eshanized/M31A/internal/tools/search"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/types"
)

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
func NewBash(workDir string, maxTimeoutSecs int, additionalBlockedCommands []string, additionalObfuscationPatterns []string) *exec.Bash {
	return exec.NewBash(workDir, maxTimeoutSecs, additionalBlockedCommands, additionalObfuscationPatterns)
}

func NewFileRead(workDir string) *fileops.FileRead {
	return fileops.NewFileRead(workDir)
}

func NewFileWrite(workDir, backupDir string) *fileops.FileWrite {
	return fileops.NewFileWrite(workDir, backupDir)
}

func NewFileList(workDir string) *fileops.FileList {
	return fileops.NewFileList(workDir)
}

func NewFileDelete(workDir, backupDir string) *fileops.FileDelete {
	return fileops.NewFileDelete(workDir, backupDir)
}

func NewFileMove(workDir, backupDir string) *fileops.FileMove {
	return fileops.NewFileMove(workDir, backupDir)
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

func NewWebFetch(workDir string, timeoutSecs int, dnsCache *search.DNSCache) *search.WebFetch {
	return search.NewWebFetch(workDir, timeoutSecs, dnsCache)
}

func NewWebSearch(baseURL string) *search.WebSearch {
	return search.NewWebSearch(baseURL)
}

func NewDevServer(workDir string) *exec.DevServer {
	return exec.NewDevServer(workDir)
}

func NewAskUserQuestion(requestCh chan types.QuestionRequest, responseCh chan types.QuestionResponse, pending *sync.Map) *ai.AskUserQuestion {
	return ai.NewAskUserQuestion(requestCh, responseCh, pending)
}

func CheckDangerousCommand(command string, additionalBlocked []string, additionalObfuscation []string) (string, bool) {
	return exec.CheckDangerousCommand(command, additionalBlocked, additionalObfuscation)
}

func ScrubEnvironment(cmd *exec.Cmd) {
	exec.ScrubEnvironment(cmd)
}
