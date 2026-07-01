# Narrative Engine Architecture — M31A v1.6.2 Milestone 2A.5

## 1. Event Classification Matrix

Every event in M31A classified: **Narrative** (user sees human language), **Hidden** (internal only), **Grouped** (batched), **Expanded** (always shown with detail).

### 1.1 Summary

| Category | Count | Percentage |
|----------|-------|-----------|
| **Narrative** (user sees human language) | 38 | 31% |
| **Hidden** (internal only) | 55 | 45% |
| **Grouped** (batched with similar events) | 22 | 18% |
| **Expanded** (always shown with detail) | 2 | 2% |
| **Dead code** (never emitted) | 2 | 2% |

### 1.2 Workflow Engine Events (32 types)

| # | Event | Category | Narrative Treatment | Justification |
|---|-------|----------|-------------------|---------------|
| 1 | PhaseTransitionStartMsg | **Narrative** | "Understanding project structure" | Answers "what is M31A doing?" |
| 2 | PhaseTransitionCompleteMsg | **Narrative** | "Project structure understood" | Completion feedback |
| 3 | ThinkingStartMsg | **Hidden** | — | Internal drain trigger only |
| 4 | ThinkingCompleteMsg | **Hidden** | — | Internal drain trigger only |
| 5 | IntermediateProgressMsg | **Grouped** | Merged into parent narrative | Prevents conversation flooding |
| 6 | ResearchProgressMsg | **Narrative** | "Researching authentication patterns" | User needs to know research is happening |
| 7 | TaskStartMsg | **Narrative** | "Implementing user authentication" | Answers "what is M31A doing?" |
| 8 | TaskUpdateMsg | **Narrative** | "Authentication implementation complete" | Completion feedback |
| 9 | TaskDiffSummaryMsg | **Grouped** | Appended to task completion narrative | Detail, not top-level |
| 10 | ToolStartMsg | **Grouped** | Merged into parent task narrative | Individual tools are noise |
| 11 | ToolCompleteMsg | **Grouped** | Merged into parent task narrative | Individual tools are noise |
| 12 | SelfHealStartMsg | **Narrative** | "Recovering from test failure" | User needs to know recovery is happening |
| 13 | SelfHealCompleteMsg | **Narrative** | "Recovery successful" / "Recovery failed" | Completion feedback |
| 14 | InitAnalysisMsg | **Narrative** | "Analyzed Go project (42 files)" | Orientation information |
| 15 | InitPreflightMsg | **Narrative** | "Preflight checks passed" | Orientation information |
| 16 | DiscussQualityMsg | **Hidden** | — | Internal quality gate |
| 17 | DiscussCompletenessMsg | **Hidden** | — | Internal quality gate |
| 18 | StreamChunkMsg (discuss) | **Grouped** | Merged into discuss streaming | Progressive rendering, not narrative |
| 19 | PlanCheckMsg | **Hidden** | — | Internal quality gate |
| 20 | PlanRevisionMsg | **Narrative** | "Refining plan (iteration 2/3)" | User needs to know plan is being refined |
| 21 | PlanChunkProgressMsg | **Narrative** | "Planning wave 2/3 (8 tasks)" | Progress feedback |
| 22 | PlanProgressMsg | **Hidden** | — | Dead code, never emitted |
| 23 | ExecutePreflightMsg | **Narrative** | "Validating dependencies" | Pre-execution orientation |
| 24 | ExecuteQualityGateMsg | **Hidden** | — | Internal quality gate |
| 25 | ExecuteLoopDetectMsg | **Narrative** | "Detected repeated operation, adjusting approach" | Anomaly worth reporting |
| 26 | VerifyReportMsg | **Narrative** | "Verification: 85% pass rate" | Completion feedback |
| 27 | RuntimeCheckCompleteMsg | **Narrative** | "Runtime check complete" | Completion feedback |
| 28 | ShipPreflightMsg | **Narrative** | "Preparing final commit" | Pre-ship orientation |
| 29 | ShipChangelogMsg | **Grouped** | Appended to ship narrative | Detail, not top-level |
| 30 | DemonstrationReadyMsg | **Narrative** | "Changes ready for review" | Completion feedback |
| 31 | CompactionCompleteMsg | **Narrative** | "Context compressed (saved 12K tokens)" | Transparent context management |
| 32 | AgentSwitchMsg | **Narrative** | "Switching to planning agent" | Agent transitions are orientation |

### 1.3 Streaming Pipeline Events (14 types)

| # | Event | Category | Narrative Treatment | Justification |
|---|-------|----------|-------------------|---------------|
| 33 | StreamMsg | **Grouped** | Progressive rendering | Raw streaming, not narrative |
| 34 | StreamDoneMsg | **Narrative** | Conversation message appears | Final output |
| 35 | StreamErrorMsg | **Narrative** | "Provider rate limited, switching..." | Error is user-facing |
| 36 | TickMsg | **Hidden** | — | Internal render timer |
| 37 | AgentStreamMsg | **Grouped** | Progressive rendering | Raw streaming |
| 38 | AgentToolStartMsg | **Grouped** | Merged into agent activity narrative | Individual tools are noise |
| 39 | AgentToolDoneMsg | **Grouped** | Merged into agent activity narrative | Individual tools are noise |
| 40 | AgentToolProgressMsg | **Grouped** | Merged into agent activity narrative | Status update |
| 41 | AgentIterationDoneMsg | **Grouped** | Appended to agent narrative | Progress marker |
| 42 | AgentIterationMsg | **Grouped** | Agent iteration summary | Progress marker |
| 43 | AgentDoneMsg | **Narrative** | Agent activity complete | Completion feedback |
| 44 | AgentErrorMsg | **Narrative** | "Agent encountered an error" | Error is user-facing |
| 45 | AgentCompressedMsg | **Narrative** | "Auto-compressed: 8 messages removed" | Transparent context management |
| 46 | AgentThinkingMsg | **Hidden** | — | Internal status |

