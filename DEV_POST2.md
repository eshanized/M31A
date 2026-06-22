---
title: "Building an AI Agent That Learns from Its Mistakes: The Ledger System"
description: "How M31A's cross-session learning ledger creates a feedback loop that makes your AI coding assistant sharper over time"
tags: [ai, go, developer-tools, machine-learning, terminal]
cover_image: ""
published: false
---

# Building an AI Agent That Learns from Its Mistakes: The Ledger System

Every time you start a new ChatGPT or Copilot session, it starts from zero. No memory of what worked yesterday, no idea which models performed best on your codebase, no record of which tasks failed and why.

**M31A's cross-session learning ledger changes that.** Every session writes a structured record to a markdown file. Over time, the agent queries this ledger to learn from past sessions — which frameworks are common, what types of tasks fail, and how long things typically take.

In this post, I'll show you how the ledger works, why markdown is the perfect storage format, and how this simple feedback loop creates compounding value.

## The Problem: AI Agents with Amnesia

Here's the typical experience with AI coding tools:

1. You spend 10 minutes explaining your project structure
2. The agent generates code that doesn't match your patterns
3. You correct it, it works
4. Next session? Same dance from scratch

The agent never learns that:
- You prefer functional React components over class components
- Your Go services use `chi` for routing, not `gorilla/mux`
- Tasks involving database migrations always need extra verification
- Claude handles your codebase better than GPT-4 for refactoring

**Every session is a fresh start.** The knowledge you build up exists only in your head.

## The Solution: A Structured Learning Record

M31A solves this with a **cross-session learning ledger** (`pkg/ledger/`). After every session, the agent writes a structured record:

```markdown
| Session    | Model              | Tasks | Failed | Cost  | Duration | Framework | Goal                    |
|------------|--------------------|-------|--------|-------|----------|-----------|-------------------------|
| a1b2c3d4   | claude-3.5-sonnet  | 5     | 1      | $0.12 | 8min     | react     | refactor auth middleware |
| e5f6g7h8   | gpt-4-turbo        | 3     | 0      | $0.08 | 4min     | go        | add rate limiting       |
| i9j0k1l2   | claude-3.5-sonnet  | 7     | 2      | $0.21 | 12min    | python    | implement celery tasks  |
```

This isn't just logging — it's a **feedback loop**. The agent can query this data to make better decisions in future sessions.

## What Gets Tracked

The ledger captures seven key dimensions per session:

```go
// From pkg/ledger/ledger.go
type LedgerEntry struct {
    SessionID   string
    Timestamp   time.Time
    Model       string
    Provider    string
    TaskCount   int
    FailedTasks int
    CostEstimate float64
    Duration    time.Duration
    ProjectType string
    Framework   string
    GoalKeywords []string
    FailureReasons []string
}
```

### Goal Keywords (with Stop-Word Filtering)

The agent extracts keywords from your goal description, filtering out common stop words:

```go
// From pkg/ledger/keywords.go
var stopWords = map[string]bool{
    "the": true, "a": true, "an": true, "is": true,
    "to": true, "for": true, "of": true, "with": true,
    // ... 100+ stop words
}

func ExtractKeywords(goal string) []string {
    words := strings.Fields(strings.ToLower(goal))
    var keywords []string
    for _, w := range words {
        w = strings.Trim(w, ".,;:!?")
        if !stopWords[w] && len(w) > 2 {
            keywords = append(keywords, w)
        }
    }
    return keywords
}
```

This means "Refactor the authentication middleware to use JWT" becomes `["refactor", "authentication", "middleware", "jwt"]` — the semantic core of your intent.

### Failure Tracking

When tasks fail, the ledger records why:

```go
type FailureReason string

const (
    FailureSyntaxError    FailureReason = "syntax_error"
    FailureTestFailure    FailureReason = "test_failure"
    FailureTimeout        FailureReason = "timeout"
    FailureDependency     FailureReason = "dependency_error"
    FailureLLMRefusal     FailureReason = "llm_refusal"
)
```

Over time, you can see patterns: "40% of my Python failures are test failures" or "Claude refuses to modify security-critical code."

## Querying the Ledger

The real power comes from querying historical data. M31A exposes ledger statistics:

```go
// From pkg/ledger/ledger.go
type LedgerStats struct {
    TotalSessions      int
    AvgTaskCount       float64
    AvgCost            float64
    AvgDurationMinutes float64
    TotalFailedTasks   int
    TopFailures        []string
    TopFrameworks       []string
    ByProjectType      map[string]int
    ModelPerformance   map[string]ModelStats
}

type ModelStats struct {
    Sessions    int
    AvgCost     float64
    AvgDuration float64
    FailureRate float64
}
```

