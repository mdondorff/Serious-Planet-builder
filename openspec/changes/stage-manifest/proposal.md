# stage-manifest (BUILD-047, TEST-103, TEST-104; briefing item 18, PR 2)

## What
- `docs/stages/manifest.txt`: a committed text manifest of a small fixture (a compact landmass of 24 cells a face edge, the new chain: landmass, each round, H1, thermal, deposition, pit spreading): per stage and field a data hash and the key numbers, one line each, in the format of the probe's `stages.txt` without its cost fields.
- A tier-A test (about a second) runs the fixture through the production functions with a `SheetProbe` installed, writes the sheets to `target/review/stages/ci/`, and fails when the manifest differs from what the run produced, naming the stage, the field and the first differing column. CI runs it with the other tier-A tests, so a stale manifest fails CI.
- `cargo xtask stages [--update]`: runs that test; `--update` rewrites the manifest from the run. One command, one file.
- Tier A on the numbers (TEST-103): per stage the key numbers' invariants (counts, ranges, volumes conserved by thermal and the pit spreading, the deposition's balance); (TEST-104): two definitions of one quantity agree: the pit cells of the surface (`pit_cells`) equal the cells of H1's basins; the eroded column after thermal equals the sediment report's.
- CI uploads the fixture's sheets as an artifact (`stage-sheets`), so a reviewer sees them beside the diff. The sheets are not committed here (PR 3 commits them at gates, under a size cap).

## Not in this change
- The advisory agent (PR 3). Committing sheets.
- The production phase plan's emits (the explorer's #109) and K7's (follow-up).
