package workflow

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// SmokeTestResult holds the result of a single HTTP smoke test.
type SmokeTestResult struct {
	Route      string `json:"route"`
	Method     string `json:"method"`
	Status     int    `json:"status"`
	Passed     bool   `json:"passed"`
	Error      string `json:"error,omitempty"`
	DurationMs int64  `json:"duration_ms"`
	BodySize   int    `json:"body_size"`
	HasContent bool   `json:"has_content"`
}

// RuntimeSummary holds the overall runtime verification results.
type RuntimeSummary struct {
	ProjectType string            `json:"project_type"`
	ServerURL   string            `json:"server_url"`
	ServerReady bool              `json:"server_ready"`
	Tests       []SmokeTestResult `json:"tests"`
	TotalPassed int               `json:"total_passed"`
	TotalFailed int               `json:"total_failed"`
	TotalTests  int               `json:"total_tests"`
	Errors      []string          `json:"errors,omitempty"`
	DurationMs  int64             `json:"duration_ms"`
}

// runtimeServerEntry tracks a managed dev server process.
type runtimeServerEntry struct {
	cmd  *exec.Cmd
	port int
}

// runRuntime starts a dev server, runs smoke tests, and reports results.
func (e *Engine) runRuntime(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("runtime phase starting", "goal", goal)

	e.emit(IntermediateProgressMsg{
		Phase:   "runtime",
		Message: "Starting runtime verification...",
	})

	runtimeCtx, cancel := context.WithTimeout(ctx, 3*time.Minute)
	defer cancel()

	summary := &RuntimeSummary{}
	phaseStart := time.Now()

	projectType := detectProjectType(e.workDir)
	summary.ProjectType = projectType

	var server *runtimeServerEntry
	var serverURL string

	switch projectType {
	case "nodejs":
		server, serverURL = e.startNodeJSServer(runtimeCtx)
	case "python":
		server, serverURL = e.startPythonServer(runtimeCtx)
	case "go":
		server, serverURL = e.startGoServer(runtimeCtx)
	case "rust":
		server, serverURL = e.startRustServer(runtimeCtx)
	default:
		server, serverURL = e.startStaticServer(runtimeCtx)
	}

	if server != nil {
		defer e.stopRuntimeServer(server)
	}

	summary.ServerURL = serverURL
	if serverURL == "" {
		e.logger.Warn("runtime: no dev server started, skipping smoke tests")
		summary.Errors = append(summary.Errors, "could not start dev server for runtime verification")
		return e.finishRuntime(summary, phaseStart)
	}

	ready := e.waitForServerReady(runtimeCtx, serverURL, 30*time.Second)
	summary.ServerReady = ready

	if !ready {
		e.logger.Warn("runtime: dev server not ready", "url", serverURL)
		summary.Errors = append(summary.Errors, fmt.Sprintf("dev server at %s not ready after 30s", serverURL))
		return e.finishRuntime(summary, phaseStart)
	}

	e.emit(IntermediateProgressMsg{
		Phase:   "runtime",
		Message: fmt.Sprintf("Server ready at %s — running smoke tests...", serverURL),
	})

	summary.Tests = e.runSmokeTests(runtimeCtx, serverURL, goal)

	for _, test := range summary.Tests {
		if test.Passed {
			summary.TotalPassed++
		} else {
			summary.TotalFailed++
		}
	}
	summary.TotalTests = len(summary.Tests)

	e.emit(RuntimeCheckCompleteMsg{
		Summary: *summary,
	})

	return e.finishRuntime(summary, phaseStart)
}

// finishRuntime creates the PhaseResult and saves session state.
func (e *Engine) finishRuntime(summary *RuntimeSummary, phaseStart time.Time) (*PhaseResult, error) {
	summary.DurationMs = time.Since(phaseStart).Milliseconds()

	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseRuntime,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("runtime: save checkpoint failed", "error", err)
	}

	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseRuntime,
		fmt.Sprintf("%d/%d checks passed", summary.TotalPassed, summary.TotalTests),
		"runtime verification complete"); err != nil {
		e.logger.Warn("runtime: save state failed", "error", err)
	}

	allPassed := summary.TotalFailed == 0 && len(summary.Errors) == 0

	result := &PhaseResult{
		Phase:          m31types.PhaseRuntime,
		Success:        allPassed,
		RuntimeSummary: summary,
	}
	if !allPassed {
		var errMsgs []string
		errMsgs = append(errMsgs, summary.Errors...)
		for _, test := range summary.Tests {
			if !test.Passed {
				errMsgs = append(errMsgs, fmt.Sprintf("%s %s: %s", test.Method, test.Route, test.Error))
			}
		}
		result.Error = strings.Join(errMsgs, "; ")
	}

	e.logger.Info("runtime phase complete",
		"passed", summary.TotalPassed,
		"failed", summary.TotalFailed,
		"total", summary.TotalTests)

	return result, nil
}