You can run `/ledger stats` in the TUI to see:

```
📊 Ledger Statistics (12 sessions)

Models Used:
  claude-3.5-sonnet  8 sessions  $0.14 avg  2.3% failure rate
  gpt-4-turbo        4 sessions  $0.09 avg  5.1% failure rate

Top Frameworks:
  react      5 sessions
  go         4 sessions
  python     3 sessions

Average per Session:
  Tasks: 4.2  |  Cost: $0.11  |  Duration: 6.8min
  Failed Tasks: 0.3 (7.1% failure rate)
```

**The insight**: Claude is more expensive but has half the failure rate on this codebase. For critical tasks, use Claude. For quick refactors, GPT-4 is fine.

## The Learning Loop in Action

Here's a real scenario showing how the ledger creates value over time:

### Session 1: First Contact

```
Goal: "Add rate limiting to the API gateway"
Model: gpt-4-turbo
Tasks: 3
Failed: 1 (test_failure)
Cost: $0.08
Duration: 5min
Framework: go
```

The ledger records that GPT-4 failed a test on a Go project.

### Session 2: The Agent Adapts

```
Goal: "Implement request validation middleware"
Model: claude-3.5-sonnet  ← Agent chose Claude based on Session 1 failure
Tasks: 4
Failed: 0
Cost: $0.15
Duration: 7min
Framework: go
```

The agent learned: "GPT-4 had test failures on Go projects. Try Claude instead."

### Session 3: Pattern Recognition

After 10 sessions, the ledger shows:

```
Go Projects:
  gpt-4-turbo:   3 sessions, 2 failures (test failures)
  claude-3.5:     7 sessions, 0 failures
  
Recommendation: Use Claude for Go projects
```

**This isn't hard-coded logic.** It's emergent behavior from structured data collection.

## Why Markdown?

I chose markdown tables over a database for several reasons:

### 1. Human Readability

You can open `ledger.md` in any editor and understand the data:

```markdown
| Session | Model | Tasks | Failed | Cost | Duration |
|---------|-------|-------|--------|------|----------|
| a1b2c3  | claude | 5    | 1      | $0.12| 8min     |
```

No SQL knowledge required. No special tools. Just text.

### 2. Git-Friendly

The ledger lives in `.m31a/ledger.md` and gets committed with your project:

```bash
$ git log --oneline -- .m31a/ledger.md
a1b2c3d Session: refactor auth middleware (claude-3.5, 5 tasks, $0.12)
e5f6g7h Session: add rate limiting (gpt-4, 3 tasks, $0.08)
```

You can see how your AI usage evolves over the project's lifetime.

### 3. Diffable

When something changes, you see exactly what:

```diff
-| a1b2c3 | claude-3.5-sonnet | 5 | 1 | $0.12 | 8min | react |
+| a1b2c3 | claude-3.5-sonnet | 5 | 0 | $0.12 | 7min | react |
```

The failure count dropped. The agent is learning.

### 4. Zero Dependencies

No SQLite, no PostgreSQL, no ORMs. Just `bufio.Scanner` and string splitting:

```go
// From pkg/ledger/parser.go
func ParseLedgerLine(line string) (*LedgerEntry, error) {
    fields := strings.Split(line, "|")
    if len(fields) < 8 {
        return nil, ErrInvalidFormat
    }
    
    return &LedgerEntry{
        SessionID:  strings.TrimSpace(fields[1]),
        Model:      strings.TrimSpace(fields[2]),
        TaskCount:  parseInt(fields[3]),
        FailedTasks: parseInt(fields[4]),
        // ...
    }, nil
}
```

## Advanced: Model Performance Tracking

The ledger tracks per-model statistics to answer: "Which model should I use for this task?"

```go
// From pkg/ledger/stats.go
func (l *Ledger) ModelStats() map[string]ModelStats {
    stats := make(map[string]ModelStats)
    
    for _, entry := range l.entries {
        s := stats[entry.Model]
        s.Sessions++
        s.TotalCost += entry.CostEstimate
        s.TotalDuration += entry.Duration
        if entry.FailedTasks > 0 {
            s.TotalFailures++
        }
        stats[entry.Model] = s
    }
    
    // Calculate averages
    for model, s := range stats {
        s.AvgCost = s.TotalCost / float64(s.Sessions)
        s.AvgDuration = s.TotalDuration / time.Duration(s.Sessions)
        s.FailureRate = float64(s.TotalFailures) / float64(s.Sessions)
        stats[model] = s
    }
    
    return stats
}
```

The agent uses this data during the **model selection phase**:

