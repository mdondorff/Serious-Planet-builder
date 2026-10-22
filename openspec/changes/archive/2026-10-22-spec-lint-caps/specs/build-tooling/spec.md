## ADDED Requirements

### Requirement: BUILD-023 The hot files stay within their caps
`cargo xtask spec-lint` SHALL fail when a file every session reads at its start is over its cap (ADR 0075 point 1): `docs/status.md` 15 KiB, `docs/learnings.md` 20 KiB, `docs/spikes.md` 10 KiB, `CLAUDE.md` 7,514 B and `docs/constitution.md` 8,574 B, with a line end counted as one byte, naming the file, its size and the cap. A cap changes only by a new ADR. A test that cites a requirement ID that no spec holds already fails the lint (TEST-001); this requirement adds the caps.

Verify: A
Status: active
Source: opus-planner (2026-10-21, M7 H1), ADR 0075

#### Scenario: Over and at the cap
- **WHEN** each hot file in turn is one byte over its cap, and when every file is at its cap
- **THEN** the first fails naming only that file with its size and cap, the second passes, a missing or uncapped file is no error, a CRLF line end counts as one byte, and the repository's own hot files are within their caps
