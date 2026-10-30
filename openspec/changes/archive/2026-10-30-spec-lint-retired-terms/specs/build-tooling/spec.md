## ADDED Requirements

### Requirement: BUILD-028 Retired terms stay out of the current specs and docs
`cargo xtask spec-lint` SHALL read the terms of the first column of the "Former terms" table in section 8 of `docs/domain.md` and fail a mention, as a whole phrase in any case with `-` and `_` read as a space, in the main specs, `domain.md` above section 8, `status.md`, `roadmap.md`, `README.md` and `docs/briefings/`, naming file, line, term and the row's "Now" column. A row marked *code removal pending* SHALL be reported as a count of warnings. A single-word term SHALL be skipped unless the row says `Lint: all`; `Lint: a; b` adds spellings and `Lint: none` skips the row.

Verify: A
Status: active
Source: owner (2026-10-30), opus-planner, docs/domain.md section 8

#### Scenario: A planted term
- **WHEN** a checked file holds a retired term whose row is not pending
- **THEN** spec-lint fails naming the file, line, term and the "Now" text, and a mention in an ADR, an archive or the log is not read

#### Scenario: A pending term
- **WHEN** a checked file holds a term whose row says code removal pending
- **THEN** spec-lint passes and prints the count of such mentions
