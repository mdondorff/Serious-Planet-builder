## ADDED Requirements
### Requirement: BUILD-040 A change of a gate or an owner decision leaves a contract-change line
Spec-lint SHALL fail a branch that adds, removes or changes a row of the gate table under `### 12.1` of `docs/design/terrain-target.md` or of the table under `## Owner decisions` of `docs/briefings/terrain-prototype.md`, against the merge base with `origin/main`, unless the same branch adds to the briefing's `## Contract changes` list a line `- YYYY-MM-DD | gate or section | what changed | who decided` whose second field contains the row's first cell (without a trailing parenthesis). A malformed line SHALL cover nothing. A missing briefing with a changed row SHALL fail.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94

#### Scenario: A gate changes
- **WHEN** a branch changes the G-SURF row and adds no line, adds a line naming G-CON, adds a malformed line, or adds a dated line naming G-SURF
- **THEN** the first three are refused naming G-SURF and the last passes; a line already on the base covers no new change
