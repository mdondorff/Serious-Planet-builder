## ADDED Requirements
### Requirement: GEN-155 The sediment balance SHALL be the same under every partition the pass has
The deposition's balance per catchment SHALL be reported term by term, and the totals and the per-catchment terms SHALL not depend on how the region is partitioned.
- `catchment_terms` SHALL return for every catchment of the sweep's own route its outlet, cell count, eroded, raised and exported volumes and the relative residual of `eroded - raised - exported`; `catchment_balance` SHALL be the sum of those terms.
- The compact cube and the masked cube SHALL give the same catchments, the same terms (bit for bit, the residual included) and the same heights; the cells outside the landmass SHALL be left alone.
- The whole sediment pass SHALL give the same heights and report on any thread count, on a region that thermal cuts into more than two pieces.
- The sums of the terms SHALL match the eroded column read from the heights and the rise read from the volume change (relative 1e-9); the catchments, grouped by outlet, SHALL add up to the whole (1e-12) with the identity under 1e-9 in each group. This is the accounting of a cut along the divides, not a cut of the pass.
- The partition varied is stated in the test; a spatial cut of the pass is not built (open).

Verify: A
Status: planned
Source: A3 section 6.7 G-SED, inbox #116

#### Scenario: a compact landmass under three partitions
- **WHEN** a corner-cap landmass of a sphere of 24 cells a face edge is carved for two rounds and the sediment pass runs on the compact and the masked cube, on 1, 3 and 8 threads, and its catchments are grouped by outlet id modulo 2, 3 and 5
- **THEN** the terms, the heights and the report are the same bits under the first two partitions, and under the third the groups' totals add up to the whole's with the identity at most 1e-9 in every group
