package eventstore

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/memory/artifacts"
	"github.com/google/uuid"
	_ "modernc.org/sqlite"
)

var (
	ErrMigrationFailed     = errors.New("migration failed")
	ErrArtifactParseFailed = errors.New("artifact parse failed")
	ErrAlreadyMigrated     = errors.New("already migrated: .m31a/ already exists")
)

const (
	migrationSessionID = "00000000-0000-0000-0000-000000000000"
)

type MigrationPayload struct {
	PlanningDir string `json:"planning_dir"`
	M31aDir     string `json:"m31a_dir"`
	Description string `json:"description,omitempty"`
}

type MigrationCounts struct {
	Requirements int
	Phases       int
	Decisions    int
	Research     int
	CodebaseMaps int
	EventsTotal  int
}

func marshal(v interface{}) json.RawMessage {
	data, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return data
}

func Migrate(ctx context.Context, planningDir, m31aDir string) (*MigrationCounts, error) {
	// Validate paths
	absPlanningDir, err := filepath.Abs(planningDir)
	if err != nil {
		return nil, coreerrors.Wrap(err, "resolve planning dir")
	}
	absM31aDir, err := filepath.Abs(m31aDir)
	if err != nil {
		return nil, coreerrors.Wrap(err, "resolve m31a dir")
	}

	// Check if already migrated
	eventsDB := filepath.Join(absM31aDir, "events.db")
	if _, err := os.Stat(eventsDB); err == nil {
		return nil, coreerrors.Wrap(ErrAlreadyMigrated, "events.db already exists at "+eventsDB)
	}

	// Create .m31a/ directory structure
	dirs := []string{"decisions", "research", "codebase", "graphs"}
	for _, d := range dirs {
		if err := os.MkdirAll(filepath.Join(absM31aDir, d), types.DirPermission); err != nil {
			return nil, coreerrors.Wrapf(err, "create dir %s", d)
		}
	}

	// Initialize EventStore
	store, err := NewEventStore(eventsDB)
	if err != nil {
		return nil, coreerrors.Wrap(err, "create event store")
	}
	defer store.Close()

	// Create ProjectionManager
	pm := NewProjectionManager(store)
	pm.Register(NewProjectProjection())
	pm.Register(NewRequirementsProjection())
	pm.Register(NewDecisionsProjection())
	pm.Register(NewResearchProjection())
	pm.Register(NewRunProjection())

	// Emit MigrationStarted event
	migrationEvt := types.Event{
		Type:      types.EventMigrationStarted,
		Payload:   marshal(MigrationPayload{PlanningDir: absPlanningDir, M31aDir: absM31aDir, Description: "Migrate .planning/ to .m31a/"}),
		SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
	}
	if err := store.Append(ctx, migrationEvt); err != nil {
		return nil, coreerrors.Wrap(err, "append migration start event")
	}

	counts := &MigrationCounts{}

	// Migrate REQUIREMENTS.md
	reqCount, err := migrateRequirements(ctx, store, absPlanningDir, absM31aDir)
	if err != nil {
		return nil, err
	}
	counts.Requirements = reqCount

	// Migrate ROADMAP.md
	phaseCount, err := migrateRoadmap(ctx, store, absPlanningDir, absM31aDir)
	if err != nil {
		return nil, err
	}
	counts.Phases = phaseCount

	// Migrate PROJECT.md
	if err := migrateProject(ctx, store, absPlanningDir, absM31aDir); err != nil {
		return nil, err
	}

	// Migrate STATE.md
	if err := migrateState(ctx, store, absPlanningDir); err != nil {
		return nil, err
	}

	// Migrate CONTEXT.md
	if err := migrateContext(ctx, store, absPlanningDir); err != nil {
		return nil, err
	}

	// Migrate decisions/
	decCount, err := migrateDecisions(ctx, store, absPlanningDir, absM31aDir)
	if err != nil {
		return nil, err
	}
	counts.Decisions = decCount

	// Migrate research/
	resCount, err := migrateResearch(ctx, store, absPlanningDir, absM31aDir)
	if err != nil {
		return nil, err
	}
	counts.Research = resCount

	// Migrate codebase/
	codebaseCount, err := migrateCodebase(ctx, store, absPlanningDir, absM31aDir)
	if err != nil {
		return nil, err
	}
	counts.CodebaseMaps = codebaseCount

	// Migrate config.json
	if err := migrateConfig(ctx, store, absPlanningDir, absM31aDir); err != nil {
		return nil, err
	}

	// Rebuild all projections
	for _, name := range []string{"project", "requirements", "decisions", "research", "runs"} {
		if err := pm.Rebuild(ctx, name); err != nil {
			return nil, coreerrors.Wrapf(err, "rebuild projection %s", name)
		}
	}

	// Emit MigrationCompleted event
	completedEvt := types.Event{
		Type:      types.EventMigrationCompleted,
		Payload:   marshal(MigrationPayload{
			PlanningDir: absPlanningDir,
			M31aDir:     absM31aDir,
			Description: fmt.Sprintf("Migrated %d requirements, %d phases, %d decisions, %d research, %d codebase maps", counts.Requirements, counts.Phases, counts.Decisions, counts.Research, counts.CodebaseMaps),
		}),
		SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
	}
	if err := store.Append(ctx, completedEvt); err != nil {
		return nil, coreerrors.Wrap(err, "append migration completed event")
	}

	// Archive .planning/
	if err := archivePlanning(absPlanningDir); err != nil {
		return nil, coreerrors.Wrap(err, "archive .planning/")
	}

	counts.EventsTotal = counts.Requirements + counts.Phases + counts.Decisions + counts.Research + counts.CodebaseMaps + 2 // +2 for MigrationStarted/Completed

	return counts, nil
}

