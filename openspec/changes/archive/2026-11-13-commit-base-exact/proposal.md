## Why
- Crusader's R3a line check (#125) found the viewer's ground up to about 3 m off the stored B + L at line cells (434.1 against 435.2 at cell 68727, 5537), and the lead took it for a base difference under `--commit`.
- Probe over the `r3a-e1` commit, 625 cells around 68728, 5536 (read only, `crates/app/examples/commit_base_probe.rs`): the viewer's B minus the bake's B is 0.000 m at every cell (min, p50, p99, max). The drawn ground minus (B + L) is p50 -2.06 m, min -7.4 m, max +2.0 m, and the same minus (viewer B + L): all of it is the crease detail the field adds on top (GEN-109, ADR 0098). The cell 68727, 5537 reads 434.09 drawn against 435.19 B + L.
- REND-067 (crusader) places a river line along B and needs B equal to the bake's, with the commit's `base_id`, to compare.

## What changes
- VIEW-072: `WorldTerrain::bake_base()` returns B (the C1 read of the exact lattice, the bake's closure) and the commit's `base_id` for a commit opened in memory, none otherwise; a test holds it bit-equal to the bake's read.
- The probe example stays as a dev tool. The detail on the ground is the lead's separate decision (ADR 0122 proposed, GEN-187).

## Not in this change
- The crease detail over channel beds (GEN-187), the held-back tiles of a commit flight, the measure mode of `repro --window` (BUILD-054).

## Impact
app (worldfield.rs, lib.rs test, examples/commit_base_probe.rs). Spec: viewer.
