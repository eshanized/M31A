package workflow

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/pkg/types"
)

// ProjectAnalysis holds the results of deep project analysis.
type ProjectAnalysis struct {
	ProjectType     string
	Framework       string
	Language        string
	DependencyCount int
	TestFileRatio   float64
	FileCount       int
	HealthScore     int
	Details         []string
}

// PreflightCheck holds the results of environment pre-flight checks.
type PreflightCheck struct {
	Passed         bool
	RuntimeVersion string
	DiskSpaceOK    bool
	GitRemoteOK    bool
	Issues         []string
}

// runDeepAnalysis performs deep project analysis beyond basic type detection.
func (e *Engine) runDeepAnalysis(workDir string) ProjectAnalysis {
	analysis := ProjectAnalysis{
		ProjectType: detectProjectType(workDir),
	}

	// Deep framework detection
	analysis.Framework = detectFramework(workDir, analysis.ProjectType)
	analysis.Language = detectLanguage(analysis.ProjectType)

	// Dependency audit
	analysis.DependencyCount = countDependencies(workDir, analysis.ProjectType)

	// File count and test ratio
	analysis.FileCount = countProjectFiles(workDir)
	analysis.TestFileRatio = calculateTestRatio(workDir)

	// Health score
	analysis.HealthScore = calculateHealthScore(analysis)

	// Details
	if analysis.Framework != "" {
		analysis.Details = append(analysis.Details,
			fmt.Sprintf("Framework: %s", analysis.Framework))
	}
	if analysis.DependencyCount > 0 {
		analysis.Details = append(analysis.Details,
			fmt.Sprintf("Dependencies: %d", analysis.DependencyCount))
	}
	analysis.Details = append(analysis.Details,
		fmt.Sprintf("Test coverage ratio: %.0f%%", analysis.TestFileRatio*100))
	analysis.Details = append(analysis.Details,
		fmt.Sprintf("Health score: %d/100", analysis.HealthScore))

	return analysis
}

// detectFramework performs deep framework detection beyond the basic project type.
func detectFramework(workDir, projectType string) string {
	switch projectType {
	case "nodejs":
		return detectNodeFramework(workDir)
	case "python":
		return detectPythonFramework(workDir)
	case "go":
		return detectGoFramework(workDir)
	case "rust":
		return detectRustFramework(workDir)
	}
	return ""
}

// detectNodeFramework detects the specific Node.js framework.
func detectNodeFramework(workDir string) string {
	pkgPath := filepath.Join(workDir, "package.json")
	data, err := os.ReadFile(pkgPath)
	if err != nil {
		return ""
	}

	var pkg struct {
		Dependencies    map[string]string `json:"dependencies"`
		DevDependencies map[string]string `json:"devDependencies"`
	}
	if err := json.Unmarshal(data, &pkg); err != nil {
		return ""
	}

	allDeps := make(map[string]string)
	for k, v := range pkg.Dependencies {
		allDeps[k] = v
	}
	for k, v := range pkg.DevDependencies {
		allDeps[k] = v
	}

	// Check frameworks in priority order
	frameworks := []struct {
		pkg  string
		name string
	}{
		{"next", "Next.js"},
		{"nuxt", "Nuxt"},
		{"remix", "Remix"},
		{"@remix-run/react", "Remix"},
		{"svelte", "Svelte"},
		{"@sveltejs/kit", "SvelteKit"},
		{"astro", "Astro"},
		{"gatsby", "Gatsby"},
		{"react", "React"},
		{"vue", "Vue"},
		{"@angular/core", "Angular"},
		{"express", "Express"},
		{"fastify", "Fastify"},
		{"hono", "Hono"},
		{"koa", "Koa"},
	}

	for _, fw := range frameworks {
		if _, ok := allDeps[fw.pkg]; ok {
			return fw.name
		}
	}

	return "Node.js"
}

// detectPythonFramework detects the specific Python framework.
func detectPythonFramework(workDir string) string {
	// Check for common Python project files
	checks := []struct {
		file    string
		content string
		name    string
	}{
		{"requirements.txt", "django", "Django"},
		{"requirements.txt", "flask", "Flask"},
		{"requirements.txt", "fastapi", "FastAPI"},
		{"requirements.txt", "streamlit", "Streamlit"},
		{"pyproject.toml", "django", "Django"},
		{"pyproject.toml", "flask", "Flask"},
		{"pyproject.toml", "fastapi", "FastAPI"},
	}

	for _, check := range checks {
		data, err := os.ReadFile(filepath.Join(workDir, check.file))
		if err != nil {
			continue
		}
		if strings.Contains(strings.ToLower(string(data)), check.content) {
			return check.name
		}
	}

	return "Python"
}

