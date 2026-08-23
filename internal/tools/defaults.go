package tools

import (
	"os"
	"path/filepath"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/tools/ai"
	"github.com/eshanized/M31A/internal/tools/codeanalysis"
	"github.com/eshanized/M31A/internal/tools/exec"
	"github.com/eshanized/M31A/internal/tools/fileops"
	"github.com/eshanized/M31A/internal/tools/git"
	"github.com/eshanized/M31A/internal/tools/network"
	"github.com/eshanized/M31A/internal/tools/search"
	"github.com/eshanized/M31A/internal/tools/todo"
)

// DefaultDispatcher creates a Dispatcher with default settings for the given context.
// The policy parameter allows specifying the permission policy (interactive, headless, CI, etc.).
// If policy is nil, defaults to HeadlessDenyPolicy for safety.
func DefaultDispatcher(workDir, backupDir, sessionsDir string, cfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig, policy PermissionDecider) (*Dispatcher, error) {
	// If no policy provided, create one based on config
	if policy == nil {
		if cfg != nil {
			switch cfg.DefaultMode {
			case "allow-safe":
				policy = NewHeadlessAllowDecider() // Allow safe operations
			case "deny-all":
				policy = NewHeadlessDenyDecider()
			case "prompt":
				// For interactive mode, we'd need an InteractivePolicy with channels
				// But in defaults.go we don't have channels, so default to deny
				policy = NewHeadlessDenyDecider()
			default:
				policy = NewHeadlessDenyDecider()
			}
		} else {
			policy = NewHeadlessDenyDecider()
		}
	}
	d := newDispatcher(cfg, policy)
	d.workDir_ = workDir

	// Load persistent permissions for this project
	if d.persistentPerms != nil {
		persistentRules := d.persistentPerms.Load(workDir)
		if len(persistentRules) > 0 {
			// Filter out expired rules
			now := time.Now()
			for _, rule := range persistentRules {
				if !isRuleExpired(rule, now) {
					d.rules = append(d.rules, rule)
				}
			}
		}
	}

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
	store := exec.NewOutputStore(outputDir, maxLines, maxBytes)
	d.SetOutputStore(store)
	// Best-effort cleanup of old output files on startup
	_, _ = store.Cleanup(OutputRetentionDays * 24 * time.Hour)

	// Extract config values with safe defaults
	bashMaxTimeoutSecs := 1800
	webfetchMaxRetries := 3
	var additionalBlockedCommands []string
	var additionalObfuscationPatterns []string
	if toolsCfg != nil {
		if toolsCfg.BashMaxTimeoutSecs > 0 {
			bashMaxTimeoutSecs = toolsCfg.BashMaxTimeoutSecs
		}
		if toolsCfg.WebfetchMaxRetries > 0 {
			webfetchMaxRetries = toolsCfg.WebfetchMaxRetries
		}
		additionalBlockedCommands = toolsCfg.AdditionalBlockedCommands
		additionalObfuscationPatterns = toolsCfg.AdditionalObfuscationPatterns
	}

	// Create DNS cache for web fetch
	dnsCache := search.NewDNSCache(search.DNSCacheTTL, 64)

	// Register fileops tools
	if err := d.Register(fileops.NewFileRead(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(fileops.NewFileWrite(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(fileops.NewEdit(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(fileops.NewFileList(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(fileops.NewFileDelete(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(fileops.NewFileMove(workDir, backupDir)); err != nil {
		return nil, err
	}

	// Register exec tools
	if err := d.Register(exec.NewBash(workDir, bashMaxTimeoutSecs, additionalBlockedCommands, additionalObfuscationPatterns)); err != nil {
		return nil, err
	}
	if err := d.Register(exec.NewDevServer(workDir)); err != nil {
		return nil, err
	}

	// Register search tools
	if err := d.Register(search.NewWebFetch(sessionsDir, webfetchMaxRetries, dnsCache)); err != nil {
		return nil, err
	}
	webSearchBaseURL := ""
	if toolsCfg != nil && toolsCfg.WebSearchBaseURL != "" {
		webSearchBaseURL = toolsCfg.WebSearchBaseURL
	}
	if err := d.Register(search.NewWebSearch(webSearchBaseURL)); err != nil {
		return nil, err
	}
	if err := d.Register(search.NewGlob(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(search.NewGrep(workDir)); err != nil {
		return nil, err
	}

	// Register AI tools
	if err := d.Register(ai.NewAskUserQuestion(d.questionReqCh, d.questionRespCh, &d.pendingQuestions)); err != nil {
		return nil, err
	}

	// Register todo tools (still at root)
	t := todo.NewTodoWrite(sessionsDir, "")
	d.todoWrite = t
	if err := d.Register(t); err != nil {
		return nil, err
	}
	tr := todo.NewTodoRead(sessionsDir, "")
	d.todoRead = tr
	if err := d.Register(tr); err != nil {
		return nil, err
	}

	// Register remaining root tools
	if err := d.Register(codeanalysis.NewCodeMap(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(codeanalysis.NewCodeComplexity(workDir, nil)); err != nil {
		return nil, err
	}
	if err := d.Register(network.NewHTTPCheck()); err != nil {
		return nil, err
	}
	if err := d.Register(git.NewGit(workDir)); err != nil {
		return nil, err
	}
	return d, nil
}

// NewDispatcher implements ai.ToolDispatcher interface for use by subagents.
// This is a factory function that creates a Dispatcher configured for subagent workspaces.
func NewDispatcher(workDir, backupDir, sessionsDir string, permCfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig) (ai.ToolDispatcher, error) {
	return DefaultDispatcher(workDir, backupDir, sessionsDir, permCfg, toolsCfg, nil)
}

func homeDir() string {
	if h, err := os.UserHomeDir(); err == nil {
		return h
	}
	return filepath.Join(os.TempDir(), ".m31a")
}
