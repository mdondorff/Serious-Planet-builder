## ADDED Requirements
### Requirement: BUILD-047 The stage manifest is committed and required up to date
`docs/stages/manifest.txt` SHALL hold, for the stage probe's fixture (a compact landmass of 24 cells a face edge through the rounds, H1 and the sediment pass), one line per stage and field: the cells, minimum, 5th, 50th and 95th percentile, maximum, the scale counts, the hash of the data (never of a PNG) and the stage's numbers, without the probe's cost fields. A tier-A test SHALL run the fixture and fail when the file differs, naming the stage, the field and the first differing column. `cargo xtask stages` SHALL run that check and `cargo xtask stages --update` SHALL rewrite the file. The fixture's sheets SHALL be written to `target/review/stages/ci/`, which CI's review artifact carries.

Verify: A
Status: active
Source: Owner 2026-11-09 (stage previews: a committed text manifest; images in target/ and CI), architect order inbox #100, BUILD-046

#### Scenario: A stale manifest
- **WHEN** a number, a line or a whole stage differs between the file and the run
- **THEN** the test fails naming the stage, the field and the column, and says to run `cargo xtask stages --update`