// startNodeJSServer starts a Node.js dev server.
func (e *Engine) startNodeJSServer(ctx context.Context) (*runtimeServerEntry, string) {
	port := findFreePort()
	if port == 0 {
		return nil, ""
	}

	pm := detectPackageManager(e.workDir)
	devCmd := "npm run dev"
	if pm != "" {
		devCmd = strings.Replace(pm, "run", "run dev", 1)
		if pm == "yarn" {
			devCmd = "yarn dev"
		}
	}

	cmd := exec.CommandContext(ctx, "sh", "-c", fmt.Sprintf("PORT=%d %s", port, devCmd))
	cmd.Dir = e.workDir
	cmd.Env = append(os.Environ(), fmt.Sprintf("PORT=%d", port))

	if err := cmd.Start(); err != nil {
		e.logger.Warn("runtime: failed to start Node.js dev server", "error", err)
		return nil, ""
	}

	return &runtimeServerEntry{cmd: cmd, port: port}, fmt.Sprintf("http://localhost:%d", port)
}

// startPythonServer starts a Python dev server.
func (e *Engine) startPythonServer(ctx context.Context) (*runtimeServerEntry, string) {
	port := findFreePort()
	if port == 0 {
		return nil, ""
	}

	cmd := exec.CommandContext(ctx, "python3", "-m", "http.server", fmt.Sprintf("%d", port))
	cmd.Dir = e.workDir

	if err := cmd.Start(); err != nil {
		e.logger.Warn("runtime: failed to start Python server", "error", err)
		return nil, ""
	}

	return &runtimeServerEntry{cmd: cmd, port: port}, fmt.Sprintf("http://localhost:%d", port)
}

// startGoServer starts a Go dev server.
func (e *Engine) startGoServer(ctx context.Context) (*runtimeServerEntry, string) {
	port := findFreePort()
	if port == 0 {
		return nil, ""
	}

	cmd := exec.CommandContext(ctx, "go", "run", ".")
	cmd.Dir = e.workDir
	cmd.Env = append(os.Environ(), fmt.Sprintf("PORT=%d", port))

	if err := cmd.Start(); err != nil {
		e.logger.Warn("runtime: failed to start Go server", "error", err)
		return nil, ""
	}

	return &runtimeServerEntry{cmd: cmd, port: port}, fmt.Sprintf("http://localhost:%d", port)
}

// startRustServer starts a Rust dev server.
func (e *Engine) startRustServer(ctx context.Context) (*runtimeServerEntry, string) {
	port := findFreePort()
	if port == 0 {
		return nil, ""
	}

	cmd := exec.CommandContext(ctx, "cargo", "run")
	cmd.Dir = e.workDir
	cmd.Env = append(os.Environ(), fmt.Sprintf("PORT=%d", port))

	if err := cmd.Start(); err != nil {
		e.logger.Warn("runtime: failed to start Rust server", "error", err)
		return nil, ""
	}

	return &runtimeServerEntry{cmd: cmd, port: port}, fmt.Sprintf("http://localhost:%d", port)
}

// startStaticServer starts a simple static file server for HTML projects.
func (e *Engine) startStaticServer(ctx context.Context) (*runtimeServerEntry, string) {
	port := findFreePort()
	if port == 0 {
		return nil, ""
	}

	distDir := e.workDir
	for _, candidate := range []string{"dist", "build", "public", "out", "_site", "."} {
		p := filepath.Join(e.workDir, candidate)
		if info, err := os.Stat(p); err == nil && info.IsDir() {
			if candidate != "." {
				if hasIndexHTML(p) {
					distDir = p
					break
				}
			} else if hasIndexHTML(p) {
				distDir = p
				break
			}
		}
	}

	cmd := exec.CommandContext(ctx, "python3", "-m", "http.server", fmt.Sprintf("%d", port))
	cmd.Dir = distDir

	if err := cmd.Start(); err != nil {
		e.logger.Warn("runtime: failed to start static server", "error", err)
		return nil, ""
	}

	return &runtimeServerEntry{cmd: cmd, port: port}, fmt.Sprintf("http://localhost:%d", port)
}

// stopRuntimeServer kills the dev server process and its children.
func (e *Engine) stopRuntimeServer(server *runtimeServerEntry) {
	if server.cmd == nil || server.cmd.Process == nil {
		return
	}
	if pgid, err := getProcessGroup(server.cmd.Process.Pid); err == nil {
		_ = killProcessGroup(pgid)
	} else {
		_ = server.cmd.Process.Kill()
	}
	_ = server.cmd.Wait()
}

// waitForServerReady polls the URL until it responds or timeout expires.
func (e *Engine) waitForServerReady(ctx context.Context, url string, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	client := &http.Client{Timeout: 3 * time.Second}

	for time.Now().Before(deadline) {
		if ctx.Err() != nil {
			return false
		}
		req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
		if err != nil {
			time.Sleep(500 * time.Millisecond)
			continue
		}
		resp, err := client.Do(req)
		if err == nil {
			resp.Body.Close()
			if resp.StatusCode < 500 {
				return true
			}
		}
		time.Sleep(500 * time.Millisecond)
	}
	return false
}

