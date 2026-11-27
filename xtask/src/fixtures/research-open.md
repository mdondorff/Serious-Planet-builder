# SPIKE-146: the C2 read against the facets (fixture, TEST-163)

Opened: 2026-11-28 (lead)
Exit: S-Test E3 (mode R)
Question: does a C2 read remove the 814 m facets on R1h?
Hypothesis: the C2 read removes the facets
Constraints: spike branch only; no default change
Out of scope: the K5 washboard
Evaluation (quantitative): facet excess Xdiag on R1h, F1, R3b
Evaluation (visual): the look sheet at a 10 degree sun
Falsifier: Xdiag stays above 0.05 on R1h
Code location: branch spike/c2-read

## Method
- rows on the five windows
