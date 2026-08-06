# Phase 5: Intelligence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-06
**Phase:** 05-intelligence
**Areas discussed:** Plan generation strategy, Context window management, Model routing intelligence, Verification confidence

---

## Plan generation strategy

### Task granularity
| Option | Description | Selected |
|--------|-------------|----------|
| Merge related operations | Combine related file edits, tool calls, and verification into fewer, larger tasks | ✓ |
| Complexity-based granularity | Keep tasks small (1-2 operations each) but add complexity scoring to decide granularity per goal | |
| Agent decides per goal | Let the LLM decide task boundaries based on the goal | |

**User's choice:** Merge related operations
**Notes:** Reduces plan length but each task does more.

---

### Failure handling
| Option | Description | Selected |
|--------|-------------|----------|
| Retry failed tasks only | Re-run the same plan on failure, skip completed tasks, retry failed ones | |
| Re-plan from failure point | On failure, re-plan from the failed task onward with new context | ✓ |
| Agent decides | Let the agent choose retry vs re-plan based on failure type | |

**User's choice:** Re-plan from failure point
**Notes:** More adaptive but slower.

---

### Plan format
| Option | Description | Selected |
|--------|-------------|----------|
| Keep markdown task lists | Plans are markdown files (already implemented) | ✓ |
| Richer task metadata | Add structured metadata: estimated time, complexity score, dependencies as DAG edges, risk level per task | |
| Markdown + JSON sidecar | Keep markdown but add a JSON sidecar with metadata for programmatic use | |

**User's choice:** Keep markdown task lists
**Notes:** Current format works well.

---

### Outcome recording
| Option | Description | Selected |
|--------|-------------|----------|
| Record outcomes for learning | After execution, record which tasks succeeded/failed, how long they took | ✓ |
| No outcome recording | Don't record outcomes — each plan is independent | |
| Agent decides | Let the agent decide what to record based on what seems useful | |

**User's choice:** Record outcomes for learning
**Notes:** Use this to improve future plan generation.

---

## Context window management

### Compaction strategy
| Option | Description | Selected |
|--------|-------------|----------|
| Recency + summary | Keep only the most recent messages plus a fixed-size summary of older context | ✓ |
| Relevance scoring | Score each message by relevance to current task, keep top-K by score | |
| Hybrid scoring | Use both recency and relevance as weights | |

**User's choice:** Recency + summary
**Notes:** Simple and predictable.

---

### Compaction timing
| Option | Description | Selected |
|--------|-------------|----------|
| Before phase transitions | Compact before each phase transition | ✓ |
| Threshold-based | Compact when context usage exceeds a threshold (e.g., 80%) | |
| Agent decides | Let the agent decide when to compact based on task complexity | |

**User's choice:** Before phase transitions
**Notes:** Proactive but may drop useful context prematurely.

---

### Summary format
| Option | Description | Selected |
|--------|-------------|----------|
| Single summary paragraph | Summarize all older messages into a single paragraph | |
| Topic-grouped summaries | Group messages by topic (tool calls, decisions, errors) and summarize each group separately | |
| Keep tool results, summarize rest | Keep full tool call results (they're actionable), summarize everything else | ✓ |

**User's choice:** Keep tool results, summarize rest
**Notes:** Tool results are actionable and should be preserved.

---

### Token counting
| Option | Description | Selected |
|--------|-------------|----------|
| Keep tiktoken | Current: tiktoken-go for token counting | |
| Provider-specific tokenizers | Use provider-specific tokenizers (cl100k for OpenAI, etc.) | ✓ |
| Agent decides | Let the agent choose based on provider capabilities | |

**User's choice:** Provider-specific tokenizers
**Notes:** More accurate counts per provider.

---

## Model routing intelligence

### Routing strategy
| Option | Description | Selected |
|--------|-------------|----------|
| Cost-first routing | Pick the cheapest model that can handle the task | |
| Quality-first routing | Pick the best model for the task, cost secondary | |
| Complexity-adaptive routing | Adapt routing based on task complexity: simple tasks use cheap models, complex tasks use expensive ones | ✓ |

**User's choice:** Complexity-adaptive routing
**Notes:** Balances cost and quality based on task needs.

---

### Fallback behavior
| Option | Description | Selected |
|--------|-------------|----------|
| Auto-fallback to next provider | If the primary model fails, try the next provider automatically | ✓ |
| Ask user on failure | If the primary model fails, ask the user which provider to try next | |
| Retry same, then fallback | If the primary model fails, retry with the same provider before falling back | |

**User's choice:** Auto-fallback to next provider
**Notes:** Seamless experience without user interruption.

---

### Complexity assessment
| Option | Description | Selected |
|--------|-------------|----------|
| Rule-based complexity scoring | Use fixed rules: simple = fast model, complex = powerful model | ✓ |
| History-based routing | Track task success rates per model, route to models with highest success rate | |
| Agent decides | Let the agent decide based on current context and model availability | |

**User's choice:** Rule-based complexity scoring
**Notes:** Simple and predictable.

---

### Cost optimization
| Option | Description | Selected |
|--------|-------------|----------|
| No cost limits | Use the best model available | |
| Per-session cost cap | Set a per-session cost cap. When exceeded, switch to cheaper models | ✓ |
| Per-task cost cap | Set a per-task cost cap | |
| Agent decides | Let the agent decide based on user preferences | |

**User's choice:** Per-session cost cap
**Notes:** Balances cost control with flexibility.

---

## Verification confidence

### Verification approach
| Option | Description | Selected |
|--------|-------------|----------|
| Automated checks only | Run go build, go test, go vet, lint. No confidence scoring | |
| Automated + confidence scoring | Automated checks + estimate success probability based on test results | ✓ |
| Adaptive verification depth | Quick check for simple tasks, deep review for complex tasks | |

**User's choice:** Automated + confidence scoring
**Notes:** More intelligent verification.

---

### Thoroughness
| Option | Description | Selected |
|--------|-------------|----------|
| Always run full verification | Run all verification steps regardless of change size | ✓ |
| Scale verification to change size | Skip lint/vet for small changes, run full suite for larger changes | |
| Agent decides | Let the agent decide which checks to run based on change type | |

**User's choice:** Always run full verification
**Notes:** Consistent and reliable.

---

### Failure handling
| Option | Description | Selected |
|--------|-------------|----------|
| Auto-heal and retry | If verification fails, automatically bisect to find the cause and retry | ✓ |
| Show failure, ask user | If verification fails, show the user what went wrong and ask for guidance | |
| Log and continue | If verification fails, log the failure and move on to the next task | |

**User's choice:** Auto-heal and retry
**Notes:** Uses existing bisect capability.

---

### Confidence threshold
| Option | Description | Selected |
|--------|-------------|----------|
| 100% task success rate | All tasks pass verification | |
| 90% task success rate | At least 90% of tasks pass | ✓ |
| Agent decides | Let the agent decide based on task criticality | |

**User's choice:** 90% task success rate
**Notes:** Tolerates minor failures while maintaining quality.

---

## the agent's Discretion

- Specific complexity scoring rules for model routing
- Confidence scoring formula based on verification results
- Auto-heal strategies based on failure type
- Outcome recording format for plan learning

## Deferred Ideas

None — discussion stayed within phase scope
