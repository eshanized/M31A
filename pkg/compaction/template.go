package compaction

const summaryTemplate = `You are a conversation compactor. Summarize the conversation history below into a structured summary that preserves all critical context for continuing the session.

Produce a summary with EXACTLY these sections:

## Goal
What the user is trying to accomplish.

## Constraints
Any limitations, requirements, or rules established during the conversation.

## Progress
### Done
What has been completed so far.

### In Progress
What is currently being worked on.

### Blocked
Any blockers or unresolved issues.

## Key Decisions
Important decisions made during the conversation and their rationale.

## Next Steps
Concrete actions to take next.

## Critical Context
Any essential information that would be lost without this summary (file contents, error messages, configurations, etc.).

## Relevant Files
List of files that were discussed, created, or modified.

Guidelines:
- Be thorough but concise. Target under 2000 tokens.
- Preserve specific file paths, function names, error messages, and code snippets.
- Do NOT include tool call details or raw tool outputs.
- Focus on WHAT and WHY, not HOW (the recent messages already have the HOW).
`

// Template returns the compaction summary prompt template.
func Template() string {
	return summaryTemplate
}