func migrateRequirements(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) (int, error) {
	path := filepath.Join(planningDir, "REQUIREMENTS.md")
	content, err := os.ReadFile(path)
	if err != nil {
		return 0, coreerrors.Wrapf(err, "read %s", path)
	}

	requirements := parseRequirements(string(content))
	count := 0
	for _, req := range requirements {
		evt := types.Event{
			Type:      types.EventRequirementCreated,
			Payload:   marshal(req),
			SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
		}
		if err := store.Append(ctx, evt); err != nil {
			return 0, coreerrors.Wrapf(err, "append requirement %s", req.ID)
		}
		count++
	}

	// Write projection
	if err := writeRequirementsProjection(m31aDir, requirements); err != nil {
		return count, coreerrors.Wrap(err, "write requirements projection")
	}

	return count, nil
}

func parseRequirements(content string) []types.Requirement {
	var requirements []types.Requirement
	lines := strings.Split(content, "\n")
	var currentReq *types.Requirement

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "- [") && strings.Contains(line, "] ") {
			// Checkbox item: - [ ] TITLE or - [x] TITLE
			status := types.RequirementStatusPending
			if strings.HasPrefix(line, "- [x] ") || strings.HasPrefix(line, "- [X] ") {
				status = types.RequirementStatusCompleted
			} else if strings.HasPrefix(line, "- [/] ") {
				status = types.RequirementStatusActive
			}

			title := strings.TrimSpace(line[5:]) // Remove "- [x] "
			// Remove phase prefix if present (e.g., "[P1] Title")
			phase := 1
			if strings.HasPrefix(title, "[P") {
				if idx := strings.Index(title, "] "); idx != -1 {
					phaseStr := title[2:idx]
					if p, err := strconv.Atoi(phaseStr); err == nil {
						phase = p
					}
					title = title[idx+2:]
				}
			}

			req := types.Requirement{
				ID:          uuid.New(),
				Title:       title,
				Description: "",
				Phase:       phase,
				Status:      status,
				Traceability: []string{},
				CreatedAt:   time.Now(),
				UpdatedAt:   time.Now(),
			}
			requirements = append(requirements, req)
			currentReq = &requirements[len(requirements)-1]
		} else if currentReq != nil && line != "" && !strings.HasPrefix(line, "- [") && !strings.HasPrefix(line, "#") {
			// Description line
			if currentReq.Description == "" {
				currentReq.Description = line
			} else {
				currentReq.Description += " " + line
			}
		}
	}
	return requirements
}

