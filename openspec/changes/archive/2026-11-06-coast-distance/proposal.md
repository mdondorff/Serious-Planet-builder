## Why
GEN-118 measured the shelf from the smooth coast (the continent stage without its detail), which lies up to hundreds of km from the field's own coast: active margins missed their preset (seed 2: p50 228 km against 30 km) and the floor needed a wide fade at the coast. A ray search from every sample to the field's coast was built and rejected (10 to 18 times the cost, steps over islets). A lattice distance transform over the field's own coast removes both.

## What Changes
- GEN-136: a lattice over the cube faces (512 cells a face edge, about 20 km) holding, per point, the signed distance to the nearest sea-level crossing of the continent value and the margin activity at that crossing. Crossings are found on the cell edges (a sign change, the point placed by linear interpolation); the nearest crossing to each point comes from a 3D bucket search within the cap (the widest shelf plus the slope); the read is bilinear. Built once per class source on the first sea read; the bytes do not depend on the thread count.
- GEN-118: the shelf hangs from it: the distance and the margin type are those of the coast of the field (islands included), so the floor needs only a short fade (coast ramp 0.01) and no blend into the abyss; the passive width default is 80 km (Earth's mean shelf width, Kennett 1982) instead of 150, because the field's coast is long and rugged and the share of the ocean under 200 m would otherwise be 12 to 17 %.
- `CLASS_SOURCE_VERSION` 8, `GLOBAL_VERSION` 9 (older stores refused; owner policy). The preview now measures the shelf from the coast lattice on the floor: where the 130 m and 4,050 m isobaths lie from the coast, by margin type.

## Cost budget
| | Value |
|---|---|
| build, once per class source (up to 16 threads, on a 20-core machine) | about 4.6 s wall (512 cells, 1.6 M field evaluations); measured once, not enforced by a test |
| read per sample | about 0.26 us (200,000 reads in 0.052 s; a bilinear read of two arrays) |
| memory | 6 x 513 x 513 x (4 + 1) B = 7.9 MB |
| the gradient-based shelf it replaces | 6 `continent_smooth` evaluations and one plate search per near-coast sea sample, several us |
| the rejected ray search | 10 to 18 x the gradient-based shelf (preview 108 to 187 s against 10 to 20 s; measured in the spike, patch not kept) |
The bake must build one class source and share it (`BakeInputs` owns one); `with_shelf` resets the lattice, so a changed profile costs one more build. A planet bake pays the build once and 0.26 us per sea sample; the 1.6e9 samples of a 16,384-cell bake cost about 7 CPU-minutes of reads. Tests that read a sea point pay the build once per class source (about 4 s on a loaded machine with 20 threads, more on fewer).

## Earth-specific values (ADR 0108)
`ShelfProfile` gains `coast_lattice_cells` (512, calibration), loses `abyss_blend`, and the Earth defaults change: `passive_width_km` 80 (Earth state), `coast_ramp` 0.01 (calibration). The table is in GEN-118.

## Numbers (20 km a pixel, `generate --shelf-preview`, seeds 1, 2, 3)
| | seed 1 | seed 2 | seed 3 | preset |
|---|---|---|---|---|
| shelf share of the ocean (under 200 m) | 10.9 % | 7.8 % | 7.6 % | Earth 7 to 9 % |
| passive: shelf break p50 from the coast | 75 km | 76 km | 75 km | 80 km |
| active: shelf break p50 from the coast | 30 km | 30 km | 31 km | 30 km |
| passive: foot of the slope (4,050 m) p50 | 152 km | 152 km | 152 km | 80 + 72 km |
| active: foot of the slope p50 | 103 km | 104 km | 104 km | 30 + 74 km |
The preview time falls from about 20 s to 13 s.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-118 (modified), GEN-136 (new)

## Known limits
- The preview's distances read the same lattice the floor was built from, so the break at 30 km (active) and 75 km (passive) is the parameters read back, not an independent check; the independent evidence is the exactness test on a synthetic island, the continuity walks (a face-edge walk and the 5 m bound at every coast of the field) and the share of the ocean, which depends on the field's real coastline.
- Seed 1's shelf share is 10.9 % (seeds 2 and 3: 7.8 and 7.6 %): outside the 7 to 9 % target.
- The passive width of 80 km is a calibration (Earth's mean shelf width, tuned to the share on this generator's longer coast), not a measurement of Earth's passive margins; an off-Earth preset sets its own.
- The shelf profile (and `coast_lattice_cells`) is not hashed into the store keys; a world definition that sets it needs that first (`with_shelf` has no caller yet).
- The coast lattice is built on the owner's threads where the owner gives them (`with_coast_threads`; `BakeInputs` passes its own), else on the machine's (at most 16).
- The removed near-shore pit test guarded the smooth-gradient estimate, which no longer exists.
- Supersedes the distance and fade of `2026-11-06-shelf-profile` (history; its proposal describes the smooth-gradient estimate, 150 km passive width, a 0.06 ramp and an abyss blend).
- Seed 1's share is 10.9 %: its coast is longer than the other seeds'.
- The lattice's coast is within a cell (20 km) of the field's: crossings on cell edges miss islets under a cell; the fade ramp and the continuity test bound the step at 5 m at the field's coast.
- The preview's isobath bands (within 15 % of 130 m and of 4,050 m) are pixel samples of the stored floor.
