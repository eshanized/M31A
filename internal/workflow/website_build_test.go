package workflow

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

// --- PromptRegistry Tests ---

func TestPromptRegistry_WebsiteBuildPromptLoaded(t *testing.T) {
	registry, err := LoadPrompts()
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	if registry.WebsiteBuild == "" {
		t.Fatal("WebsiteBuild prompt is empty — website-build.md not loaded")
	}
}

func TestPromptRegistry_WebsiteBuildContainsDesignSystem(t *testing.T) {
	registry, err := LoadPrompts()
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	for _, want := range []string{
		"Design System",
		"Color Tokens",
		"Typography",
		"Dark",
		"Pre-built Components",
	} {
		if !strings.Contains(registry.WebsiteBuild, want) {
			t.Errorf("WebsiteBuild prompt missing section %q", want)
		}
	}
}

func TestPromptRegistry_WebsiteBuildContainsShadcnComponents(t *testing.T) {
	registry, err := LoadPrompts()
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	for _, want := range []string{
		"Pre-built Components",
		"shadcn/ui",
		"button.tsx",
		"card.tsx",
		"accordion.tsx",
		"@/components/ui/button",
	} {
		if !strings.Contains(registry.WebsiteBuild, want) {
			t.Errorf("WebsiteBuild prompt missing %q", want)
		}
	}
}

func TestPromptRegistry_WebsiteBuildContainsPageList(t *testing.T) {
	registry, err := LoadPrompts()
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	for _, want := range []string{
		"app/page.tsx",
		"app/about/page.tsx",
		"app/pricing/page.tsx",
		"app/contact/page.tsx",
		"app/blog/page.tsx",
		"app/not-found.tsx",
	} {
		if !strings.Contains(registry.WebsiteBuild, want) {
			t.Errorf("WebsiteBuild prompt missing page path %q", want)
		}
	}
}

func TestPromptRegistry_WebsiteBuildContainsColorPalettes(t *testing.T) {
	registry, err := LoadPrompts()
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	for _, want := range []string{
		"SaaS",
		"E-commerce",
		"Portfolio",
		"Blog",
		"Corporate",
	} {
		if !strings.Contains(registry.WebsiteBuild, want) {
			t.Errorf("WebsiteBuild prompt missing palette %q", want)
		}
	}
}

// --- ScopeIncludes Tests ---

func TestScopeIncludes_WebsiteInScope(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Intent:     m31types.IntentFeature,
		Complexity: m31types.ComplexityComplex,
		Confidence: 0.9,
		Scope:      []string{"website", "nextjs", "ui"},
		Summary:    "Build a website with Next.js",
	})
	if !engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes('website') returned false for scope [website, nextjs, ui]")
	}
}

func TestScopeIncludes_WebsiteNotInScope(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Intent:     m31types.IntentFeature,
		Complexity: m31types.ComplexitySimple,
		Confidence: 0.8,
		Scope:      []string{"auth", "middleware"},
		Summary:    "Add authentication",
	})
	if engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes('website') returned true for scope [auth, middleware]")
	}
}

func TestScopeIncludes_NilIntentResult(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// intentResult is nil by default
	if engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes('website') returned true with nil intentResult")
	}
}

func TestScopeIncludes_CaseInsensitive(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Scope: []string{"Website"},
	})
	if !engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes should be case-insensitive")
	}
	if !engine.ScopeIncludes("WEBSITE") {
		t.Error("ScopeIncludes should be case-insensitive")
	}
}

func TestScopeIncludes_EmptyScope(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Scope: []string{},
	})
	if engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes('website') returned true for empty scope")
	}
}

// --- ExtractWebsiteTemplate Tests ---

func TestExtractWebsiteTemplate_CreatesFiles(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	// Check core config files
	for _, file := range []string{
		"package.json",
		"next.config.ts",
		"tsconfig.json",
		"postcss.config.mjs",
		"tailwind.config.ts",
		"components.json",
		".gitignore",
	} {
		path := filepath.Join(dest, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s to exist", file)
		}
	}
}

func TestExtractWebsiteTemplate_CreatesAppFiles(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	for _, file := range []string{
		"app/globals.css",
		"app/layout.tsx",
		"app/page.tsx",
		"app/not-found.tsx",
	} {
		path := filepath.Join(dest, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s to exist", file)
		}
	}
}

func TestExtractWebsiteTemplate_CreatesComponentFiles(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	for _, file := range []string{
		"components/ui/button.tsx",
		"components/ui/card.tsx",
		"components/ui/input.tsx",
		"components/ui/textarea.tsx",
		"components/ui/badge.tsx",
		"components/ui/accordion.tsx",
		"components/ui/tabs.tsx",
		"components/ui/dialog.tsx",
		"components/ui/sheet.tsx",
		"components/ui/select.tsx",
		"components/ui/label.tsx",
		"components/ui/separator.tsx",
		"components/ui/navigation-menu.tsx",
		"components/ui/sonner.tsx",
	} {
		path := filepath.Join(dest, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected component %s to exist", file)
		}
	}
}