### 1.4 Background Listener Events (8 types)

| # | Event | Category | Narrative Treatment | Justification |
|---|-------|----------|-------------------|---------------|
| 47 | PermissionRequestMsg | **Expanded** | Permission modal always shown | Safety: user must explicitly approve |
| 48 | QuestionRequestMsg | **Expanded** | Question modal always shown | User must answer |
| 49 | HealthCheckTickMsg | **Hidden** | — | Internal timer |
| 50 | HealthCheckResultMsg | **Hidden** | — | Diagnostic, not orientation |
| 51 | RefreshCacheMsg | **Hidden** | — | Internal timer |
| 52 | CacheRefreshResultMsg | **Hidden** | — | Internal cache management |
| 53 | SidebarRefreshTickMsg | **Hidden** | — | Internal timer |
| 54 | ConfigReloadMsg | **Narrative** | "Configuration reloaded" | User should know config changed |

### 1.5 TUI-Internal Events (59 types)

| # | Event | Category | Treatment |
|---|-------|----------|-----------|
| 55-60 | AppMsg, PopScreenMsg, SlashCommandMsg, HomeSubmitMsg, GoalSubmittedMsg, PhaseModelPickedMsg | **Hidden** | User-initiated navigation |
| 61 | ModelSelectedMsg | **Narrative** | "Switched to Claude 3.5 Sonnet" |
| 62 | IntentClassifiedMsg | **Hidden** | Internal routing |
| 63 | PhaseResultMsg | **Narrative** | Phase completion summary |
| 64 | PlanReadyMsg | **Narrative** | "Plan ready: 6 tasks, ~15 min" |
| 65-67 | PlanApproveMsg, PlanRefineMsg, ExecutePauseMsg | **Hidden** | User-initiated |
| 68 | HealResultMsg | **Narrative** | "Healing attempt succeeded/failed" |
| 69-72 | PermissionResponseMsg, PermissionTickMsg, QuestionResponseMsg, QuestionResponse | **Hidden** | Internal routing |
| 73-74 | DiscussAnswerMsg, DiscussCompleteMsg | **Hidden** | User-initiated |
| 75 | DiscussAnswerTimeoutMsg | **Narrative** | "Question timed out, proceeding" |
| 76 | ToastMsg | **Narrative** | Transient notification |
| 77-78 | ToastExpiryMsg, DismissToastMsg | **Hidden** | Internal cleanup |
| 79-81 | SettingsSavedMsg, ResetCompleteMsg, ConfigSavedMsg | **Narrative** | Confirmation |
| 82 | FallbackEventMsg | **Narrative** | "Switched from OpenRouter to Zen" |
| 83 | OptimizedMsg | **Narrative** | "Recommended model change" |
| 84 | sessionRestoredMsg | **Narrative** | "Session restored" |
| 85-99 | resumeScreenReadyMsg through ToolClickMsg | **Hidden** | Screen population / user-initiated |
| 100 | FirstRunCompleteMsg | **Narrative** | "Setup complete" |
| 101-102 | firstRunModelsMsg, firstRunKeyValidationMsg | **Hidden** | Screen population |
| 103 | ErrorMsg | **Narrative** | Error banner in conversation |
| 104 | SubagentEventMsg | **Grouped** | Merged into parent agent narrative |
| 105-113 | metricsLoadedMsg through ThemeChangedMsg | **Hidden** | Internal / deprecated |

### 1.6 Subagent Events (8 types)

| # | Event | Category | Treatment |
|---|-------|----------|-----------|
| 114 | EventSpawned | **Narrative** | "Spawning explore agent" |
| 115-116 | EventToolStart, EventToolDone | **Grouped** | Merged into parent agent narrative |
| 117 | EventTextDelta | **Grouped** | Progressive rendering |
| 118 | EventThinking | **Hidden** | Internal status |
| 119 | EventDone | **Narrative** | "Explore agent completed" |
| 120 | EventError | **Narrative** | "Explore agent encountered an error" |
| 121 | EventCancelled | **Narrative** | "Agent cancelled" |

---

## 2. Narrative Taxonomy

### 2.1 Categories

Every narrative belongs to exactly one category. Categories map to sidebar phases.

