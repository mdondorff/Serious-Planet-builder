# round-f32 (GEN-166 PR 1, TEST-120)

## Why
- A3 section 9: the round surface is stored f32; the round computes in f64 and the routing and sweeps read the widened heights (BUDG-016 lever; PR 2 stores the carried heights as f32).
- The whole bake so far carries f64 across rounds; every G-ROUND, G-PART, G-DET pass holds for f64 only.

## What (PR 1: the narrowing)
- `CarveParams::round_f32` (default false). `carve::narrow_f32`: `z = f64::from(z as f32)`, refuses a height that is not an f32.
- Narrowed at the end of every round and after the last one: `carve_rock` (before the next routing, and at the end), `solve_rounds_with` and `solve_rounds_memo` (after the write-back).
- Keys: the layer key byte `0xC6` and the catchment key bit 61, only when set; off, every key and result is today's.
- The callers of the round rules turn it on: the default `BuildParams`, `landmass_scale`, `rounds_region`, `k7_region`, `region_scale`, `edit_cases`.
- The memoised solve's refusal without the canonical flood is tested (architect's small item).
- PR 2 (not here): f32 storage of the carried heights, then G-SCALE.

## Declared moves
- The plan's stage hashes and the k=1 plan hashes change (the rounds' heights are narrowed): stated per fixture in the PR.
- G-EQ (f32 against f64 on R3a, R3b: relief within 2 %, membership and channel status on at most 1 %) is the region gate after this PR, not a tier A test.
