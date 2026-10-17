## ADDED Requirements

### Requirement: BUILD-010 Log entries are fragments
A new log entry SHALL be a file `docs/log.d/<YYYY-MM-DD>-<id>.md` holding exactly one entry line, `- YYYY-MM-DD **title** ...`; `<id>` is lower-case letters, digits, `-`, `_` or `.`, starting with a letter or digit. `cargo xtask spec-lint` SHALL fail on a fragment with a bad name, with other than one non-blank line, with a line that is not an entry, or with an entry date that differs from the file-name date, naming the file and the reason. `cargo xtask log [--since YYYY-MM-DD] [--limit N]` SHALL print the fragments and the entries of `docs/log.md` merged: date descending, then fragments before `log.md` lines of the same date, fragments by file name descending, `log.md` lines in file order. The output SHALL be deterministic.

Verify: A
Status: active
Source: housekeeping skill (root causes), PR #222 incident

#### Scenario: Fragment accepted
- **WHEN** `docs/log.d/2026-10-17-x.md` holds one line `- 2026-10-17 **x** ...`
- **THEN** spec-lint passes it

#### Scenario: Malformed fragment
- **WHEN** a fragment holds two entries, or its entry date differs from its file-name date, or its name is not `YYYY-MM-DD-<id>.md`
- **THEN** spec-lint fails and names the file and the reason

#### Scenario: Merged output
- **WHEN** fragments and `docs/log.md` entries of several dates exist
- **THEN** the output is newest first and identical for any read order

#### Scenario: One day, several fragments
- **WHEN** three fragments carry the same date
- **THEN** they print by file name descending, before the `log.md` lines of that date