| Category | Description | Sidebar Phase | Visual Treatment |
|----------|-------------|--------------|-----------------|
| **Orienting** | Where am I? What is this project? | Initialize | Muted text |
| **Understanding** | Reading and analyzing code | Initialize | Muted text |
| **Discussing** | Asking and answering questions | Discuss | Muted text |
| **Planning** | Breaking down the work | Plan | Brand text |
| **Researching** | Gathering external information | Plan | Muted text |
| **Executing** | Making changes to the codebase | Execute | Brand text, bold |
| **Verifying** | Running tests and validation | Verify | Brand text |
| **Recovering** | Self-healing after failures | Execute/Verify | Warning text |
| **Shipping** | Committing and finalizing | Ship | Success text |
| **Learning** | Compacting context, summarizing | Any | Muted text |
| **Alerting** | Errors, warnings, provider issues | Any | Error/Warning text |

### 2.2 Narrative Types by Category

#### Orienting

| Type | Template | Example |
|------|----------|---------|
| ProjectAnalyzed | "Analyzed {lang} project ({n} files)" | "Analyzed Go project (42 files)" |
| PreflightPassed | "Preflight checks passed" | "Preflight checks passed" |
| PreflightFailed | "Preflight: {n} issues found" | "Preflight: 3 issues found" |
| SessionRestored | "Session restored ({n} messages)" | "Session restored (24 messages)" |

#### Understanding

| Type | Template | Example |
|------|----------|---------|
| ReadingCode | "Reading {file}" | "Reading auth/middleware.go" |
| ReadingProject | "Reading project structure" | "Reading project structure" |
| AnalyzingCode | "Analyzing {scope}" | "Analyzing authentication flow" |
| MappingCodebase | "Mapping codebase dependencies" | "Mapping codebase dependencies" |

#### Discussing

| Type | Template | Example |
|------|----------|---------|
| AskingQuestion | "Preparing questions" | "Preparing questions" |
| WaitingForAnswer | "Waiting for your input" | "Waiting for your input" |
| QuestionTimedOut | "Question timed out, proceeding" | "Question timed out, proceeding" |

#### Planning

| Type | Template | Example |
|------|----------|---------|
| PlanningWork | "Planning implementation" | "Planning implementation" |
| PlanWave | "Planning wave {current}/{total}" | "Planning wave 2/3" |
| PlanRefining | "Refining plan (iteration {n}/{max})" | "Refining plan (iteration 2/3)" |
| PlanReady | "Plan ready: {n} tasks" | "Plan ready: 6 tasks" |

#### Researching

| Type | Template | Example |
|------|----------|---------|
| ResearchingTopic | "Researching {topic}" | "Researching JWT validation patterns" |
| FetchingUrl | "Reading {url}" | "Reading docs.example.com/auth" |
| SearchingWeb | "Searching for {query}" | "Searching for Go middleware patterns" |

#### Executing

| Type | Template | Example |
|------|----------|---------|
| StartingTask | "Implementing {description}" | "Implementing user authentication" |
| TaskComplete | "{description} complete" | "User authentication complete" |
| TaskFailed | "{description} failed" | "User authentication failed" |
| WritingFile | "Writing {file}" | "Writing auth/middleware.go" |
| EditingFile | "Editing {file}" | "Editing auth/handler.go" |
| RunningCommand | "Running {command}" | "Running go test ./..." |
| ReadingFile | "Reading {file}" | "Reading config.yaml" |
| SearchingCode | "Searching for {pattern}" | "Searching for auth middleware" |
| FindingFiles | "Finding {pattern}" | "Finding test files" |

#### Verifying

| Type | Template | Example |
|------|----------|---------|
| RunningTests | "Running verification" | "Running verification" |
| TestsPassed | "Verification passed ({n} tests)" | "Verification passed (42 tests)" |
| TestsFailed | "Verification: {n}/{m} passed" | "Verification: 38/42 passed" |
| BuildFailed | "Build failed" | "Build failed" |

#### Recovering

| Type | Template | Example |
|------|----------|---------|
| SelfHealing | "Recovering from {error}" | "Recovering from test failure" |
| HealingAttempt | "Healing attempt {n}/{max}" | "Healing attempt 2/3" |
| HealingSucceeded | "Recovery successful" | "Recovery successful" |
| HealingFailed | "Recovery failed" | "Recovery failed" |

#### Shipping

| Type | Template | Example |
|------|----------|---------|
| PreparingCommit | "Preparing final commit" | "Preparing final commit" |
| ChangelogReady | "Changelog: {n} entries" | "Changelog: 3 entries" |
| ChangesReady | "Changes ready for review" | "Changes ready for review" |
| Committed | "Committed: {message}" | "Committed: Add auth middleware" |

#### Learning

| Type | Template | Example |
|------|----------|---------|
| ContextCompressed | "Context compressed (saved {n} tokens)" | "Context compressed (saved 12K tokens)" |
| MessagesRemoved | "Auto-compressed: {n} messages removed" | "Auto-compressed: 8 messages removed" |

#### Alerting

| Type | Template | Example |
|------|----------|---------|
| ProviderError | "Provider error: {error}" | "Provider error: rate limited" |
| ProviderSwitch | "Switched from {from} to {to}" | "Switched from OpenRouter to Zen" |
| ConfigReloaded | "Configuration reloaded" | "Configuration reloaded" |
| LoopDetected | "Detected repeated operation, adjusting" | "Detected repeated operation, adjusting" |
| ModelSwitched | "Switched to {model}" | "Switched to Claude 3.5 Sonnet" |

---

## 3. Workflow State Mapping

### 3.1 State to Narrative to Sidebar to Conversation

