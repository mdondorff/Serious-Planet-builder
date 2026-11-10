## ADDED Requirements
### Requirement: GEN-156 The stored change L of the K7 chain is measured as kind 13 records
The measurement SHALL encode the core of the local change L of every 1,024-cell tile of the prototype region, from K7 with the channels burned in, as kind 13 counts streams and report the bytes per land cell, the spread over tiles and the extrapolation to an Earth-sized body.
- A stream SHALL decode to every cell within half a quantisation step (0.125 m) of L.
- The extrapolation SHALL name its land-cell count and say that the region is one area, not a global figure.
- The measurement SHALL NOT decide a store format; gate G-STORE is not closed by it.

Verify: D
Review: the measurement table of `openspec/changes/l-store/design.md`, judged by the architect (A3 section 12.1 gives G-STORE as a numeric gate)
Status: planned
Source: A3 section 10 (G-STORE), inbox #106 step 2

#### Scenario: the prototype region
- **WHEN** `k7_region 200 8 store` is followed by `l_measure`
- **THEN** the report lists every tile with its bytes and land cells, the pooled, median and highest bytes per land cell, and the Earth-scale size at the pooled and at the highest rate, with a round-trip error of at most 0.125 m

### Requirement: GEN-154 G-SURF is gated at the cell centres and the vertices are reported as G-SURF-I on the stored change
The surface gate SHALL read the runtime's `B + L` against the bake's surface at the centres of the local cells with `L` as the store keeps it, and SHALL report the lattice vertices as G-SURF-I without gating them.
- The gate at the centres SHALL hold: at most 1e-9 m before quantisation and half a quantisation step after.
- The stored change SHALL read back as the quantisation the gate uses (`round(L / step)` in `i16` counts times the step).
- G-SURF-I SHALL report the 50th, 95th and 99th percentile and the maximum of the difference at the vertices, the share over 0.25 m, the vertices and those over 0.25 m by the bake's local step height (up to 5 m, over 5 m up to 20 m, over 20 m), the largest quotient of the difference and the step height, the spatial concentration (the bins of 64 cells with vertices over 0.25 m against the bins with vertices, and the most in one bin) and the overshoot of the unclamped read of `L`.
- G-SURF-I SHALL also report the error against the local base slope (the difference over the largest rise of `B` across one cell, over the vertices whose rise is at least 1 m: 99th percentile and maximum, and their number) and the overshoot of the runtime read beyond the range of the four composed cells (maximum and the vertices over 0.25 m).
- The clamp of the C1 read SHALL stay.

Verify: A
Status: planned
Source: A3 section 12.1 (G-SURF, G-SURF-I), owner 2026-11-09, inbox #105 step 3

#### Scenario: a smooth tile, with and without a burnt channel
- **WHEN** K7 runs on a small tile and the gate is read
- **THEN** the centres hold, every vertex is in one step class and one bin, the percentiles are ordered, and a core of change encoded as a kind 13 stream decodes to the gate's quantised values
