package tools

import (
	"os"
	"path/filepath"
	"time"

	"github.com/eshanized/M31A/internal/config"
)

func DefaultDispatcher(workDir, backupDir, sessionsDir string, cfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig) (*Dispatcher, error) {
	d := NewDispatcher(cfg)
	d.workDir_ = workDir

	// Initialize output store for tool output bounding
	outputDir := filepath.Join(homeDir(), ".m31a", "tool-output")
	maxLines := DefaultOutputMaxLines
	maxBytes := DefaultOutputMaxBytes
	if toolsCfg != nil {
		if toolsCfg.OutputMaxLines > 0 {
			maxLines = toolsCfg.OutputMaxLines
		}
		if toolsCfg.OutputMaxBytes > 0 {
			maxBytes = toolsCfg.OutputMaxBytes
		}
	}
	store := NewOutputStore(outputDir, maxLines, maxBytes)
	d.SetOutputStore(store)
	// Best-effort cleanup of old output files on startup
	_, _ = store.Cleanup(OutputRetentionDays * 24 * time.Hour)

	// Load persistent permissions for this project
	pp := NewPersistentPermissions()
	if saved := pp.Load(workDir); len(saved) > 0 {
		d.mu.Lock()
		d.rules = append(d.rules, saved...)
		d.mu.Unlock()
	}

	// Extract config values with safe defaults
	bashMaxTimeoutSecs := 1800
	webfetchMaxRetries := 3
	webfetchRetryDelayMs := 100
	var additionalBlockedCommands []string
	var additionalObfuscationPatterns []string
	if toolsCfg != nil {
		if toolsCfg.BashMaxTimeoutSecs > 0 {
			bashMaxTimeoutSecs = toolsCfg.BashMaxTimeoutSecs
		}
		if toolsCfg.WebfetchMaxRetries > 0 {
			webfetchMaxRetries = toolsCfg.WebfetchMaxRetries
		}
		if toolsCfg.WebfetchRetryDelayMs > 0 {
			webfetchRetryDelayMs = toolsCfg.WebfetchRetryDelayMs
		}
		additionalBlockedCommands = toolsCfg.AdditionalBlockedCommands
		additionalObfuscationPatterns = toolsCfg.AdditionalObfuscationPatterns
	}

	if err := d.Register(NewBash(workDir, bashMaxTimeoutSecs, additionalBlockedCommands, additionalObfuscationPatterns)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileRead(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileWrite(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewEdit(workDir, backupDir)); err != nil {
		return nil, err
	}
	todo := NewTodoWrite(sessionsDir, "")
	d.todoWrite = todo
	if err := d.Register(todo); err != nil {
		return nil, err
	}
	todoRead := NewTodoRead(sessionsDir, "")
	d.todoRead = todoRead
	if err := d.Register(todoRead); err != nil {
		return nil, err
	}
	if err := d.Register(NewWebFetch(sessionsDir, false, webfetchMaxRetries, webfetchRetryDelayMs)); err != nil {
		return nil, err
	}
	webSearchBaseURL := ""
	if toolsCfg != nil && toolsCfg.WebSearchBaseURL != "" {
		webSearchBaseURL = toolsCfg.WebSearchBaseURL
	}
	if err := d.Register(NewWebSearch(webSearchBaseURL)); err != nil {
		return nil, err
	}
	if err := d.Register(NewAskUserQuestion(d.questionReqCh, d.questionRespCh, &d.pendingQuestions)); err != nil {
		return nil, err
	}
	if err := d.Register(NewGlob(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewGrep(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileList(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileDelete(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileMove(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewCodeMap(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewCodeComplexity(workDir, nil)); err != nil {
		return nil, err
	}
	if err := d.Register(NewDevServer(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewHTTPCheck()); err != nil {
		return nil, err
	}
	if err := d.Register(NewGit(workDir)); err != nil {
		return nil, err
	}
	return d, nil
}

func homeDir() string {
	if h, err := os.UserHomeDir(); err == nil {
		return h
	}
	return filepath.Join(os.TempDir(), ".m31a")
}