| Workflow State | Sidebar Display | Conversation Message | Duration |
|---------------|----------------|---------------------|----------|
| **Initialize** | "Initializing" | "Analyzing Go project (42 files)" | Until complete |
| **Discuss** | "Discussing" | "Preparing questions" → "Waiting for your input" | Until answers submitted |
| **Plan** | "Planning" | "Planning implementation" → "Plan ready: 6 tasks" | Until plan approved |
| **Execute** | "Executing" | "Implementing {task}" per task | Per-task |
| **Verify** | "Verifying" | "Running verification" → "Verification passed" | Until complete |
| **Runtime** | "Checking" | "Runtime check complete" | Until complete |
| **Ship** | "Shipping" | "Preparing final commit" → "Changes ready" | Until complete |
| **Idle** | Branch + cost | (none) | Until next action |

### 3.2 State Transition Rules

```
Idle → Initialize:  "Analyzing project..."
Initialize → Discuss:  "Preparing questions..."
Initialize → Plan:  "Planning implementation..."  (if no questions needed)
Initialize → Execute:  "Implementing..."  (if simple enough)
Discuss → Plan:  "Planning implementation..."
Plan → Execute:  "Implementing {task}..."  (per task, sequential)
Execute → Verify:  "Running verification..."
Verify → Execute:  "Recovering from failure..."  (if self-heal triggered)
Verify → Ship:  "Preparing final commit..."
Verify → Runtime:  "Runtime check complete..."
Runtime → Ship:  "Preparing final commit..."
Ship → Idle:  (sidebar returns to branch + cost)
Any → Idle:  (on cancellation or completion)
```

### 3.3 Active vs Idle Sidebar

**Idle sidebar** shows only:
- Branch name (always)
- Cost (if > 0)

**Active sidebar** adds:
- Current phase indicator
- In-progress task (max 2)
- Pending approvals (if any)
- Cost

The transition from Idle to Active happens when workflow state != idle.
The transition from Active to Idle happens when workflow state == idle.

---

## 4. Event Grouping Rules

### 4.1 Principle

Repeated tool calls must never flood the conversation. The narrative engine groups related events into a single human-readable description.

### 4.2 Grouping Patterns

#### Pattern A: Tool Sequence → Single Narrative

```
BEFORE:                          AFTER:
FileRead auth.go                 Reading authentication code
FileRead provider.go             • auth.go
FileRead middleware.go            • provider.go
FileRead handler.go               • middleware.go
                                 • handler.go
```

**Trigger:** 3+ FileRead/FileRead events within 2 seconds.
**Output:** Single "Reading {scope}" narrative with file list.

#### Pattern B: Search Sequence → Single Narrative

```
BEFORE:                          AFTER:
Grep "auth"                      Searching for authentication patterns
Glob "**/*.go"                   Found 12 matching files
Grep "middleware"
```

**Trigger:** 2+ search events (Grep/Glob) within 3 seconds.
**Output:** Single "Searching for {pattern}" narrative.

#### Pattern C: Write Sequence → Single Narrative

```
BEFORE:                          AFTER:
FileWrite auth.go                Writing authentication module
Edit auth.go                     • auth.go (created)
Edit auth.go                     • middleware.go (modified)
```

**Trigger:** 2+ file write/edit events within 5 seconds.
**Output:** Single "Writing {module}" narrative with file count.

#### Pattern D: Verify Sequence → Single Narrative

```
BEFORE:                          AFTER:
Bash "go build ./..."            Running verification
Bash "go test ./..."
Bash "go vet ./..."
```

**Trigger:** 2+ build/test commands within 10 seconds.
**Output:** Single "Running verification" narrative.

#### Pattern E: Subagent Activity → Collapsed

```
BEFORE:                          AFTER:
AgentToolStart (explore)         Exploring codebase...
AgentToolDone (explore)
AgentToolStart (explore)         (collapsed by default)
AgentToolDone (explore)
EventDone (explore)
```

**Trigger:** Subagent events from same agent.
**Output:** Single "Exploring codebase..." narrative, collapsed.

### 4.3 Anti-Patterns (Never Do This)

1. **Never show individual tool names.** "FileRead" is noise. "Reading auth/middleware.go" is signal.
2. **Never show raw event counts.** "4 tool calls" is meaningless. "Reading authentication code" is meaningful.
3. **Never show internal package names.** "internal/tools/fileread.go" means nothing to users.
4. **Never repeat the same narrative.** If "Reading auth.go" already appeared, don't show it again.

### 4.4 Grouping Window

| Event Type | Window | Max Items Shown |
|-----------|--------|----------------|
| File reads | 2 seconds | 5 files, then "and N more" |
| Searches | 3 seconds | Pattern + result count |
| File writes | 5 seconds | 3 files, then "and N more" |
| Build/test | 10 seconds | Command summary |
| Subagent tools | Entire agent lifecycle | Agent name + status |

---

## 5. Narrative Style Guide

### 5.1 Writing Rules