// detectGoFramework detects Go frameworks/libraries.
func detectGoFramework(workDir string) string {
	goMod, err := os.ReadFile(filepath.Join(workDir, "go.mod"))
	if err != nil {
		return ""
	}
	content := string(goMod)

	frameworks := []struct {
		module string
		name   string
	}{
		{"github.com/gin-gonic/gin", "Gin"},
		{"github.com/labstack/echo", "Echo"},
		{"github.com/gofiber/fiber", "Fiber"},
		{"github.com/go-chi/chi", "Chi"},
		{"github.com/gorilla/mux", "Gorilla Mux"},
		{"github.com/bufbuild/connect-go", "ConnectRPC"},
		{"google.golang.org/grpc", "gRPC"},
	}

	for _, fw := range frameworks {
		if strings.Contains(content, fw.module) {
			return fw.name
		}
	}

	return "Go"
}

// detectRustFramework detects Rust frameworks.
func detectRustFramework(workDir string) string {
	cargo, err := os.ReadFile(filepath.Join(workDir, "Cargo.toml"))
	if err != nil {
		return ""
	}
	content := string(cargo)

	frameworks := []struct {
		crate string
		name  string
	}{
		{"actix-web", "Actix-web"},
		{"axum", "Axum"},
		{"rocket", "Rocket"},
		{"warp", "Warp"},
		{"tauri", "Tauri"},
		{"bevy", "Bevy"},
	}

	for _, fw := range frameworks {
		if strings.Contains(content, fw.crate) {
			return fw.name
		}
	}

	return "Rust"
}

// detectLanguage maps project type to primary language.
func detectLanguage(projectType string) string {
	switch projectType {
	case "go":
		return "Go"
	case "nodejs":
		return "TypeScript/JavaScript"
	case "python":
		return "Python"
	case "rust":
		return "Rust"
	case "java":
		return "Java"
	case "cc":
		return "C/C++"
	}
	return "Unknown"
}

// countDependencies counts project dependencies from lock files or manifests.
func countDependencies(workDir, projectType string) int {
	switch projectType {
	case "nodejs":
		return countNodeDeps(workDir)
	case "go":
		return countGoDeps(workDir)
	case "python":
		return countPythonDeps(workDir)
	case "rust":
		return countRustDeps(workDir)
	}
	return 0
}

func countNodeDeps(workDir string) int {
	data, err := os.ReadFile(filepath.Join(workDir, "package.json"))
	if err != nil {
		return 0
	}
	var pkg struct {
		Dependencies    map[string]string `json:"dependencies"`
		DevDependencies map[string]string `json:"devDependencies"`
	}
	if err := json.Unmarshal(data, &pkg); err != nil {
		return 0
	}
	return len(pkg.Dependencies) + len(pkg.DevDependencies)
}

func countGoDeps(workDir string) int {
	data, err := os.ReadFile(filepath.Join(workDir, "go.mod"))
	if err != nil {
		return 0
	}
	count := 0
	inRequire := false
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "require (") {
			inRequire = true
			continue
		}
		if trimmed == ")" {
			inRequire = false
			continue
		}
		if inRequire && trimmed != "" && !strings.HasPrefix(trimmed, "//") {
			count++
		}
	}
	return count
}

func countPythonDeps(workDir string) int {
	data, err := os.ReadFile(filepath.Join(workDir, "requirements.txt"))
	if err != nil {
		return 0
	}
	count := 0
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed != "" && !strings.HasPrefix(trimmed, "#") && !strings.HasPrefix(trimmed, "-") {
			count++
		}
	}
	return count
}

func countRustDeps(workDir string) int {
	data, err := os.ReadFile(filepath.Join(workDir, "Cargo.toml"))
	if err != nil {
		return 0
	}
	count := 0
	inDeps := false
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed == "[dependencies]" {
			inDeps = true
			continue
		}
		if strings.HasPrefix(trimmed, "[") {
			inDeps = false
			continue
		}
		if inDeps && strings.Contains(trimmed, "=") {
			count++
		}
	}
	return count
}

