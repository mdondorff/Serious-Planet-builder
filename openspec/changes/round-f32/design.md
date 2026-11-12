# round-f32: design (GEN-166; architect D24 row 8, inbox #125)

Status: PR 1 (the narrowing) implemented on `change/round-f32`; PR 2 (f32 storage) and the region gates follow. Heavy runs: the bulldozer's slot.

## Contract (A3 §9)
- The round surface is stored f32; the round computes in f64 and the routing and sweeps read the widened heights.
- Narrowed once, at the end of a round; read by the next round, K6, K7.
- Today `solve_rounds` and `carve_rock` carry f64 across rounds; every G-ROUND, G-PART, G-DET pass so far holds for f64 only.

## Where the narrowing sits
- Point: after the last step of every round (a round = `reroute_every` steps), before the next routing reads `z`, and after the final round.
- `carve_rock`: at `step % reroute_every == 0` with `step > 0` before `route_rules`, and after the last step.
- `solve_rounds_with`, `solve_rounds_memo` (and the partitioned/compact paths that write `z` back): in the write-back loop of each round; same cells, same rule.
- Cells of one-cell catchments (lone outlets) are not queued: their height is the input's; narrowing is idempotent on a value that is already f32, so they are narrowed too (cheap, uniform).
- The breach is part of the round (it acts on `z` at the round start, in f64); it is not narrowed separately.
- The taper of the uplift reads the heights at the start of the call (`taper_of`), unchanged.
- A new `CarveParams` field `round_f32: bool` (default false: the default bake, K7, the sediment pass and every existing test keep f64). Callers using `RoundRules::ROUND` turn it on (the plan, `rounds_region`, `k7_region`, `landmass_*`).

## Rounding rule
- `z = f64::from(z as f32)`: round to nearest, ties to even (the `as` cast), on the supported targets (x86-64 SSE2, MSVC and Linux).
- Heights are finite (asserted) and |z| < 1e4 m: no overflow.
- ulp(f32) at 800 m = 6.1e-5 m, at 4,000 m = 4.9e-4 m, at 8,192 m and above = 9.8e-4 m. The flood epsilon is 1e-3 m; the filled surface is f64 scratch derived from the widened heights. A cell the sweep clamped to `receiver + epsilon` can end a round with a descent just below epsilon after the narrowing (at 512 m to 16,384 m the f32 spacing is at most 2^-10 = 9.766e-4 m, a deficit of 2.3e-5 m): the next flood lifts it by that much (a micro-fill below every pit threshold). Above 16,384 m the spacing exceeds epsilon and a carved chain of epsilon steps collapses to equal heights (ties by cell id): a region with relief above 16 km needs a larger epsilon or the f64 round surface. Reported in G-EQ with the receiver changes. Two neighbours at 8 km or more can be bit-equal after narrowing; equal heights are ties by global cell id (§6.4), deterministic.
- A per-round change below half an ulp does not move a cell (lost at the narrowing point, not accumulated). Measured under G-EQ: the share of land cells whose round change is below half an ulp, by round.

## What G-EQ expects (§12 row, text unchanged)
- Re-solve against a fresh build, E1-E7: bit equality, both through the f32 form (the memoised and partitioned paths narrow exactly where the plain one does).
- f32 canonical against the f64 run, on R3a and R3b (reported, not assumed): relief within 2 %; catchment membership and channel-cell status changed on at most 1 % of land and channel cells; no connectivity change.
- Also reported: |dz| p50/p99/max after round 4; receiver changes per round (share of land); pit share at 0.5 m against the f64 run on the same fixture; H1 basin counts and classes.
- A failure is an architecture-gate failure: it goes to the architect before any workaround.

