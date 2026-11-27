## ADDED Requirements
### Requirement: TEST-162 The PR-body review lines on fixture bodies and paths
Tier-A tests SHALL check BUILD-075's table row by row on fixture bodies, commit lists and changed paths: each line missing, naming no commit of the PR, and present; a docs-only PR; a record, a promoting and a rejecting research package, a deleted and an archived record; the `Sprint:` line with no, the wrong and the active sprint; and the active sprint found only in the sprint form.

Verify: A
Status: active
Source: Lead 2026-11-27 (the combined sprint process plan's item 6), BUILD-075

#### Scenario: A record PR
- **WHEN** a docs-only PR adds `docs/milestones/S1/E1-record.md` with no review lines, then with `Research-Review: READY <sha>` of a commit of the PR and `Owner-Look: yes (<date>, <sheet>)`
- **THEN** the first gives two problems naming the record, and the second none
