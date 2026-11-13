# 0122 Fine detail pinned to the stored samples and off on river lines
Status: accepted (owner, 2026-11-13: "this reads as acceptable"; the risk is the cost per node; watch for a 100 m lattice at low sun)
Date: 2026-11-13
Source: SPIKE-113 (#1471, #1474); explorer's commit-base probe (#135, 2026-11-13); amends ADR 0098 point 2

## Context
- ADR 0098 point 2 keeps the crease octaves up to 2 layer cells (about 200 m) as the fine-detail term, with an amplitude set by the base elevation only.
- Under `--commit` the drawn ground differs from the stored B + L by p50 -2.1 m (min -7.4, max +2.0 m) on R3a; the base B is bit-equal to the bake's, so the whole difference is this term.
- On a river line the term draws ±1-7 m of relief over the stored monotone bed; the owner saw it as water flowing uphill (flight finding F2).
- SPIKE-113: detail pinned to 0 at the layer's cell centres keeps every stored sample and every receiver; the same detail unpinned changes 3.9 % of samples and 17 % of receivers.

## Decision
1. Where the erosion layer is read, the fine-detail term is pinned: the term minus its bilinear interpolant from the four surrounding cell centres, so the drawn ground equals the stored samples at every cell centre.
2. On a river line's trench cells (the line placement of REND-067, its trench width) the term is 0, with a fade of one cell outside the trench.
3. Where no layer is read, ADR 0098 points 2 and 3 stand unchanged.

## Consequences
- The drawn ground under a commit agrees with the stored data at every sample; rises drawn on a river line come only from the stored data.
- The pinning costs about 5 times the unpinned term per node (SPIKE-113, an upper bound from a loaded machine).
- Point 2 depends on the line placement at read time (REND-067); until it lands, point 1 ships alone.
- Lit goldens over the layer move; they go through golden-review as a declared change.