func writeRequirementsProjection(m31aDir string, requirements []types.Requirement) error {
	var b strings.Builder
	b.WriteString("# Requirements\n\n")
	b.WriteString(fmt.Sprintf("Total: %d\n\n", len(requirements)))

	// Group by phase
	phaseMap := make(map[int][]types.Requirement)
	for _, req := range requirements {
		phaseMap[req.Phase] = append(phaseMap[req.Phase], req)
	}

	phases := make([]int, 0, len(phaseMap))
	for p := range phaseMap {
		phases = append(phases, p)
	}
	// Sort phases
	for i := 0; i < len(phases)-1; i++ {
		for j := i + 1; j < len(phases); j++ {
			if phases[i] > phases[j] {
				phases[i], phases[j] = phases[j], phases[i]
			}
		}
	}

	for _, phase := range phases {
		reqs := phaseMap[phase]
		b.WriteString(fmt.Sprintf("## Phase %d\n\n", phase))
		for _, req := range reqs {
			statusIcon := "⬜"
			switch req.Status {
			case "completed":
				statusIcon = "✅"
			case "active":
				statusIcon = "🔄"
			case "deferred":
				statusIcon = "⏸️"
			case "dropped":
				statusIcon = "❌"
			}
			b.WriteString(fmt.Sprintf("%s %s (ID: %s)\n", statusIcon, req.Title, req.ID.String()[:8]))
			if req.Description != "" {
				b.WriteString(fmt.Sprintf("  %s\n", req.Description))
			}
			b.WriteString("\n")
		}
	}

	return types.AtomicWrite(filepath.Join(m31aDir, "requirements.md"), []byte(b.String()))
}

func migrateRoadmap(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) (int, error) {
	path := filepath.Join(planningDir, "ROADMAP.md")
	content, err := os.ReadFile(path)
	if err != nil {
		return 0, coreerrors.Wrapf(err, "read %s", path)
	}

	phases := parseRoadmap(string(content))
	count := 0
	for _, phase := range phases {
		evt := types.Event{
			Type:      types.EventPlanCreated,
			Payload:   marshal(phase),
			SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
		}
		if err := store.Append(ctx, evt); err != nil {
			return 0, coreerrors.Wrapf(err, "append phase %s", phase.ID)
		}
		count++
	}

	// Write projection
	if err := writeRoadmapProjection(m31aDir, phases); err != nil {
		return count, coreerrors.Wrap(err, "write roadmap projection")
	}

	return count, nil
}

type PhaseData struct {
	ID             string   `json:"id"`
	Number         int      `json:"number"`
	Name           string   `json:"name"`
	Goal           string   `json:"goal"`
	Requirements   []string `json:"requirements"`
	SuccessCriteria []string `json:"success_criteria"`
	Status         string   `json:"status"`
}

func parseRoadmap(content string) []PhaseData {
	var phases []PhaseData
	lines := strings.Split(content, "\n")
	var currentPhase *PhaseData
	phaseNum := 0

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "## Phase ") || strings.HasPrefix(line, "### Phase ") {
			phaseNum++
			// Extract phase name
			name := strings.TrimSpace(strings.TrimPrefix(line, "## Phase "))
			name = strings.TrimSpace(strings.TrimPrefix(name, "### Phase "))
			if idx := strings.Index(name, ": "); idx != -1 {
				name = name[idx+2:]
			}
			currentPhase = &PhaseData{
				ID:               uuid.New().String(),
				Number:           phaseNum,
				Name:             name,
				Requirements:     []string{},
				SuccessCriteria:  []string{},
				Status:           "pending",
			}
			phases = append(phases, *currentPhase)
		} else if currentPhase != nil {
			if strings.HasPrefix(line, "- **Goal:**") || strings.HasPrefix(line, "**Goal:**") {
				currentPhase.Goal = strings.TrimSpace(strings.TrimPrefix(line, "- **Goal:**"))
				currentPhase.Goal = strings.TrimSpace(strings.TrimPrefix(currentPhase.Goal, "**Goal:**"))
			} else if strings.HasPrefix(line, "- **Requirements:**") || strings.HasPrefix(line, "**Requirements:**") {
				// Next lines will have requirements
			} else if strings.HasPrefix(line, "  - REQ-") || strings.HasPrefix(line, "- REQ-") {
				reqID := strings.TrimSpace(strings.TrimPrefix(line, "  - "))
				reqID = strings.TrimSpace(strings.TrimPrefix(reqID, "- "))
				currentPhase.Requirements = append(currentPhase.Requirements, reqID)
			} else if strings.HasPrefix(line, "- **Success Criteria:**") || strings.HasPrefix(line, "**Success Criteria:**") {
				// Next lines
			} else if strings.HasPrefix(line, "  - ") && strings.Contains(line, "criteria") {
				// Skip for now
			}
		}
	}
	return phases
}

