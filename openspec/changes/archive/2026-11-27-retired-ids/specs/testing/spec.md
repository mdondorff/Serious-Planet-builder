## ADDED Requirements
### Requirement: TEST-160 The retired IDs are checked and never issued again
Tier-A tests SHALL check BUILD-073: next-id on a fixture with an ID only in the file and one only in the change archive, each of spec-lint's three failures and its passes, and the repository's own file against its specs and archive.

Verify: A
Status: active
Source: Lead 2026-11-27, BUILD-073

#### Scenario: An archived requirement missing from the file
- **WHEN** the change archive removes VIEW-073, no main spec holds it, and the file does not list it
- **THEN** spec-lint fails naming VIEW-073 and the archive's file
