# Look review 1, second site after the bank rule (GEN-058, ANET_VERSION 3)

Same site, poses and command as `docs/milestones/M8/look-review-1-hills/` and `look-review-1-hills-ra/` (R-a, anchors on the ground), with the banks settled to the angle of repose. Not goldens.

| Item | After R-a | After the bank rule |
|---|---|---|
| Pairs over 0.9, steepest-site chunk (`chunk_incision`) | 6,010 (with a protected cell 2,516) | 570 (228) |
| Pairs over 0.9, second-site chunk | 20,612 (4,474) | 4,566 (386) |
| Steepest pair of two unprotected cells, sites 1 and 2 | 2.33 and 3.37 | 1.27 and 1.64 |
| Capped plan nodes at the six stills (`stills.txt`) | 126 to 131 | 122 to 126 |
| Capped tiles of the 30 m pose (126 now) with an 8 m slope over 0.9 / over 3.42 (`capped-30m.txt`) | 22 / 10 | 11 / 1 |
| Steepest capped tile of the 30 m pose | 4.64 | 4.48 |
| Snap | 446 m from the anchor cell, bearing 195 | 334 m, bearing 200 (the chunk field changed) |

Reading (coder, not a review): the protected pairs over the repose slope fall about tenfold, which was the aim; the capped plan nodes fall by only 4 to 6 because what still caps is flat tiles next to the gorge (115 of the 126 capped tiles of the 30 m pose have a steepest slope under 0.9), held by the height extent of their neighbourhood and the ceiling of ADR 0074, not by slope. What remains over 0.9 is mostly the slump rule's rate limit between free cells and cells next to a channel that stand lower than it.