func writeRoadmapProjection(m31aDir string, phases []PhaseData) error {
	var b strings.Builder
	b.WriteString("# Roadmap\n\n")
	b.WriteString(fmt.Sprintf("Total Phases: %d\n\n", len(phases)))

	for _, phase := range phases {
		b.WriteString(fmt.Sprintf("## Phase %d: %s\n\n", phase.Number, phase.Name))
		if phase.Goal != "" {
			b.WriteString(fmt.Sprintf("**Goal:** %s\n\n", phase.Goal))
		}
		if len(phase.Requirements) > 0 {
			b.WriteString("**Requirements:**\n")
			for _, req := range phase.Requirements {
				b.WriteString(fmt.Sprintf("- %s\n", req))
			}
			b.WriteString("\n")
		}
	}

	return types.AtomicWrite(filepath.Join(m31aDir, "roadmap.md"), []byte(b.String()))
}

func migrateProject(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) error {
	path := filepath.Join(planningDir, "PROJECT.md")
	content, err := os.ReadFile(path)
	if err != nil {
		return coreerrors.Wrapf(err, "read %s", path)
	}

	project := parseProject(string(content))

	evt := types.Event{
		Type:      types.EventProjectInitialized,
		Payload:   marshal(project),
		SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
	}
	if err := store.Append(ctx, evt); err != nil {
		return coreerrors.Wrap(err, "append project initialized event")
	}

	// Write projection
	return writeProjectProjection(m31aDir, project)
}

func parseProject(content string) *types.Project {
	// Simple parsing - extract key fields
	project := &types.Project{
		ID:        uuid.New(),
		Name:      "M31A",
		RootPath:  ".",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "# ") && project.Name == "M31A" {
			project.Name = strings.TrimPrefix(line, "# ")
		}
		// Could parse more fields
	}
	return project
}

func writeProjectProjection(m31aDir string, project *types.Project) error {
	data, err := json.MarshalIndent(project, "", "  ")
	if err != nil {
		return err
	}
	return types.AtomicWrite(filepath.Join(m31aDir, "project.md"), data)
}

func migrateState(ctx context.Context, store *SQLiteEventStore, planningDir string) error {
	path := filepath.Join(planningDir, "STATE.md")
	content, err := os.ReadFile(path)
	if err != nil {
		return coreerrors.Wrapf(err, "read %s", path)
	}

	// Emit StateSnapshot event
	evt := types.Event{
		Type:      types.EventConfigChanged, // Use as generic state snapshot
		Payload:   marshal(map[string]string{"content": string(content)}),
		SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
	}
	return store.Append(ctx, evt)
}

func migrateContext(ctx context.Context, store *SQLiteEventStore, planningDir string) error {
	path := filepath.Join(planningDir, "CONTEXT_M31A.md")
	content, err := os.ReadFile(path)
	if err != nil {
		return coreerrors.Wrapf(err, "read %s", path)
	}

	// Emit ContextCaptured event
	evt := types.Event{
		Type:      types.EventConfigChanged,
		Payload:   marshal(map[string]string{"content": string(content)}),
		SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
	}
	return store.Append(ctx, evt)
}

