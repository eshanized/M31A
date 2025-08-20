# Audit Report Index — Deep Logical Errors & Hardcoded Values

**Created:** 2026-06-05  
**Purpose:** Cross-reference of all audit reports in `rush/` related to logical errors and hardcoded values

---

## Related Reports

| Report | Focus | Overlap with This Report |
|--------|-------|-------------------------|
| `tui_logical_errors_and_connectivity_report.md` | TUI screen connectivity | Supplements with 10 additional bugs |
| `tui_core_wiring_report.md` | Core wiring issues | Overlaps on config wiring (Part 4.2) |
| `deep_codebase_audit_2026.md` | General codebase audit | Overlaps on hardcoded values (Part 1) |
| `comprehensive_deep_audit_2026.md` | Comprehensive audit | Overlaps on TUI issues (Part 3) |
| `tui_ui_audit_report.md` | UI audit | Overlaps on TUI screen issues (Part 3) |
| `internal_wiring_and_logic_report.md` | Internal wiring | Overlaps on config wiring (Part 4) |

---

## Key Findings from Previous Reports (Confirmed & Supplemented)

1. **Hardcoded model capability maps** — First identified in `deep_codebase_audit_2026.md`, confirmed in this report with exact line numbers
2. **Config fields not wired to providers** — First identified in `tui_core_wiring_report.md`, expanded with full list of 8 unwired fields
3. **Self-heal not working** — First identified in `tui_logical_errors_and_connectivity_report.md`, confirmed with code analysis
4. **Settings UX issues** — First identified in `tui_ui_audit_report.md`, expanded with unsaved changes warning

---

## New Findings Not in Previous Reports

1. **BUG-01**: First-run validation hardcoded URLs (NEW)
2. **BUG-03**: Ship phase excluded from workflowRunning (NEW)
3. **BUG-04**: Theme "auto" not handled in runtime switch (NEW)
4. **BUG-06**: New session redirects to first-run wizard (NEW)
5. **Health check threshold mismatch** between config and providers (NEW)
6. **WebFetch User-Agent hardcoded** instead of using Version variable (NEW)
7. **Config bool merge logic** overwrites explicit false values (NEW)

---

## Files Created

- `deep_logical_errors_and_hardcoded_values_report.md` — Main report (this document supplements it)