```
User goal: "Refactor the payment processing module"

Agent analysis:
  - Task involves financial code (high complexity)
  - Past sessions show Claude has 0% failure rate on payment code
  - GPT-4 failed 2/3 times on similar tasks
  
Recommendation: claude-3.5-sonnet (higher cost, lower risk)
```

## Implementation: The Write Path

Writing ledger entries is simple but robust:

```go
// From pkg/ledger/writer.go
func (l *Ledger) WriteEntry(entry LedgerEntry) error {
    l.mu.Lock()
    defer l.mu.Unlock()
    
    // Open file in append mode
    f, err := os.OpenFile(l.path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
    if err != nil {
        return fmt.Errorf("opening ledger: %w", err)
    }
    defer f.Close()
    
    // Format as markdown table row
    line := fmt.Sprintf("| %s | %s | %d | %d | $%.2f | %s | %s | %s |\n",
        entry.SessionID,
        entry.Model,
        entry.TaskCount,
        entry.FailedTasks,
        entry.CostEstimate,
        entry.Duration.Round(time.Second),
        entry.Framework,
        entry.GoalSummary(),
    )
    
    if _, err := f.WriteString(line); err != nil {
        return fmt.Errorf("writing entry: %w", err)
    }
    
    return nil
}
```

Key design decisions:
- **Append-only**: Never modify past entries. History is immutable.
- **Atomic writes**: Open with `O_APPEND|O_CREATE` to avoid race conditions.
- **No locking at file level**: Use a mutex in memory for concurrent access.

## Privacy Considerations

The ledger stores **metadata**, not code:

- ✅ Model name, task count, cost, duration
- ✅ Framework type (react, go, python)
- ✅ Goal keywords (filtered, not full text)
- ✅ Failure categories (not stack traces)
- ❌ Actual code changes
- ❌ Full goal descriptions
- ❌ API keys or tokens

Your code never touches the ledger. Only operational metadata is recorded.

## The Compounding Effect

After 50 sessions, the ledger becomes a **knowledge base**:

```
📊 Your M31A Usage Patterns

Most Productive Model:
  claude-3.5-sonnet for Go projects (0% failure, $0.14 avg)
  gpt-4-turbo for Python scripts (3% failure, $0.08 avg)

Task Complexity Distribution:
  trivial: 23% (avg 2min, $0.03)
  simple:  41% (avg 5min, $0.08)
  moderate: 28% (avg 10min, $0.18)
  complex:  8% (avg 25min, $0.42)

Common Failure Patterns:
  - Database migrations: 12% failure rate (always verify)
  - React hooks: 8% failure rate (test thoroughly)
  - Security middleware: 0% failure rate (Claude excels)
```

**This data is yours.** It lives in your repo, gets committed with your code, and helps the agent serve you better over time.

## Getting Started

The ledger is automatic. Every M31A session writes to it:

```bash
# Start a session
$ m31a

# After the session completes, check your ledger
$ cat .m31a/ledger.md

# Or use the built-in stats command
$ m31a /ledger stats
```

The ledger file is created on first session and appended to on every subsequent session.

## What's Next

The ledger is just the beginning. Planned enhancements:

1. **Goal similarity matching**: Find past sessions with similar goals to inform current decisions
2. **Cost optimization alerts**: Warn when a session is exceeding historical averages
3. **Model recommendation engine**: Suggest models based on task type and historical performance
4. **Export to analytics**: Pipe ledger data to external dashboards

## Lessons Learned

Building the ledger taught me three things:

1. **Simple data beats complex systems.** A markdown table is easier to debug, version, and share than a SQLite database.

2. **Metadata is underrated.** You don't need to store code changes to learn from them. Task counts, failure rates, and durations tell you most of what you need.

3. **Learning compounds.** The first 10 sessions are noise. After 50, patterns emerge. After 100, the agent is predictably better at serving your specific codebase.

## Conclusion

The cross-session learning ledger is M31A's secret weapon. It turns every session into training data, creating a feedback loop that makes the agent sharper over time. No cloud sync, no analytics servers, no privacy concerns — just a markdown file that gets smarter as you use it.

If you're building AI tools, consider adding structured logging from day one. The insights compound faster than you think.

**Try it out:**

```bash
# Install M31A
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash

# Run a few sessions
$ m31a

# Check your ledger after a few sessions
$ m31a /ledger stats
```

**Links:**
- GitHub: [github.com/eshanized/M31A](https://github.com/eshanized/M31A)
- Documentation: [docs/](https://github.com/eshanized/M31A/tree/master/docs)
- Previous post: [Building M31A: A Terminal-Native AI Coding Agent](https://dev.to/eshanized/building-m31a)

---

*What patterns would you track in your AI coding sessions? I'd love to hear what metadata would be most valuable to you. Open an issue or reach out on Twitter.*