func migrateDecisions(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) (int, error) {
	decisionsDir := filepath.Join(planningDir, "decisions")
	entries, err := os.ReadDir(decisionsDir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, nil
		}
		return 0, coreerrors.Wrap(err, "read decisions dir")
	}

	count := 0
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}
		path := filepath.Join(decisionsDir, entry.Name())
		content, err := os.ReadFile(path)
		if err != nil {
			return count, coreerrors.Wrapf(err, "read decision %s", entry.Name())
		}

		decision := parseDecision(entry.Name(), string(content))
		if decision != nil {
			evt := types.Event{
				Type:      types.EventDecisionLogged,
				Payload:   marshal(decision),
				SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
			}
			if err := store.Append(ctx, evt); err != nil {
				return count, coreerrors.Wrapf(err, "append decision %s", entry.Name())
			}
			count++

			// Write projection
			if err := artifacts.WriteDecision(m31aDir, decision); err != nil {
				return count, coreerrors.Wrapf(err, "write decision projection %s", entry.Name())
			}
		}
	}
	return count, nil
}

func parseDecision(filename, content string) *types.Decision {
	// Parse decision markdown
	lines := strings.Split(content, "\n")
	var decision types.Decision
	var inRationale bool

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "# Decision: ") {
			decision.Title = strings.TrimPrefix(line, "# Decision: ")
			// ID from filename
			idStr := strings.TrimSuffix(filename, ".md")
			if idx := strings.Index(idStr, "-"); idx != -1 {
				idStr = idStr[:idx]
				if parsed, err := uuid.Parse(idStr); err == nil {
					decision.ID = parsed
				}
			}
		} else if strings.HasPrefix(line, "**Status:** ") {
			decision.Status = types.DecisionStatus(strings.TrimPrefix(line, "**Status:** "))
		} else if strings.HasPrefix(line, "**Timestamp:** ") {
			tsStr := strings.TrimPrefix(line, "**Timestamp:** ")
			if ts, err := time.Parse(time.RFC3339, tsStr); err == nil {
				decision.Timestamp = ts
			}
		} else if line == "## Rationale" {
			inRationale = true
		} else if strings.HasPrefix(line, "## Alternatives") {
			inRationale = false
		} else if inRationale && line != "" {
			decision.Rationale += line + "\n"
		}
	}

	if decision.Title == "" {
		return nil
	}
	return &decision
}

func migrateResearch(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) (int, error) {
	researchDir := filepath.Join(planningDir, "research")
	entries, err := os.ReadDir(researchDir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, nil
		}
		return 0, coreerrors.Wrap(err, "read research dir")
	}

	count := 0
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}
		path := filepath.Join(researchDir, entry.Name())
		content, err := os.ReadFile(path)
		if err != nil {
			return count, coreerrors.Wrapf(err, "read research %s", entry.Name())
		}

		research := parseResearch(entry.Name(), string(content))
		if research != nil {
			evt := types.Event{
				Type:      types.EventResearchCompleted,
				Payload:   marshal(research),
				SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
			}
			if err := store.Append(ctx, evt); err != nil {
				return count, coreerrors.Wrapf(err, "append research %s", entry.Name())
			}
			count++

			// Write projection
			if err := artifacts.WriteResearch(m31aDir, research); err != nil {
				return count, coreerrors.Wrapf(err, "write research projection %s", entry.Name())
			}
		}
	}
	return count, nil
}