1. **Present tense.** "Reading file" not "Read file" or "Will read file".
2. **Active voice.** "Analyzing code" not "Code is being analyzed".
3. **Maximum 6 words per activity title.** "Implementing user authentication" (4 words) ✓. "Reading the contents of the authentication middleware file" (10 words) ✗.
4. **Never expose tool names.** "Reading file" not "FileRead". "Searching code" not "Grep".
5. **Never expose internal packages.** "Reading auth code" not "Reading internal/tools/auth.go".
6. **Never expose implementation details.** "Implementing authentication" not "Editing internal/auth/middleware.go lines 45-67".
7. **Never exaggerate.** "Reading code" not "Deep-diving into the codebase".
8. **Never use marketing language.** "Analyzing project" not "Intelligently analyzing your project".
9. **Always explain intent.** "Implementing user authentication" explains what M31A is trying to accomplish.
10. **Prefer nouns over verbs for completed actions.** "Authentication complete" not "Finished implementing authentication".

### 5.2 Word Choices

| Instead of | Use |
|-----------|-----|
| FileRead | Reading |
| FileWrite | Writing |
| Edit | Editing |
| Grep/Glob | Searching / Finding |
| Bash | Running |
| ToolCall | (never expose) |
| TaskRunner | (never expose) |
| PhaseTransition | (never expose) |
| StreamChunk | (never expose) |
| CompactionComplete | Context compressed |

### 5.3 Verb Map

| Tool | Narrative Verb | Template |
|------|---------------|----------|
| FileRead | Reading | "Reading {file}" |
| FileWrite | Writing | "Writing {file}" |
| Edit | Editing | "Editing {file}" |
| Glob | Finding | "Finding {pattern}" |
| Grep | Searching | "Searching for {query}" |
| Bash (test) | Running tests | "Running verification" |
| Bash (build) | Building | "Building project" |
| Bash (other) | Running | "Running {description}" |
| WebFetch | Reading | "Reading {url}" |
| WebSearch | Searching | "Searching for {query}" |
| CodeMap | Mapping | "Mapping code dependencies" |
| TodoWrite | (internal) | (never narrative) |
| Agent | Spawning | "Spawning {type} agent" |

### 5.4 File Name Rendering

When a file path must appear in a narrative:

1. **Strip common prefixes.** "internal/auth/middleware.go" → "auth/middleware.go"
2. **Strip src/ prefix.** "src/components/App.tsx" → "components/App.tsx"
3. **Show relative paths.** Never show absolute paths.
4. **Truncate long names.** "auth/middleware.go" ✓. "internal/handlers/v2/api/auth/middleware.go" → "handlers/v2/auth/middleware.go"
5. **Never show line numbers.** "Editing auth/handler.go" not "Editing auth/handler.go:45-67"

---

## 6. Transparency Rules

### 6.1 What Must Always Be Visible

| Event | Reason | Display Method |
|-------|--------|---------------|
| Permission request | Safety: user must approve | Modal overlay |
| Question request | User must answer | Modal overlay |
| File deletion | Destructive action | Expanded narrative + permission |
| Dangerous Bash | Risk to system | Permission modal |
| Provider error | May affect user | Error narrative |
| Provider switch | User should know | Info narrative |
| Self-heal start | Recovery is happening | Warning narrative |
| Loop detection | Anomaly | Warning narrative |
| Context compression | Context management | Info narrative |

### 6.2 What Must Always Be Collapsed

| Event | Reason | Display Method |
|-------|--------|---------------|
| Successful FileRead | Routine, frequent | Grouped into parent narrative |
| Successful Grep/Glob | Routine, frequent | Grouped into parent narrative |
| Successful Bash (non-dangerous) | Routine | Grouped into parent narrative |
| Subagent tool calls | Detail, not orientation | Collapsed under agent name |
| Streaming chunks | Rendering, not narrative | Progressive text rendering |
| Tick messages | Internal | Hidden |
| Health checks | Diagnostic | Hidden |
| Cache refreshes | Internal | Hidden |

### 6.3 What Expands on Error

| Event | Normal Treatment | Error Treatment |
|-------|-----------------|----------------|
| FileRead | Collapsed | Expanded: "Failed to read auth.go: permission denied" |
| FileWrite | Collapsed | Expanded: "Failed to write auth.go: disk full" |
| Bash (test) | Collapsed | Expanded: "Build failed: undefined reference to..." |
| Subagent | Collapsed | Expanded: "Explore agent error: context overflow" |
| Provider | Hidden | Expanded: "Provider error: rate limited, switching to Zen" |

### 6.4 Permission Expansion Rules

| Risk Level | Treatment |
|-----------|-----------|
| Low | Auto-approve (if configured), no narrative |
| Medium | Permission modal, brief narrative |
| High | Permission modal, detailed narrative with file/command |
| Dangerous | Permission modal, full expansion, cannot auto-approve |

---

## 7. Timing Rules

### 7.1 Minimum Display Duration

Narratives must remain visible for a minimum time to prevent flickering.

| Narrative Type | Minimum Duration | Rationale |
|---------------|-----------------|-----------|
| Phase transition | 500ms | User needs to register the change |
| Task start | 300ms | User needs to see what's happening |
| Task complete | 2000ms | User needs to read the result |
| Error | 5000ms | User needs to read and understand |
| Self-heal | 1000ms | User needs to register recovery |
| Info (toast) | 3000ms | Standard notification duration |
| Warning (toast) | 5000ms | User needs to read warning |
| Success (toast) | 2000ms | Confirmation is quick to read |

