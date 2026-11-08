## Why
ADR 0105 points 1 and 3: pass 1 only detaches material, so kept pits (GEN-120) deepen every step (p50 63 m against 3-7 m in real mountains), and flat alluvial floors and intermontane basins have no mechanism at pass 1's scale. Deposition changes the heights the routing sees, so it must be in the bake before the planet run (owner, 2026-11-05: all four review items in M9; plan row `pass1-deposition`, GEN-131).

## What Changes (planned; design notes only in this PR, no code)
- Pass 1's implicit carve gains a deposition term (Yuan et al. 2019 form), with eroded volume routed down the receiver tree and filling kept depressions up to their spill level before the rest passes on.
- Mass is kept: what is eroded is deposited or leaves through an outlet.
- The fill-depth channel (GEN-120, kinds 16 and 17) keeps its meaning: spill level minus the deposited surface.
- GEN-131 added; GEN-100 gains the sweep. Kernel version and key change.

## Impact
- Code (later): `generators::bake::{carve, route, face}`; the bake examples. No golden moves.
- Cost: an estimate is in design.md; it is measured in the first code PR.
- Depends on nothing unmerged; `arid-sinks` (GEN-132) builds on it.