func TestExtractWebsiteTemplate_CreatesLibFiles(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	for _, file := range []string{
		"lib/utils.ts",
		"lib/constants.ts",
	} {
		path := filepath.Join(dest, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s to exist", file)
		}
	}
}

func TestExtractWebsiteTemplate_CreatesHookFiles(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	for _, file := range []string{
		"hooks/use-media-query.ts",
		"hooks/use-scroll.ts",
	} {
		path := filepath.Join(dest, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s to exist", file)
		}
	}
}

// --- Template Content Validation Tests ---

func TestExtractWebsiteTemplate_PackageJSONHasRadixDeps(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(dest, "package.json"))
	if err != nil {
		t.Fatalf("failed to read package.json: %v", err)
	}
	content := string(data)

	for _, dep := range []string{
		"@radix-ui/react-accordion",
		"@radix-ui/react-dialog",
		"@radix-ui/react-tabs",
		"@radix-ui/react-select",
		"@radix-ui/react-label",
		"@radix-ui/react-separator",
		"@radix-ui/react-slot",
		"@radix-ui/react-navigation-menu",
		"class-variance-authority",
		"clsx",
		"tailwind-merge",
		"next-themes",
		"lucide-react",
		"sonner",
	} {
		if !strings.Contains(content, dep) {
			t.Errorf("package.json missing dependency %s", dep)
		}
	}
}

func TestExtractWebsiteTemplate_GlobalsCSSHasShadcnVars(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(dest, "app/globals.css"))
	if err != nil {
		t.Fatalf("failed to read globals.css: %v", err)
	}
	content := string(data)

	for _, want := range []string{
		"--background:",
		"--foreground:",
		"--primary:",
		"--secondary:",
		"--muted:",
		"--accent:",
		"--destructive:",
		"--border:",
		"--input:",
		"--ring:",
		"--radius:",
		".dark",
	} {
		if !strings.Contains(content, want) {
			t.Errorf("globals.css missing CSS variable/selector %q", want)
		}
	}
}

func TestExtractWebsiteTemplate_ComponentsJSONisValid(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(dest, "components.json"))
	if err != nil {
		t.Fatalf("failed to read components.json: %v", err)
	}
	content := string(data)

	for _, want := range []string{
		`"style": "default"`,
		`"rsc": true`,
		`"tsx": true`,
		`"@/components/ui"`,
		`"@/lib/utils"`,
	} {
		if !strings.Contains(content, want) {
			t.Errorf("components.json missing %q", want)
		}
	}
}

func TestExtractWebsiteTemplate_ButtonComponentHasVariants(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(dest, "components/ui/button.tsx"))
	if err != nil {
		t.Fatalf("failed to read button.tsx: %v", err)
	}
	content := string(data)

	for _, want := range []string{
		"buttonVariants",
		"default",
		"destructive",
		"outline",
		"secondary",
		"ghost",
		"link",
		"asChild",
		"@radix-ui/react-slot",
	} {
		if !strings.Contains(content, want) {
			t.Errorf("button.tsx missing %q", want)
		}
	}
}

func TestExtractWebsiteTemplate_AccordionComponentUsesRadix(t *testing.T) {
	dest := t.TempDir()
	if err := ExtractWebsiteTemplate(dest); err != nil {
		t.Fatalf("ExtractWebsiteTemplate failed: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(dest, "components/ui/accordion.tsx"))
	if err != nil {
		t.Fatalf("failed to read accordion.tsx: %v", err)
	}
	content := string(data)

	for _, want := range []string{
		"@radix-ui/react-accordion",
		"AccordionItem",
		"AccordionTrigger",
		"AccordionContent",
		"ChevronDown",
	} {
		if !strings.Contains(content, want) {
			t.Errorf("accordion.tsx missing %q", want)
		}
	}
}

// --- ExtractWebsiteTemplateTo Tests ---

func TestExtractWebsiteTemplateTo_CreatesTempDir(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Scope: []string{"website"},
	})

	dir, err := engine.ExtractWebsiteTemplateTo()
	if err != nil {
		t.Fatalf("ExtractWebsiteTemplateTo failed: %v", err)
	}
	if dir == "" {
		t.Fatal("ExtractWebsiteTemplateTo returned empty path")
	}
	if _, err := os.Stat(dir); os.IsNotExist(err) {
		t.Fatalf("extracted directory %s does not exist", dir)
	}

	// Verify a key file exists
	if _, err := os.Stat(filepath.Join(dir, "package.json")); os.IsNotExist(err) {
		t.Error("package.json not found in extracted directory")
	}
}

