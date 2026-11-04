# SPIKE-101 High-mountain relief: why 0.21 of real, and the fix

Opened: 2026-11-04
Question: why does the high-mountain class reach only 0.21 of GLO-30's 3 km relief even as built (SPIKE-99), and which change (a ridged band for the high classes, a larger band amplitude, a steeper belt core, more pass-1 steps on high-mountain tiles) closes the gap without wrecking the hypsometry?
Why it matters: M9 plan item I9; the owner treats the high-mountain gap as its own item (2026-11-04; planner inbox #84).
Time box: about 1 day of agent work.
Success criteria: the cause split by term (belt core, band, erosion, class map); per candidate on seed 14's mountains tile, against the GLO-30 Alps and Everest samples: 3 km and 10 km relief p50, slope p90 and hypsometry, without and (for the best) with the erosion layer; the cost of more pass-1 steps; stills at 2 km and 10 km with a low sun, labelled; a ranked recommendation.
Fallback if it fails: record the decomposition and the gap per term as the input for R7 tuning.
Code location: branch `spike/high-mountain-relief` (throwaway; never merged into the main crates).
