## MODIFIED Requirements

### Requirement: BUILD-003 Tiered test commands
`cargo xtask test fast` SHALL run tier A tests for the changed crates and every crate above them, `cargo xtask test` SHALL run tiers A and B, and `cargo xtask test gpu` SHALL run tier C; tiers SHALL be selected by test-binary name (`gpu_*` is tier C, `oracle_*` tier B, all others tier A) on the software adapter unless `--real` is given. `cargo xtask test gpu --real` SHALL also run the tier-B oracles, on the real adapter.

Verify: A
Status: active
Source: ADR 0009, report §16

#### Scenario: Change in a middle crate
- **WHEN** only files of `streaming` changed
- **THEN** `test fast` selects `streaming`, `frame`, `render`, `editor` and `app`

#### Scenario: Shared crate changed
- **WHEN** `testkit`, `perf` or a root manifest changed, or nothing changed
- **THEN** `test fast` selects every crate