## Fixtures that change
| Fixture | Change |
|---|---|
| `bake_round.rs` bowl and depth fixtures, sea-outlet digest `0xb9e6_6858_0505_bf11` | new digest for the f32 run; the f64 digest stays as the `round_f32 = false` case |
| `bake_rounds.rs` (1, 3, 8 threads), `bake_rock.rs`, `bake_compact.rs`, `bake_cube.rs` | f64 cases unchanged; one f32 case each: solver = `carve_rock` (G-ROUND) |
| partitioned and memo tests (G-PART, G-INC) | f32 case: partitioned = unpartitioned, memo = fresh |
| `bake_edits.rs`, `bake_eviction.rs` (E1-E7, G-EQ) | f32 case: re-solve = fresh, undo/replay/after eviction |
| `stage_manifest.rs`, `bake_probe.rs` | stage hashes of the f32 plan; the probe reads the narrowed `z` |
| k=1 plan hashes (`landmass_scale 1 8 plan`) | change; the new ones are stated in the PR with the reason (the rounds' z narrowed) |
| H1, K7, goldens | change only where they read the round output with the flag on; stated per fixture |

## Memory
- The narrowing alone does not lower the peak: `z` stays `&mut [f64]` (values f32-representable).
- A saving needs f32 storage of the carried heights (8 B to 4 B a cell; 148,929,871 land cells: about 0.55 GiB for one copy) and a widening read in `gather_with`: a second step, after the numerics pass the gates. G-SCALE (two k=4 runs) follows the storage step; peak commit and working set against 11,444 MiB.

## PR 2: the heights are stored f32 from the rounds to the sediment pass (GEN-166)
Where the bytes are (phase-plan measurements, k=4, 149.3 M cells, spill 1,194,732,528 B): `z` is f64 (1,139 MiB) from the landmass to the end of the plan; the peak is in hydrology (working set 9,741 to 9,956 MiB, peak 10.6 to 10.9 GiB, commit 11,376 to 11,387 MiB). The rounds' own peak (working set 7.6 GiB) is under it.
- What changes: after the rounds `build_resident` narrows `z` to `Vec<f32>` (570 MiB; exact, the heights are f32-representable under `round_f32`) and drops the f64 copy. The span from after the rounds to before the sediment pass (drivers, the weighted route and its filled surface, hydrology, G-CON, river pieces, channel graph, lakes) reads the f32 heights. The sediment pass widens them back to f64 (it needs `&mut [f64]` against `z_start`); its working set (4.2 GiB) is far under the peak. `Built.z` stays `Vec<f64>`.
- How the readers take f32: a trait `Heights` (`len`, `at(i) -> f64`; implemented for `[f64]` and `[f32]`) and the readers generic over it: `priority_flood_with`, `priority_flood_ungraded`, `lifted_flats`, `regrade_flats`, `route_rules_filled`, `hydrology`, `river_pieces`, `ChannelGraph::from_hydro`, `drivers`. Widening f32 to f64 is exact and `at` is an inlined read: the f64 instantiation is today's code (same operations in the same order), so every f64 result and key is bit-identical; the f32 instantiation reads the same values, so the plan's hashes are the f64 plan's hashes with the flag on.
- Only under `round_f32`: with f64 rounds the heights are not f32-representable and the plan keeps `Vec<f64>`.
- No key change: the storage is a memory layout of the same values (no key byte, no kernel version).
- The bar (architect, 2026-11-10, inbox #125): the step is kept only if it saves at least 200 MiB at k=4. Measured as the peak commit and the peak working set of the complete resident build (the MARK lines of `landmass_scale 4 8 plan`, two runs of the branch against the ledger's G-SCALE row of the same code: 11,387 and 11,376 MiB peak commit, 10,883 and 10,873 MiB peak working set); expected saving 570 MiB at the hydrology peak. The hashes (H1 label, channel, segments, z after the sediment pass) are equal to the f64 plan's.
- Tier A first (no heavy run): `Heights` on both types gives the same bits through the flood, the weighted route, H1 and the graph on a small compact cube; the plan with the flag on equals the plan on the f64 copy of its surface, hash for hash; `build_resident` holds no f64 `z` between the rounds and the sediment pass (a counter of live f64 height arrays at the probe marks).

## Key
- D24 row 9: a key part only when on. The layer key adds the byte 0xC6 and `catchment_key` the bit 61 for `round_f32` only when set; with it off the keys are today's bytes (no BAKE_KERNEL_VERSION change). Pass 2 and K7 refuse the flag (their tile key has no byte for it).

## Steps
1. Failing tier-A fixtures: `carve_rock` with `round_f32` equals `solve_rounds` with it, bit for bit, on 1, 3 and 8 threads; narrowing idempotent; bowl and sea-outlet digests.
2. The flag at the points above (carve, round, memo, partitioned).
3. Numerics-reviewer; verifier.
4. Re-run G-ROUND, G-PART, G-DET, G-EQ on R3a/R3b; report the f32-against-f64 numbers above.
5. Storage step (f32 carried heights), then G-SCALE.
