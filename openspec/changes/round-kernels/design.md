## Context
- A3 §6.3 (F7): inside a round a sweep touches a cell and its receiver only, so a round computed per catchment on a shared routing equals the whole-region carve. Two couplings remain in the routing itself: the MFD area crosses D8 borders, and the flood's tie order depends on other catchments' cells.

## Decisions
1. Both rules sit behind `RoundRules` (default off). The layer key gets `0xC4, clip, canonical` only when a rule is on, so existing layers keep their keys (CON-07); the kernel version is not bumped.
2. MFD clip, local form: for cell `p` a lower neighbour `q` with a different catchment root is dropped from the weights and from their sum. The first wording (the share of a foreign neighbour goes to the cell's own receiver, `ToReceiver`) keeps the foreign levels in the denominator: measured, it does not close the area under the catchment (below). Differs from A3's first text; A3 is corrected in this change. A cell whose lower neighbours are all foreign cannot occur: its receiver is a lower neighbour of its own tree.
3. Canonical flood: no FIFO queue; every reached cell is pushed to the heap at `max(z, parent + epsilon)` and cells are visited by (level, id). The id order of a rectangular grid is the global id order (row-major), so it is the same in every window. The levels over a flat differ from the FIFO ones by sub-millimetre amounts (the gradient follows the id order, not the breadth-first distance).
4. G-ROUND is `Schedule::Catchments`: the breach and the sweeps visit the cells grouped by catchment (stable within the region order), catchments ascending or descending by outlet id. Bit-equal to `Schedule::Region` on smooth and terraced lands, under today's rules and the round rules (tests). This is the exactness of F7 on the new kernels; threads are step 2.
5. G-LOCAL is the round as a pure function of its key inputs. A catchment routed alone does not equal its part of the region on flats, for the measured flood rules (below); routing locality is G-PART.

6. Memory, not measured at face scale: with the clip on, the filled surface (8 bytes a cell) stays alive until the area is done, and `Schedule::Catchments` holds a grouped order (4 bytes a cell) for the round. Estimated +4 to +8 bytes a cell over today's route peak. The prototype region and the resident round of step 2 measure it; the face bake does not use the rules.

## Measurements (`examples/round_probe`; four 96 x 96 lands, about 820 catchments over 8 cells)
A catchment routed alone: the grid masked to its members, its outlet the only outlet, the same jitter; compared with its cells in the region routing.
| Rules | smooth: areas differ | terraced: areas differ | terraced: receivers differ |
|---|---|---|---|
| today's kernels | 26,041 of 26,265 | 27,090 of 27,942 | 0 |
| canonical flood only | 26,041 | 27,090 | 1 |
| ToReceiver clip + canonical | 23,939 | 25,043 | 1 |
| local clip only | 0 | 45 | 0 |
| local clip + canonical (round rules) | 0 | 7 | 1 |
- The remaining cell: a member on a near-sea-level flat first reached from a lower neighbour in another catchment (2.1660 m + epsilon in the region, 2.1674 m alone): the level moves by 0.4 mm, a neighbouring receiver flips, 7 areas follow.
- Probe (b), not in the kernels: a raised cell takes its flooder as receiver, the others keep the steepest jittered receiver. Then 0 of 54,207 member cells differ in filled level or receiver when catchments of that receiver set are routed alone. It stays an option if the alone form is needed.
- Flood cost at 3,072 cells a side, one thread: a land 1.30 s FIFO, 1.37 s canonical; broad terraces 0.29 s and 0.99 s (3.5x); 7.5e6 of 9.4e6 filled levels differ by sub-millimetre amounts there.

## Earth-specific values (ADR 0108 point 4)
No constant is introduced. The fixtures use a 800 m cell and 1 mm epsilon as in the carve's defaults (`CarveParams::epsilon`).
