# SPIKE-99 The relief driver k: drop it, with or without an uplift map

Opened: 2026-11-03
Question: what does dropping the relief driver k (GEN-097, GEN-099, ADR 0088 point 2) do to relief realism, and does relief from an uplift map plus the erosion bake close the gap?
Why it matters: the owner expects to drop k but fears a relief gap that hurts the realism statistics (owner, 2026-11-03; planner inbox #80); the M9 bake design depends on it.
Time box: about 1 day of agent work.
Success criteria: per relief class and scale band (local relief in 1/3/10 km windows, std per band 0.2-1/1-3/3-10/10-30 km, slope median and p90, hypsometry), the gap between base 2 (k = 1) and the GLO-30 samples, and the share of that gap base 3 (k = 1, uplift map) closes, each with and without the erosion layer, on seed 14's three preview regions; a ranked recommendation (drop k with base 3, drop k with something else, keep k).
Fallback if it fails: keep k (base 1) and record the measured gap as the reason.
Code location: branch `spike/relief-driver-k` (throwaway; never merged into the main crates).
