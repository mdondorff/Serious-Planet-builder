## ADDED Requirements

### Requirement: BUILD-013 The owner's bless is applied by a command, on every adapter
`cargo xtask bless` SHALL list the candidate goldens of every adapter: those of `target/review/candidates/<adapter>/<name>.png` and of each `--extra <dir>` (`<dir>/<adapter>/<name>.png`, a later source replacing an earlier one), leaving out a candidate byte-equal to its golden. `cargo xtask bless --apply <name>[,<name>]... --approved-by <who>` SHALL promote the candidates of exactly the named goldens on every adapter that has one to `tests/goldens/<adapter>/<name>.png`, and remove the names from `tests/goldens/pending.txt`. `--apply` without a non-blank `--approved-by`, an unknown argument, and `--approved-by` without `--apply` SHALL be errors. A name without a candidate, or a candidate whose `driver.txt` (beside it) differs from the recorded `tests/goldens/<adapter>/driver.txt` (or has none recorded), SHALL be an error naming it, and then nothing is written. `driver.txt` is never copied. The command does not decide what is blessed: the `bless` skill (ADR 0017) calls it only for what the owner approved.

Verify: A
Status: active
Source: opus-planner (2026-10-18), ADR 0017, TEST-022

#### Scenario: Three adapters from three sources
- **WHEN** the RTX candidate is in `target/review/candidates/`, the WARP candidate in one `--extra` directory and the llvmpipe candidate in another, and the owner approved the golden
- **THEN** `bless --extra ... --extra ... --apply v --approved-by owner` writes the three goldens, leaves goldens not named alone and removes `v` from `pending.txt`

#### Scenario: Refusals write nothing
- **WHEN** one of three candidates comes from another driver than the recorded one, or a name has no candidate
- **THEN** the command fails naming `<adapter>/<name>` and the reason, and the adapters that were fine are not written either