### 7.2 Merge Thresholds

| Event Sequence | Merge Window | Result |
|---------------|-------------|--------|
| 3+ FileRead events | 2 seconds | Single "Reading {scope}" narrative |
| 2+ search events | 3 seconds | Single "Searching for {pattern}" |
| 2+ file writes | 5 seconds | Single "Writing {module}" |
| 2+ build commands | 10 seconds | Single "Running verification" |
| TaskStart → ToolStart → ToolComplete → TaskComplete | Entire task | Single "{description} complete" |

### 7.3 Replacement Rules

| Current Narrative | New Event | Action |
|------------------|-----------|--------|
| "Reading auth.go" | FileRead(auth.go) done | Replace with "Reading auth.go ✓" for 500ms, then next narrative |
| "Implementing task 1" | TaskComplete | Replace with "Task 1 complete" for 2s, then next task |
| "Running verification" | TestsPassed | Replace with "Verification passed (42 tests)" for 2s |
| "Recovering from failure" | HealingSucceeded | Replace with "Recovery successful" for 1s |

### 7.4 Collapse Timing

| Section | Auto-Collapse After | Trigger |
|---------|-------------------|---------|
| Tool call list | 5 seconds after last tool | Timer |
| File read list | 3 seconds after last read | Timer |
| Agent activity | 10 seconds after agent done | AgentDone event |
| Progress bar | Immediately on completion | TaskComplete event |

### 7.5 Flickering Prevention

1. **Debounce rapid events.** If 5 events arrive in 100ms, batch them and render once.
2. **Don't update narrative on every streaming chunk.** Only update on meaningful state changes.
3. **Stable text during thinking.** If LLM is thinking, show "Thinking..." and don't change until response arrives.
4. **Minimum display time.** Every narrative must be visible for its minimum duration before replacement.

---

## 8. API Design

### 8.1 Core Interfaces

```
NarrativeEngine
  ├── Receives: RawEvent (workflow/tool/streaming/subagent)
  ├── Processes: Classification → Grouping → Template Selection → Timing
  └── Emits: NarrativeObject

NarrativeObject
  ├── Category: Orienting | Understanding | Planning | Executing | ...
  ├── Text: "Implementing user authentication"
  ├── Priority: P1-P7
  ├── Display: Sidebar | Conversation | Both
  ├── Duration: minimum display time
  ├── Collapsible: bool
  └── Timestamp: time.Time

NarrativeRenderer
  ├── Receives: NarrativeObject
  ├── Renders: Sidebar line | Conversation message | Toast
  └── Manages: Timing, replacement, collapse
```

### 8.2 Event Processing Pipeline

```
RawEvent
  → Classifier (is it Narrative? Hidden? Grouped? Expanded?)
  → Grouper (should it batch with recent events?)
  → TemplateResolver (which template to use?)
  → ParameterExtractor (extract {file}, {task}, etc.)
  → NarrativeBuilder (create NarrativeObject)
  → TimingGuard (check minimum duration, debounce)
  → Renderer (display in sidebar/conversation/toast)
```

### 8.3 Classification Interface

```
Classifier interface:
  Classify(event RawEvent) → Category:
    - Hidden: return nil
    - Narrative: return NarrativeType
    - Grouped: return GroupKey (for batching)
    - Expanded: return NarrativeType + Expanded=true
```

### 8.4 Grouping Interface

```
Grouper interface:
  ShouldGroup(event RawEvent, recentEvents []RawEvent) → bool:
    - Check time window
    - Check event type compatibility
    - Check group size limit
  
  FlushGroup(groupKey GroupKey) → NarrativeObject:
    - Assemble grouped narrative
    - Include file list / count
    - Apply template
```

### 8.5 Template Interface

```
TemplateResolver interface:
  Resolve(narrativeType NarrativeType, event RawEvent) → Template:
    - Select template from taxonomy
    - Validate parameters available
    - Return template string

ParameterExtractor interface:
  Extract(event RawEvent, template Template) → Parameters:
    - Map event fields to template placeholders
    - Truncate long values
    - Sanitize file paths
```

### 8.6 Renderer Interface

```
SidebarRenderer interface:
  Render(narrative NarrativeObject, width int) → string:
    - Apply visual treatment (color, bold)
    - Truncate to fit width
    - Handle empty state

ConversationRenderer interface:
  Render(narrative NarrativeObject) → Message:
    - Create assistant message
    - Apply markdown formatting
    - Handle replacement of previous narrative

ToastRenderer interface:
  Render(narrative NarrativeObject) → Toast:
    - Create transient notification
    - Set duration
    - Set type (info/success/warning/error)
```

### 8.7 State Machine Interface

```
NarrativeStateMachine interface:
  CurrentPhase() → Phase
  Transition(event RawEvent) → Phase:
    - Validate transition
    - Generate phase-change narrative
    - Update sidebar mode
  
  OnTaskStart(task) → NarrativeObject
  OnTaskComplete(task) → NarrativeObject
  OnSelfHeal(attempt) → NarrativeObject
  OnPhaseComplete(phase) → NarrativeObject
```

---

## 9. State Machine

### 9.1 States

