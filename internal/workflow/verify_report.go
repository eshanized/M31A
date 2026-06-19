package workflow

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
)

// VerifyReport holds the structured verification report.
type VerifyReport struct {
	SessionID   string
	Timestamp   time.Time
	Total       int
	Passed      int
	Failed      int
	Skipped     int
	Results     []VerifyTaskReport
	Security    []SecurityFinding
	ManualSteps []string
}

// VerifyTaskReport holds per-task verification results.
type VerifyTaskReport struct {
	TaskID      int
	Description string
	Status      string
	FilesExist  bool
	BuildOK     bool
	TestsOK     bool
	Errors      []string
	Duration    time.Duration
}

// SecurityFinding holds a security-related finding from scanning task files.
type SecurityFinding struct {
	Severity string // "high", "medium", "low"
	File     string
	Line     int
	Pattern  string
	Message  string
}

// generateVerifyReport creates a structured markdown verification report.
func (e *Engine) generateVerifyReport(tasks []m31types.Task, verifyResults map[int]VerificationResult) string {
	report := VerifyReport{
		SessionID: e.sessionID,
		Timestamp: time.Now(),
	}

	for _, task := range tasks {
		tr := VerifyTaskReport{
			TaskID:      task.ID,
			Description: task.Description,
			Status:      string(task.Status),
		}
		if vr, ok := verifyResults[task.ID]; ok {
			tr.FilesExist = vr.FilesExist
			tr.BuildOK = vr.SyntaxOK
			tr.TestsOK = vr.TestsOK
			tr.Errors = vr.Errors
		}
		report.Results = append(report.Results, tr)

		report.Total++
		switch task.Status {
		case m31types.StatusDone:
			report.Passed++
		case m31types.StatusFailed, m31types.StatusUnrecoverable:
			report.Failed++
		case m31types.StatusSkipped:
			report.Skipped++
		}
	}

	// Collect manual verification steps from plan
	if plan, err := ParsePlan(e.planMarkdown); err == nil && plan != nil {
		report.ManualSteps = plan.Verification.Manual
	}

	// Security scanning
	secEnabled := e.cfg != nil && e.cfg.Features.VerifySecurity
	if secEnabled {
		report.Security = e.scanSecurityFindings(tasks)
	}

	return formatVerifyReport(report)
}

// scanSecurityFindings scans task files for common security anti-patterns.
func (e *Engine) scanSecurityFindings(tasks []m31types.Task) []SecurityFinding {
	var findings []SecurityFinding

	// Patterns to detect
	patterns := []struct {
		pattern  string
		severity string
		message  string
	}{
		{"password", "high", "Possible hardcoded password"},
		{"secret", "high", "Possible hardcoded secret"},
		{"api_key", "high", "Possible hardcoded API key"},
		{"sk-", "high", "Possible API key prefix"},
		{"TODO: security", "medium", "Security TODO marker"},
		{"eval(", "medium", "Dynamic code execution (eval)"},
		{"exec(", "medium", "Dynamic code execution (exec)"},
		{"innerHTML", "low", "Potential XSS via innerHTML"},
		{"dangerouslySetInnerHTML", "low", "React dangerouslySetInnerHTML usage"},
		{"0.0.0.0", "low", "Binding to all interfaces"},
		{"TLS_INSECURE", "high", "Insecure TLS configuration"},
		{"VERIFY_NONE", "high", "Disabled certificate verification"},
	}

	seen := make(map[string]bool)
	for _, task := range tasks {
		for _, f := range task.Files {
			fullPath := filepath.Join(e.workDir, f)
			data, err := os.ReadFile(fullPath)
			if err != nil {
				continue
			}
			lines := strings.Split(string(data), "\n")
			for lineNum, line := range lines {
				lineLower := strings.ToLower(line)
				for _, p := range patterns {
					if strings.Contains(lineLower, strings.ToLower(p.pattern)) {
						key := fmt.Sprintf("%s:%d:%s", f, lineNum+1, p.pattern)
						if !seen[key] {
							seen[key] = true
							findings = append(findings, SecurityFinding{
								Severity: p.severity,
								File:     f,
								Line:     lineNum + 1,
								Pattern:  p.pattern,
								Message:  p.message,
							})
						}
					}
				}
			}
		}
	}

	return findings
}

// formatVerifyReport renders the verification report as markdown.
func formatVerifyReport(report VerifyReport) string {
	var sb strings.Builder

	sb.WriteString("# Verification Report\n\n")
	sb.WriteString(fmt.Sprintf("**Session:** %s  \n", report.SessionID))
	sb.WriteString(fmt.Sprintf("**Timestamp:** %s  \n", report.Timestamp.Format("2006-01-02 15:04:05")))

	passRate := 0
	if report.Total > 0 {
		passRate = (report.Passed * 100) / report.Total
	}
	sb.WriteString(fmt.Sprintf("**Pass Rate:** %d%% (%d/%d)\n\n", passRate, report.Passed, report.Total))

	// Per-task results
	sb.WriteString("## Task Results\n\n")
	sb.WriteString("| ID | Description | Files | Build | Tests | Status |\n")
	sb.WriteString("|----|-------------|-------|-------|-------|--------|\n")
	for _, r := range report.Results {
		fileIcon := "✗"
		if r.FilesExist {
			fileIcon = "✓"
		}
		buildIcon := "-"
		if r.FilesExist {
			if r.BuildOK {
				buildIcon = "✓"
			} else {
				buildIcon = "✗"
			}
		}
		testIcon := "-"
		if r.TestsOK {
			testIcon = "✓"
		} else if !r.BuildOK {
			testIcon = "-"
		} else {
			testIcon = "✗"
		}

		desc := r.Description
		if len(desc) > 50 {
			desc = desc[:47] + "..."
		}
		sb.WriteString(fmt.Sprintf("| %d | %s | %s | %s | %s | %s |\n",
			r.TaskID, desc, fileIcon, buildIcon, testIcon, r.Status))
	}

	// Errors
	hasErrors := false
	for _, r := range report.Results {
		if len(r.Errors) > 0 {
			hasErrors = true
			break
		}
	}
	if hasErrors {
		sb.WriteString("\n## Errors\n\n")
		for _, r := range report.Results {
			for _, err := range r.Errors {
				sb.WriteString(fmt.Sprintf("- **Task %d:** %s\n", r.TaskID, err))
			}
		}
	}

	// Security findings
	if len(report.Security) > 0 {
		sb.WriteString("\n## Security Findings\n\n")
		for _, f := range report.Security {
			sb.WriteString(fmt.Sprintf("- **[%s]** %s:%d — %s (`%s`)\n",
				strings.ToUpper(f.Severity), f.File, f.Line, f.Message, f.Pattern))
		}
	}

	// Manual steps
	if len(report.ManualSteps) > 0 {
		sb.WriteString("\n## Manual Verification Steps\n\n")
		for _, step := range report.ManualSteps {
			sb.WriteString(fmt.Sprintf("- [ ] %s\n", step))
		}
	}

	return sb.String()
}
