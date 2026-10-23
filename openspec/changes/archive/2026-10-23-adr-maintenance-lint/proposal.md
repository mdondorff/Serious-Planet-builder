# Change: adr-maintenance-lint

## Why
- ADR 0078 (owner-accepted, 2026-10-23) lets an accepted ADR change in four kinds (links, values, terms, erratum) and requires a trace line; the rule must be a check (Consequences).

## What changes
- `cargo xtask spec-lint` fails when a branch changes an accepted ADR beyond its status line and the ADR gains no `Maintained: <date>, <kind>: <what>` line (`adr_maintenance_errors`, `adr_changes_against_base`). BUILD-025.

## Tests
- Tier A (`xtask/src/speclint.rs`): status line only, body edit with and without a Maintained line, malformed lines, a proposed and a new ADR, no base.

Expected golden changes: none.
