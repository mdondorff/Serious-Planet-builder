## MODIFIED Requirements
### Requirement: BUILD-040 A change of a gate leaves a contract-change line
Spec-lint SHALL fail a branch that adds, removes or changes a row of the gate table under `### 12.1` of `docs/design/terrain-target.md`, against the merge base with `origin/main`, unless the same branch adds to that document's `## 14. Contract changes` list a line `- YYYY-MM-DD | gate or section | what changed | who decided` that names the changed row.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94; owner 2026-11-27 (D10: the terrain prototype briefing archived, its owner-decisions table no longer checked, its contract lines moved to the target document)

#### Scenario: A gate changes
- **WHEN** a branch changes the G-SURF row and adds no line, adds a line naming G-CON, adds a malformed line, or adds a dated line naming G-SURF
- **THEN** the first three are refused naming G-SURF and the last passes; a line already on the base covers no new change

## REMOVED Requirements
### Requirement: BUILD-042 An item that becomes done moves the re-estimate
**Reason**: the terrain prototype briefing it reads is archived (owner, 2026-11-27, D10).
**Migration**: none; a later milestone plan sets its own rules.
