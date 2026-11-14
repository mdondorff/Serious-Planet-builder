## Why
- P3 E1 and E2 judge every river candidate by the same numbers: closed basins on river length, cuts over 50 m, lakes at D, relief kept, the edge against H1 (`docs/milestones/P3.md`).
- Today these numbers live in three spike branches (`rivers_at_l`, `erosion_at_l`, `drain_base`), each with its own copy of the measures (lead, 2026-11-14, explorer #143).

## What changes
- BUILD-055: `cargo xtask river-gate <store dir | window | r3a>` prints the gate's numbers on one region at L, in release, under the heavy lock; exit 0 (a report, not a verdict).
- `planet_cache::river_gate`: the measures as a pure function of the region's start surface, the candidate surface, the outlets, the region's cells and H1's edge crossings.
- The `river_gate` example builds the input: the start surface (the base's C1 read plus the regional field) over a window, or that surface plus each stored L tile's change over a commit's tiles.
- TEST-136: tier-A fixtures for every measure and for the xtask's arguments.

## Not in this change
- The bars' verdicts (P3 E1's bars are printed beside the numbers, not judged).
- SPIKE-117's pit cell minimum N: a parameter (`--min-cells`, default 1) until crusader's spike reports.

## Impact
cache (river_gate.rs, examples/river_gate.rs), xtask (river-gate). No stored format, key or default path changes.