func TestExtractWebsiteTemplateTo_CachesResult(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Scope: []string{"website"},
	})

	dir1, err := engine.ExtractWebsiteTemplateTo()
	if err != nil {
		t.Fatalf("first ExtractWebsiteTemplateTo failed: %v", err)
	}

	dir2, err := engine.ExtractWebsiteTemplateTo()
	if err != nil {
		t.Fatalf("second ExtractWebsiteTemplateTo failed: %v", err)
	}

	if dir1 != dir2 {
		t.Errorf("expected cached result, got different paths: %s vs %s", dir1, dir2)
	}
}

// --- Intent Classification Integration Tests ---

func TestIntentClassification_WebsiteScope(t *testing.T) {
	// Simulate what the LLM classifier should return for "build a website with nextjs"
	raw := `{"intent":"feature","complexity":"complex","confidence":0.92,"scope":["website","nextjs","ui"],"summary":"Build a website with Next.js"}`
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("parseIntentJSON failed: %v", err)
	}

	// Verify scope includes "website"
	found := false
	for _, s := range result.Scope {
		if s == "website" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected 'website' in scope, got %v", result.Scope)
	}

	// Verify the engine would detect this
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(result)
	if !engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes('website') returned true for website scope")
	}
}

func TestIntentClassification_NonWebsiteScope(t *testing.T) {
	// Simulate a non-website feature request
	raw := `{"intent":"feature","complexity":"simple","confidence":0.88,"scope":["auth","api"],"summary":"Add JWT authentication middleware"}`
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("parseIntentJSON failed: %v", err)
	}

	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(result)
	if engine.ScopeIncludes("website") {
		t.Error("ScopeIncludes('website') returned true for non-website scope")
	}
}

// --- Prompt Injection Tests ---

func TestBuildSystemPrompt_WebsiteScopeIncludesWebsiteBuild(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Scope: []string{"website", "nextjs"},
	})

	// Simulate what buildPlanContext does
	extras := []string{engine.promptBuilder.Prompt("tool-use"), engine.promptBuilder.Prompt("plan-format"), engine.promptBuilder.Prompt("context-awareness"), engine.promptBuilder.Prompt("code-quality"), engine.promptBuilder.Prompt("code-intelligence")}
	if engine.ScopeIncludes("website") && engine.promptBuilder.Prompt("website-build") != "" {
		extras = append(extras, engine.promptBuilder.Prompt("website-build"))
	}

	prompt := engine.buildSystemPrompt(extras...)
	if !strings.Contains(prompt, "Design System") {
		t.Error("buildSystemPrompt with website scope missing Design System section")
	}
	if !strings.Contains(prompt, "Pre-built Components") {
		t.Error("buildSystemPrompt with website scope missing Pre-built Components section")
	}
}

func TestBuildSystemPrompt_NonWebsiteScopeExcludesWebsiteBuild(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetIntentResult(&m31types.IntentResult{
		Scope: []string{"auth", "api"},
	})

	extras := []string{engine.promptBuilder.Prompt("tool-use"), engine.promptBuilder.Prompt("plan-format"), engine.promptBuilder.Prompt("context-awareness"), engine.promptBuilder.Prompt("code-quality"), engine.promptBuilder.Prompt("code-intelligence")}
	if engine.ScopeIncludes("website") && engine.promptBuilder.Prompt("website-build") != "" {
		extras = append(extras, engine.promptBuilder.Prompt("website-build"))
	}

	prompt := engine.buildSystemPrompt(extras...)
	if strings.Contains(prompt, "Pre-built Components") {
		t.Error("buildSystemPrompt without website scope should NOT include WebsiteBuild prompt")
	}
}

func TestBuildSystemPrompt_NilScopeExcludesWebsiteBuild(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// intentResult is nil

	extras := []string{engine.promptBuilder.Prompt("tool-use"), engine.promptBuilder.Prompt("plan-format"), engine.promptBuilder.Prompt("context-awareness"), engine.promptBuilder.Prompt("code-quality"), engine.promptBuilder.Prompt("code-intelligence")}
	if engine.ScopeIncludes("website") && engine.promptBuilder.Prompt("website-build") != "" {
		extras = append(extras, engine.promptBuilder.Prompt("website-build"))
	}

	prompt := engine.buildSystemPrompt(extras...)
	if strings.Contains(prompt, "Pre-built Components") {
		t.Error("buildSystemPrompt with nil scope should NOT include WebsiteBuild prompt")
	}
}

// --- Template Extraction Error Handling Tests ---

func TestExtractWebsiteTemplate_DestinationIsFile(t *testing.T) {
	// Create a regular file as destination — should fail
	dest := filepath.Join(t.TempDir(), "not-a-dir.txt")
	if err := os.WriteFile(dest, []byte("hello"), 0644); err != nil {
		t.Fatalf("failed to create file: %v", err)
	}

	err := ExtractWebsiteTemplate(dest)
	if err == nil {
		t.Error("expected error when destination is a file, got nil")
	}
}