// runSmokeTests executes HTTP smoke tests against common routes.
func (e *Engine) runSmokeTests(ctx context.Context, baseURL string, goal string) []SmokeTestResult {
	routes := e.discoverRoutes(goal)

	var mu sync.Mutex
	results := make([]SmokeTestResult, len(routes))
	var wg sync.WaitGroup

	const maxConcurrency = 4
	sem := make(chan struct{}, maxConcurrency)

	for i, route := range routes {
		wg.Add(1)
		go func(idx int, r string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			result := e.smokeTestRoute(ctx, baseURL+r)
			mu.Lock()
			results[idx] = result
			mu.Unlock()
		}(i, route)
	}

	wg.Wait()
	return results
}

// smokeTestRoute makes a GET request and validates the response.
func (e *Engine) smokeTestRoute(ctx context.Context, url string) SmokeTestResult {
	start := time.Now()
	client := &http.Client{Timeout: 10 * time.Second}

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return SmokeTestResult{
			Route:      url,
			Method:     "GET",
			Passed:     false,
			Error:      fmt.Sprintf("request creation failed: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}
	}

	resp, err := client.Do(req)
	if err != nil {
		return SmokeTestResult{
			Route:      url,
			Method:     "GET",
			Passed:     false,
			Error:      fmt.Sprintf("request failed: %v", err),
			DurationMs: time.Since(start).Milliseconds(),
		}
	}
	defer resp.Body.Close()

	body := make([]byte, 1024*1024)
	n, _ := resp.Body.Read(body)
	body = body[:n]

	passed := resp.StatusCode >= 200 && resp.StatusCode < 400
	hasContent := n > 100

	var errMsg string
	if resp.StatusCode >= 400 {
		errMsg = fmt.Sprintf("HTTP %d", resp.StatusCode)
	} else if n < 50 {
		errMsg = "response body appears empty or incomplete"
		passed = false
	}

	return SmokeTestResult{
		Route:      url,
		Method:     "GET",
		Status:     resp.StatusCode,
		Passed:     passed,
		Error:      errMsg,
		DurationMs: time.Since(start).Milliseconds(),
		BodySize:   n,
		HasContent: hasContent,
	}
}

// discoverRoutes generates a list of routes to test based on the goal and project structure.
func (e *Engine) discoverRoutes(goal string) []string {
	routes := []string{"/"}
	lower := strings.ToLower(goal)

	routeHints := map[string][]string{
		"about":     {"/about"},
		"menu":      {"/menu"},
		"contact":   {"/contact"},
		"blog":      {"/blog"},
		"login":     {"/login"},
		"signup":    {"/signup", "/register"},
		"api":       {"/api", "/api/health"},
		"docs":      {"/docs"},
		"home":      {"/"},
		"product":   {"/products"},
		"cart":      {"/cart"},
		"checkout":  {"/checkout"},
		"search":    {"/search"},
		"profile":   {"/profile"},
		"settings":  {"/settings"},
		"admin":     {"/admin"},
		"dashboard": {"/dashboard"},
	}

	seen := map[string]bool{"/": true}
	for keyword, paths := range routeHints {
		if strings.Contains(lower, keyword) {
			for _, p := range paths {
				if !seen[p] {
					routes = append(routes, p)
					seen[p] = true
				}
			}
		}
	}

	routes = append(routes, e.discoverRoutesFromFileSystem()...)

	return routes
}

// discoverRoutesFromFileSystem checks for common page files and adds corresponding routes.
func (e *Engine) discoverRoutesFromFileSystem() []string {
	var routes []string
	seen := map[string]bool{}

	pagePatterns := []struct {
		file  string
		route string
	}{
		{"index.html", "/"},
		{"about.html", "/about"},
		{"contact.html", "/contact"},
		{"menu.html", "/menu"},
		{"src/pages/about.tsx", "/about"},
		{"src/pages/contact.tsx", "/contact"},
		{"src/pages/index.tsx", "/"},
		{"src/pages/menu.tsx", "/menu"},
		{"app/about/page.tsx", "/about"},
		{"app/contact/page.tsx", "/contact"},
		{"app/page.tsx", "/"},
		{"pages/about.tsx", "/about"},
		{"pages/contact.tsx", "/contact"},
		{"pages/index.tsx", "/"},
	}

	for _, pp := range pagePatterns {
		path := filepath.Join(e.workDir, pp.file)
		if _, err := os.Stat(path); err == nil && !seen[pp.route] {
			routes = append(routes, pp.route)
			seen[pp.route] = true
		}
	}

	return routes
}

// findFreePort asks the OS for a free TCP port.
func findFreePort() int {
	l, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		return 0
	}
	port := l.Addr().(*net.TCPAddr).Port
	l.Close()
	return port
}

// hasIndexHTML checks if a directory contains an index.html file.
func hasIndexHTML(dir string) bool {
	_, err := os.Stat(filepath.Join(dir, "index.html"))
	return err == nil
}
