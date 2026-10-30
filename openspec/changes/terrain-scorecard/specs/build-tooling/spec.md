## ADDED Requirements

### Requirement: BUILD-029 `cargo xtask terrain-score`
`cargo xtask terrain-score [--store DIR] [--dem DIR] [--sites FILE] [--out DIR] [--maps]` SHALL run the scorecard (TEST-079) in release under the heavy lock (BUILD-026), on the CPU only. Defaults: the linked production store, `C:/DEV/SeriousPlanetData/dem/glo30-samples`, `crates/app/data/terrain-score-sites.toml`, and `target/review/terrain-score/<date>`. It SHALL print the report's path and its wall time, take at most 60 s on the reference machine, and refuse a missing store or tile directory by name.

Verify: A
Status: planned
Source: SPIKE-57 (about 1 minute), BUILD-002, BUILD-026

#### Scenario: Arguments
- **WHEN** the command is parsed with no options, with every option, and with an unknown option
- **THEN** the first runs the example with the defaults, the second passes each option through, and the third is refused naming the known options

#### Scenario: Missing input
- **WHEN** the tile directory does not exist
- **THEN** the command fails naming the directory and the `--dem` option
