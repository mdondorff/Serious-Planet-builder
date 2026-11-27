## ADDED Requirements
### Requirement: TEST-164 The sprint files' archive move on a fixture docs tree
Tier-A tests SHALL run BUILD-077 on a fixture docs tree: a closed sprint with a report and records, a sprint that is not closed, the old milestone form, and a move onto an existing file. A refused run SHALL leave every file of the tree unchanged.

Verify: A
Status: active
Source: Lead 2026-11-27 (the combined sprint process plan's item 7), BUILD-077

#### Scenario: A sprint not closed, then closed
- **WHEN** the fixture's `P3.md` says `Status: active, closing`, then `Status: closed`, and later `docs/archive/P3/P3-report.md` already exists
- **THEN** the first run keeps both files and says why, the second plans both moves, and the third is refused naming both paths with no file changed