```
┌─────────┐
│  IDLE   │◄──────────────────────────────────┐
└────┬────┘                                   │
     │ user submits goal                      │
     ▼                                        │
┌─────────┐                                   │
│INITIALZING│                                 │
└────┬────┘                                   │
     │ project analyzed                       │
     ▼                                        │
┌─────────┐     no questions                  │
│ DISCUSS ├──────────────────┐                │
└────┬────┘                  │                │
     │ answers submitted     │                │
     ▼                       ▼                │
┌─────────┐            ┌─────────┐            │
│  PLAN   │◄───────────┤  PLAN   │            │
└────┬────┘  revise    └─────────┘            │
     │ plan approved                          │
     ▼                                        │
┌─────────┐                                   │
│ EXECUTE │──┐                                │
└────┬────┘  │ self-heal                      │
     │       │                                │
     ▼       ▼                                │
┌─────────┐  ┌─────────┐                      │
│ VERIFY  │◄─┤  HEAL   │                      │
└────┬────┘  └─────────┘                      │
     │ verification passed                    │
     ▼                                        │
┌─────────┐                                   │
│  SHIP   │──┐                                │
└────┬────┘  │ runtime check                  │
     │       ▼                                │
     │  ┌─────────┐                           │
     │  │ RUNTIME │───────────────────────────┤
     │  └─────────┘                           │
     │                                        │
     └────────────────────────────────────────┘
```

### 9.2 Transition Narratives

| From | To | Sidebar | Conversation |
|------|----|---------|--------------|
| Idle | Initialize | "Initializing" | "Analyzing {lang} project..." |
| Initialize | Discuss | "Discussing" | "Preparing questions..." |
| Initialize | Plan | "Planning" | "Planning implementation..." |
| Discuss | Plan | "Planning" | "Planning implementation..." |
| Plan | Execute | "Executing" | "Implementing {task}..." |
| Execute | Verify | "Verifying" | "Running verification..." |
| Verify | Execute | "Executing" | "Recovering from failure..." |
| Verify | Ship | "Shipping" | "Preparing final commit..." |
| Verify | Runtime | "Checking" | "Runtime check complete" |
| Runtime | Ship | "Shipping" | "Preparing final commit..." |
| Ship | Idle | (branch + cost) | "Changes ready for review" |
| Any | Idle | (branch + cost) | (none) |

### 9.3 Error Transitions

| From | Error | To | Narrative |
|------|-------|----|-----------|
| Execute | Tool failure | Execute (self-heal) | "Recovering from {error}" |
| Verify | Test failure | Execute (self-heal) | "Recovering from test failure" |
| Verify | Build failure | Execute (self-heal) | "Recovering from build failure" |
| Any | Provider error | (same) | "Provider error: {error}" |
| Any | Context overflow | (same) | "Context compressed" |
| Any | User cancel | Idle | (none) |

---

## 10. Engineering Migration Plan

### 10.1 Phase 1: Narrative Engine Core (M3B)

