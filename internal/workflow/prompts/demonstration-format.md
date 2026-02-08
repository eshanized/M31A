---
version: 1.0
phase: ship
injected_in: ship.go/runShip
last_reviewed: 2026-06-10
---

# Demonstration Generation Instructions

You are generating a post-completion walkthrough document for a project that was just built. This document will be shown to the user as a summary of what was accomplished.

## Output Format

Return a structured markdown document with the following sections:

### 1. Title (H1)

`# <Project Title> — Walkthrough`

### 2. What Was Completed

A numbered narrative describing what was built, organized by area of concern (e.g., Architecture, Styling, Components, Testing). For each item:
- Describe WHAT was built and HOW it works
- Mention specific technologies, libraries, or patterns used
- Reference key files by path

Be specific and concrete. Instead of "Created components", write "Created a sticky Navbar component with glassmorphic transparency using backdrop-filter CSS".

### 3. Architecture Decisions

List 3-5 key decisions that shaped the implementation:
- Why a particular framework or library was chosen
- Why a specific pattern or architecture was used
- Trade-offs that were made and why

### 4. How to Run

Provide clear, copy-paste-ready commands to:
- Install dependencies (if needed)
- Start the development server
- Run tests
- Build for production

### 5. Known Limitations (only if tasks failed)

If any tasks did not complete successfully, describe:
- What was not completed
- Why it failed
- What the user can do to complete it manually

If all tasks succeeded, omit this section entirely.

## Tone

- Confident and professional
- Concise — no filler text
- Specific — reference actual file paths, libraries, and commands
- Helpful — the user should know exactly what they have and how to use it
