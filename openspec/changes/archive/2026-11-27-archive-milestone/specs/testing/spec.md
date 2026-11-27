## ADDED Requirements
### Requirement: TEST-159 The milestone archive command on a fixture docs tree
Tier-A tests SHALL run BUILD-072 on a fixture docs tree and check its table's steps. A dry run, a malformed fragment and a header without its pointer sentence SHALL leave every file of the tree unchanged.

Verify: A
Status: active
Source: Lead 2026-11-27, BUILD-072

#### Scenario: Refused before any write
- **WHEN** the fixture holds a malformed fragment, or `docs/log.md` has no sentence listing the archives
- **THEN** the command fails naming the file, and every file of the tree is byte for byte as before