// calculateTestRatio calculates the ratio of test files to source files.
func calculateTestRatio(workDir string) float64 {
	var total, testFiles int
	_ = filepath.WalkDir(workDir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		rel, _ := filepath.Rel(workDir, path)
		if rel == "." {
			return nil
		}
		if d.IsDir() {
			name := d.Name()
			if name == ".git" || name == "node_modules" || name == "vendor" || name == "target" || name == ".venv" {
				return filepath.SkipDir
			}
			return nil
		}
		name := d.Name()
		ext := filepath.Ext(name)
		if ext == ".go" || ext == ".js" || ext == ".ts" || ext == ".py" || ext == ".rs" {
			total++
			if strings.HasSuffix(name, "_test.go") ||
				strings.HasSuffix(name, ".test.js") ||
				strings.HasSuffix(name, ".test.ts") ||
				strings.HasSuffix(name, ".spec.js") ||
				strings.HasSuffix(name, ".spec.ts") ||
				strings.HasPrefix(name, "test_") {
				testFiles++
			}
		}
		return nil
	})
	if total == 0 {
		return 0
	}
	return float64(testFiles) / float64(total)
}

// calculateHealthScore computes a project health score (0-100).
func calculateHealthScore(a ProjectAnalysis) int {
	score := 50 // base score

	// Test ratio bonus (up to +25)
	if a.TestFileRatio > 0 {
		bonus := int(a.TestFileRatio * 50)
		if bonus > 25 {
			bonus = 25
		}
		score += bonus
	}

	// Dependency count signal (too many = warning)
	if a.DependencyCount > 100 {
		score -= 10
	} else if a.DependencyCount > 50 {
		score -= 5
	}

	// Has framework = structured project (+10)
	if a.Framework != "" && a.Framework != a.Language {
		score += 10
	}

	// File count signal (very small project = lower confidence)
	if a.FileCount < 5 {
		score -= 10
	}

	if score < 0 {
		score = 0
	}
	if score > 100 {
		score = 100
	}

	return score
}

// runEnvironmentPreflight checks runtime environment before workflow starts.
func (e *Engine) runEnvironmentPreflight() PreflightCheck {
	check := PreflightCheck{Passed: true}

	// Detect runtime version
	projectType := detectProjectType(e.workDir)
	switch projectType {
	case "go":
		out, err := exec.Command("go", "version").Output()
		if err != nil {
			check.Issues = append(check.Issues, "Go runtime not found")
			check.Passed = false
		} else {
			check.RuntimeVersion = strings.TrimSpace(string(out))
		}
	case "nodejs":
		out, err := exec.Command("node", "--version").Output()
		if err != nil {
			check.Issues = append(check.Issues, "Node.js runtime not found")
			check.Passed = false
		} else {
			check.RuntimeVersion = "Node.js " + strings.TrimSpace(string(out))
		}
	case "python":
		out, err := exec.Command("python3", "--version").Output()
		if err != nil {
			check.Issues = append(check.Issues, "Python3 runtime not found")
			check.Passed = false
		} else {
			check.RuntimeVersion = strings.TrimSpace(string(out))
		}
	case "rust":
		out, err := exec.Command("rustc", "--version").Output()
		if err != nil {
			check.Issues = append(check.Issues, "Rust toolchain not found")
			check.Passed = false
		} else {
			check.RuntimeVersion = strings.TrimSpace(string(out))
		}
	}

	// Disk space check (basic: try to write a temp file)
	tmpFile := filepath.Join(e.workDir, ".m31a", ".preflight_check")
	if err := os.MkdirAll(filepath.Dir(tmpFile), m31types.DirPermission); err == nil {
		if err := os.WriteFile(tmpFile, []byte("ok"), m31types.FilePermission); err != nil {
			check.Issues = append(check.Issues, "Cannot write to working directory")
			check.Passed = false
			check.DiskSpaceOK = false
		} else {
			check.DiskSpaceOK = true
			_ = os.Remove(tmpFile)
		}
	}

	// Git remote check
	if e.git != nil {
		out, err := e.git.Run("remote")
		if err != nil || strings.TrimSpace(out) == "" {
			check.GitRemoteOK = false
			check.Issues = append(check.Issues, "No git remote configured")
		} else {
			check.GitRemoteOK = true
		}
	}

	return check
}
