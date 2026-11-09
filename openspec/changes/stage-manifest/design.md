## Fixture
The compact landmass of `bake_probe` (a cap of the cube at 24 cells a face edge, 4 km radius body, the sine surface of the tests): rounds (2), the H1 pass (`hydrology` on the last round's routing) and the sediment pass. The fixture driver is the test itself: it calls the production functions and installs a `SheetProbe` (the rounds and the sediment pass emit by themselves, PR 1); H1 and the landmass are emitted by the driver, because their production emit belongs to the explorer's phase plan (#109).

## The manifest line
`<stage> | <field> | cells N | min | p5 | p50 | p95 | max | below_scale | above_scale | hash | <numbers>` of the probe's `stages.txt`, with `probe_ms` and `probe_bytes` removed (they vary), sorted by stage order then field. The data is hashed, never the PNG bytes (an encoder version changes the bytes, not the data).

## The test and the command
- `stage_manifest::the_manifest_is_the_run_of_the_fixture`: the run's lines against `docs/stages/manifest.txt`; the failure names the stage, the field and the first differing column, and says `cargo xtask stages --update`.
- With `PLANET_UPDATE_STAGES=1` the test writes the file instead and passes (the one place that happens; `cargo xtask stages --update` sets it).
- The fixture is deterministic on any thread count and any machine (CON-14): the manifest holds the same lines on Windows and Linux (the determinism job already compares the two for other data; this test runs on both).

## Numbers
TEST-103: invariants the lines must satisfy, asserted on the run: the landmass cell count is the active set; thermal and the pit spreading keep the height volume within 1e-9; the deposition's eroded = deposited + exported within 1e-9; the area's maximum is the landmass area at an outlet. TEST-104: the pit-cell count of the surface at the end of each stage equals the count of H1's basin cells of the same surface (both definitions in the manifest).

## CI
The tier-A step already runs the test; the sheets are written to `target/review/stages/ci/`, and the existing `review-<OS>-ab-N` artifact (`target/review/`, always uploaded) carries them. No new job or step.

## What the fixture found
- H1 on the rounds' output has no pits: the rounds breach them (pass 1 leaves none); the pits of the fixture are on its own landmass and appear again after thermal, so TEST-104 compares the two definitions on three surfaces and requires pits on at least one.
- The deposition's `original` is the landmass before the rounds (the eroded column is `z0 - z`).
