## ADDED Requirements
### Requirement: BUILD-073 Retired requirement IDs live in one committed file, independent of the change archive
`openspec/retired-ids.txt` SHALL list every requirement ID that is in no main spec any more, one a line, sorted by prefix and number (`#` lines are comments). `cargo xtask next-id` SHALL count it and SHALL NOT read `openspec/changes/archive/`, and spec-lint SHALL check it.

Verify: A
Status: active
Source: Lead 2026-11-27 (the P3 closure plan: D3's precondition, the change archive deletable without an ID issued again), BUILD-062
Detail:
| spec-lint fails on | |
|---|---|
| the file's form | a line that is not `PREFIX-NNN`, an ID twice, an ID out of order; a missing file |
| reuse | a requirement of a main spec, or one an open change adds (`## ADDED`), under an ID of the file |
| completeness | a requirement of the change archive (any delta section) in no main spec and not in the file |
- The file was generated from the change archive on 2026-11-27: 55 IDs.
- A change archived with a REMOVED requirement adds its ID to the file in the same PR (spec-lint names it otherwise).
- Leaving the archive out changes no requirement prefix's next ID: the archive's only other tokens are prose.

#### Scenario: An ID only in the file
- **WHEN** a main spec holds VIEW-072, the file VIEW-080, and the change archive VIEW-090
- **THEN** next-id gives VIEW-081, the archive's VIEW-090 does not count, and without the file it gives VIEW-073
