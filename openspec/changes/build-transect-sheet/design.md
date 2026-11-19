## Decisions
- Input: BUILD-060's chain layout only (`chain.txt`, `s0.f32`, `final.f32`); the face from `chain.txt`'s `switch window F,...` line or `--face`.
- Transects: SPIKE-119's rule (`spikes/river-suite/results/tools/transects.py`, unchanged), re-picked on g1f8's prototype `S0` of each window (the whole 8,160-cell window; area from `route_rules` on the plain flood, epsilon 1 mm, seed 7). Fixed until the base changes again; the stale flag shows when it did (lead #150a; per-run picking parked until Mountain Base v2).
- Stale against the pick's own `S0` depth, not the relief along the line: on the pick surface itself, depth under 10 % of relief marks 2 of 15 (M1 T3 on a mountainside, R3a T2 on a descending line); against the pick depth none is stale there, and on g1f8's diffusion row M1 T3 is.
- GLO-30: the matched profiles are data in the crate (about 15 x 200 samples); the tiles are not read at run time.
- Measures: the river level is the lowest sample within 250 m of the line's middle; depth is the lower side's top less it; width at h is the distance between the first samples above river + h on each side (linear between samples), open when a side never rises that high.
- Heights on the sheet: the chain's surfaces relative to `S0`'s river level (a lowering shows), GLO-30 relative to its own river point; one vertical scale per panel, its exaggeration printed.
- The command is light (a few profiles): no heavy lock.