func parseResearch(filename, content string) *types.Research {
	lines := strings.Split(content, "\n")
	var research types.Research
	var inFindings bool
	var findings strings.Builder

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "# Research: ") {
			research.Question = strings.TrimPrefix(line, "# Research: ")
			idStr := strings.TrimSuffix(filename, ".md")
			if idx := strings.Index(idStr, "-"); idx != -1 {
				idStr = idStr[:idx]
				if parsed, err := uuid.Parse(idStr); err == nil {
					research.ID = parsed
				}
			}
		} else if strings.HasPrefix(line, "**Confidence:** ") {
			confStr := strings.TrimPrefix(line, "**Confidence:** ")
			fmt.Sscanf(confStr, "%f", &research.Confidence)
		} else if strings.HasPrefix(line, "**Created:** ") {
			tsStr := strings.TrimPrefix(line, "**Created:** ")
			if ts, err := time.Parse(time.RFC3339, tsStr); err == nil {
				research.CreatedAt = ts
			}
		} else if strings.HasPrefix(line, "**Created By:** ") {
			idStr := strings.TrimPrefix(line, "**Created By:** ")
			if parsed, err := uuid.Parse(idStr); err == nil {
				research.CreatedBy = parsed
			}
		} else if line == "## Sources" {
			inFindings = false
		} else if strings.HasPrefix(line, "- ") && !inFindings {
			research.Sources = append(research.Sources, strings.TrimPrefix(line, "- "))
		} else if line == "## Findings" {
			inFindings = true
		} else if inFindings {
			findings.WriteString(line + "\n")
		}
	}

	research.Findings = strings.TrimSpace(findings.String())
	if research.Question == "" {
		return nil
	}
	return &research
}

func migrateCodebase(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) (int, error) {
	codebaseDir := filepath.Join(planningDir, "codebase")
	entries, err := os.ReadDir(codebaseDir)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, nil
		}
		return 0, coreerrors.Wrap(err, "read codebase dir")
	}

	// Ensure codebase directory exists in m31aDir
	if err := os.MkdirAll(filepath.Join(m31aDir, "codebase"), types.DirPermission); err != nil {
		return 0, coreerrors.Wrap(err, "create codebase dir")
	}

	count := 0
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}
		path := filepath.Join(codebaseDir, entry.Name())
		content, err := os.ReadFile(path)
		if err != nil {
			return count, coreerrors.Wrapf(err, "read codebase %s", entry.Name())
		}

		evt := types.Event{
			Type:      types.EventArtifactCreated,
			Payload:   marshal(map[string]string{"file": entry.Name(), "content": string(content)}),
			SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
		}
		if err := store.Append(ctx, evt); err != nil {
			return count, coreerrors.Wrapf(err, "append codebase %s", entry.Name())
		}
		count++

		// Write to .m31a/codebase/
		dstPath := filepath.Join(m31aDir, "codebase", entry.Name())
		if err := types.AtomicWrite(dstPath, []byte(content)); err != nil {
			return count, coreerrors.Wrapf(err, "write codebase projection %s", entry.Name())
		}
	}
	return count, nil
}

func migrateConfig(ctx context.Context, store *SQLiteEventStore, planningDir, m31aDir string) error {
	path := filepath.Join(planningDir, "config.json")
	content, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil // config.json is optional
		}
		return coreerrors.Wrapf(err, "read %s", path)
	}

	var config map[string]interface{}
	if err := json.Unmarshal(content, &config); err != nil {
		return coreerrors.Wrap(err, "unmarshal config.json")
	}

	// Write to .m31a/config.toml via config loader
	// For now, just emit event
	evt := types.Event{
		Type:      types.EventConfigChanged,
		Payload:   marshal(config),
		SessionID: uuidPtr(uuid.MustParse(migrationSessionID)),
	}
	return store.Append(ctx, evt)
}

func archivePlanning(planningDir string) error {
	timestamp := time.Now().Format("20060102-150405")
	archiveName := fmt.Sprintf(".planning.archived.%s", timestamp)
	archivePath := filepath.Join(filepath.Dir(planningDir), archiveName)

	// Use os.Rename for atomic move on same filesystem
	if err := os.Rename(planningDir, archivePath); err != nil {
		// If cross-device, fallback to copy + remove
		if err := copyDir(planningDir, archivePath); err != nil {
			return err
		}
		return os.RemoveAll(planningDir)
	}
	return nil
}

func copyDir(src, dst string) error {
	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		relPath, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		dstPath := filepath.Join(dst, relPath)
		if info.IsDir() {
			return os.MkdirAll(dstPath, info.Mode())
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		return os.WriteFile(dstPath, data, info.Mode())
	})
}

func uuidPtr(u uuid.UUID) *uuid.UUID {
	return &u
}