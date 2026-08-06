package workflow

// Helper and utility methods: config adapters, caching, and template extraction.

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/compaction"
)

// gitConfig returns the git config with safe defaults when cfg is nil.
func (e *Engine) gitConfig() config.GitConfig {
	if e.cfg != nil {
		return e.cfg.Git
	}
	return config.DefaultGitConfig()
}

// promptOrGet returns the named prompt or empty string if not found.
// Errors are logged at warn level. Used by callers where prompt names are
// compile-time constants and a missing prompt is a programming error, not
// a user-facing failure.
func (e *Engine) promptOrGet(name string) string {
	s, err := e.promptBuilder.Prompt(name)
	if err != nil {
		e.logger.Warn("missing prompt template", "name", name, "error", err)
		return ""
	}
	return s
}

// budgetFromConfig extracts the budget limit from config, returning 0 if nil.
func budgetFromConfig(cfg *config.Config) float64 {
	if cfg == nil {
		return 0
	}
	return cfg.Features.BudgetLimitUSD
}

// compactionConfig converts a config.CompactionConfig to a compaction.Config.
func compactionConfig(cfg *config.Config) compaction.Config {
	if cfg == nil {
		return compaction.DefaultConfig()
	}
	c := cfg.Compaction
	if c.Buffer <= 0 {
		c.Buffer = 20000
	}
	if c.KeepTokens <= 0 {
		c.KeepTokens = 8000
	}
	return compaction.Config{
		Auto:                c.Auto,
		Buffer:              c.Buffer,
		KeepTokens:          c.KeepTokens,
		SummaryTemplate:     c.SummaryTemplate,
		SummaryTemplateFile: c.SummaryTemplateFile,
	}
}

// budgetConfigAdapter adapts *config.Config to the budget limit interface
// expected by PhaseCoordinator.PrePhaseSetup.
type budgetConfigAdapter struct {
	cfg *config.Config
}

func (a *budgetConfigAdapter) GetBudgetLimit() float64 {
	if a.cfg == nil {
		return 0
	}
	return a.cfg.Features.BudgetLimitUSD
}

// compactedMessages builds a new message list with the compaction summary
// prepended and old messages replaced. Keeps the last N messages based on
// the compactor's KeepTokens setting.
func (e *Engine) compactedMessages(original []m31types.Message, summary string) []m31types.Message {
	if e.compactor == nil || e.tokens == nil {
		return original
	}
	keepTokens := 8000
	if e.cfg != nil && e.cfg.Compaction.KeepTokens > 0 {
		keepTokens = e.cfg.Compaction.KeepTokens
	}
	_, recent := compaction.SplitMessages(original, keepTokens, e.tokens.Estimate)

	summaryMsg := m31types.Message{
		Role:    "system",
		Content: "[Compacted Session History]\n" + summary,
		Segments: []m31types.MessageSegment{
			{
				Type:    m31types.MessageCompaction,
				Content: summary,
				Visible: false,
			},
		},
		CreatedAt: time.Now(),
	}

	result := make([]m31types.Message, 0, len(recent)+1)
	result = append(result, summaryMsg)
	result = append(result, recent...)
	return result
}

// loadProjectCached returns the cached project state, loading it from disk
// on first access per session. Avoids redundant disk I/O + JSON parse across
// buildDiscussContext, buildPlanContext, buildResearchContext, and buildExecuteContext.
func (e *Engine) loadProjectCached() *m31types.ProjectState {
	e.cacheMu.RLock()
	cache := e.cache
	e.cacheMu.RUnlock()
	if cached := cache.GetProjectShared(e.sessionID); cached != nil {
		e.logger.Debug("project loaded from cache", "session_id", e.sessionID)
		return cached
	}
	project, err := e.sessionMgr.LoadProject(e.sessionID)
	if err != nil {
		e.logger.Warn("failed to load project", "error", err)
		return nil
	}
	cache.SetProjectShared(e.sessionID, project)
	e.logger.Debug("project loaded from disk", "session_id", e.sessionID)
	return project
}

// ExtractWebsiteTemplateTo extracts the bundled website template to a temporary
// directory and stores the path for later injection into the plan/execute context.
// Returns the path to the extracted template, or an error if extraction fails.
func (e *Engine) ExtractWebsiteTemplateTo() (string, error) {
	if e.websiteTemplateDir != "" {
		return e.websiteTemplateDir, nil
	}
	tmpDir, err := os.MkdirTemp("", "m31a-website-template-*")
	if err != nil {
		return "", fmt.Errorf("create temp dir: %w", err)
	}
	if err := ExtractWebsiteTemplate(tmpDir); err != nil {
		_ = os.RemoveAll(tmpDir)
		return "", fmt.Errorf("extract template: %w", err)
	}
	e.websiteTemplateDir = tmpDir
	return tmpDir, nil
}

// ExtractWebsiteTemplate copies the bundled website template into destDir.
func ExtractWebsiteTemplate(destDir string) error {
	srcDir := "templates/website-nextjs"
	return fs.WalkDir(websiteTemplateFS, srcDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return fmt.Errorf("walk template dir: %w", err)
		}
		// Compute the relative path within the template
		relPath, err := filepath.Rel(srcDir, path)
		if err != nil {
			return fmt.Errorf("compute relative path: %w", err)
		}
		if relPath == "." {
			return nil
		}
		dest := filepath.Join(destDir, relPath)
		if d.IsDir() {
			return os.MkdirAll(dest, 0o755)
		}
		data, err := websiteTemplateFS.ReadFile(path)
		if err != nil {
			return fmt.Errorf("read template file %q: %w", path, err)
		}
		// Ensure parent directory exists
		if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
			return fmt.Errorf("create parent dir %q: %w", filepath.Dir(dest), err)
		}
		return os.WriteFile(dest, data, 0o644)
	})
}

// truncateForLog truncates a string to maxLen, adding ellipsis if needed.
func truncateForLog(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