| Task | Description | Files | Risk |
|------|------------|-------|------|
| 1 | Create narrative/ package with Classifier, Grouper, TemplateResolver | narrative/classifier.go, grouper.go, templates.go | Low |
| 2 | Create NarrativeObject type | narrative/types.go | Low |
| 3 | Implement template engine with all narrative types | narrative/templates.go | Low |
| 4 | Implement event classifier (Hidden/Narrative/Grouped/Expanded) | narrative/classifier.go | Low |
| 5 | Implement event grouper (time-window batching) | narrative/grouper.go | Medium |
| 6 | Implement timing guard (minimum duration, debounce) | narrative/timing.go | Medium |
| 7 | Write unit tests for all components | narrative/*_test.go | Low |

### 10.2 Phase 2: Integration (M3C)

| Task | Description | Files | Risk |
|------|------------|-------|------|
| 8 | Wire NarrativeEngine into workflow engine | workflow/engine.go | Medium |
| 9 | Replace ToolStartMsg/ToolCompleteMsg with narrative output | workflow/execute.go | Medium |
| 10 | Replace PhaseTransitionMsg with narrative output | workflow/engine.go | Low |
| 11 | Replace TaskStartMsg/TaskUpdateMsg with narrative output | workflow/execute.go | Low |
| 12 | Wire narrative to sidebar display | tui/sidebar_model.go | Low |
| 13 | Wire narrative to conversation display | tui/repl_model.go | Medium |
| 14 | Wire narrative to toast display | tui/app_handlers.go | Low |

### 10.3 Phase 3: Cleanup (M3D)

| Task | Description | Files | Risk |
|------|------------|-------|------|
| 15 | Remove dead code (PlanProgressMsg, ThemeChangedMsg) | workflow/engine_messages.go, tuitypes.go | Low |
| 16 | Remove unused sidebar rendering methods | tui/sidebar_model.go | Low |
| 17 | Update regression tests | tui/*_test.go | Low |
| 18 | Remove token/cost display from sidebar (moved to header/footer) | tui/sidebar_model.go | Low |

### 10.4 Dependencies

- M1 (Layout Foundation) — complete
- M2A (Sidebar IA) — complete
- M2A.5 (Narrative Engine Architecture) — this document
- M2B (Sidebar Implementation) — depends on M2A.5 for narrative types
- M3B (Narrative Engine Core) — implements this design
- M3C (Integration) — wires engine into existing code
- M3D (Cleanup) — removes deprecated code

### 10.5 Rollback Strategy

If the narrative engine causes issues:
1. Disable narrative engine via feature flag
2. Restore original event messages (ToolStartMsg, etc.)
3. Restore original sidebar rendering methods
4. All M1/M2A tests still pass

### 10.6 Testing Strategy

| Test Category | Count | Coverage |
|--------------|-------|----------|
| Classifier unit tests | 12 | Every event type classified correctly |
| Grouper unit tests | 8 | Time windows, group sizes, flush behavior |
| Template unit tests | 30 | Every narrative type renders correctly |
| Timing unit tests | 6 | Minimum durations, debounce, merge windows |
| Integration tests | 10 | End-to-end event → narrative → display |
| Regression tests | 15 | Existing behavior preserved |

**Total: 81 tests**

---

## Appendix A: Complete Narrative Type Reference

| Category | Type | Template | Example |
|----------|------|----------|---------|
| Orienting | ProjectAnalyzed | "Analyzed {lang} project ({n} files)" | "Analyzed Go project (42 files)" |
| Orienting | PreflightPassed | "Preflight checks passed" | "Preflight checks passed" |
| Orienting | PreflightFailed | "Preflight: {n} issues found" | "Preflight: 3 issues found" |
| Orienting | SessionRestored | "Session restored ({n} messages)" | "Session restored (24 messages)" |
| Understanding | ReadingCode | "Reading {file}" | "Reading auth/middleware.go" |
| Understanding | ReadingProject | "Reading project structure" | "Reading project structure" |
| Understanding | AnalyzingCode | "Analyzing {scope}" | "Analyzing authentication flow" |
| Understanding | MappingCodebase | "Mapping codebase dependencies" | "Mapping codebase dependencies" |
| Discussing | AskingQuestion | "Preparing questions" | "Preparing questions" |
| Discussing | WaitingForAnswer | "Waiting for your input" | "Waiting for your input" |
| Discussing | QuestionTimedOut | "Question timed out, proceeding" | "Question timed out, proceeding" |
| Planning | PlanningWork | "Planning implementation" | "Planning implementation" |
| Planning | PlanWave | "Planning wave {current}/{total}" | "Planning wave 2/3" |
| Planning | PlanRefining | "Refining plan (iteration {n}/{max})" | "Refining plan (iteration 2/3)" |
| Planning | PlanReady | "Plan ready: {n} tasks" | "Plan ready: 6 tasks" |
| Researching | ResearchingTopic | "Researching {topic}" | "Researching JWT patterns" |
| Researching | FetchingUrl | "Reading {url}" | "Reading docs.example.com" |
| Researching | SearchingWeb | "Searching for {query}" | "Searching for Go patterns" |
| Executing | StartingTask | "Implementing {description}" | "Implementing authentication" |
| Executing | TaskComplete | "{description} complete" | "Authentication complete" |
| Executing | TaskFailed | "{description} failed" | "Authentication failed" |
| Executing | WritingFile | "Writing {file}" | "Writing auth/middleware.go" |
| Executing | EditingFile | "Editing {file}" | "Editing auth/handler.go" |
| Executing | RunningCommand | "Running {command}" | "Running go test" |
| Executing | ReadingFile | "Reading {file}" | "Reading config.yaml" |
| Executing | SearchingCode | "Searching for {pattern}" | "Searching for auth" |
| Executing | FindingFiles | "Finding {pattern}" | "Finding test files" |
| Verifying | RunningTests | "Running verification" | "Running verification" |
| Verifying | TestsPassed | "Verification passed ({n} tests)" | "Verification passed (42 tests)" |
| Verifying | TestsFailed | "Verification: {n}/{m} passed" | "Verification: 38/42 passed" |
| Verifying | BuildFailed | "Build failed" | "Build failed" |
| Recovering | SelfHealing | "Recovering from {error}" | "Recovering from test failure" |
| Recovering | HealingAttempt | "Healing attempt {n}/{max}" | "Healing attempt 2/3" |
| Recovering | HealingSucceeded | "Recovery successful" | "Recovery successful" |
| Recovering | HealingFailed | "Recovery failed" | "Recovery failed" |
| Shipping | PreparingCommit | "Preparing final commit" | "Preparing final commit" |
| Shipping | ChangelogReady | "Changelog: {n} entries" | "Changelog: 3 entries" |
| Shipping | ChangesReady | "Changes ready for review" | "Changes ready for review" |
| Shipping | Committed | "Committed: {message}" | "Committed: Add auth" |
| Learning | ContextCompressed | "Context compressed (saved {n} tokens)" | "Context compressed (saved 12K tokens)" |
| Learning | MessagesRemoved | "Auto-compressed: {n} messages removed" | "Auto-compressed: 8 messages removed" |
| Alerting | ProviderError | "Provider error: {error}" | "Provider error: rate limited" |
| Alerting | ProviderSwitch | "Switched from {from} to {to}" | "Switched from OpenRouter to Zen" |
| Alerting | ConfigReloaded | "Configuration reloaded" | "Configuration reloaded" |
| Alerting | LoopDetected | "Detected repeated operation, adjusting" | "Detected repeated operation, adjusting" |
| Alerting | ModelSwitched | "Switched to {model}" | "Switched to Claude 3.5 Sonnet" |
