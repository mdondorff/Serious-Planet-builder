# item13-findings (briefing item 13, step 5), example and results

Architect 2026-11-10 (sheet review of R3a and R3b): results section 6b with G-LAND, G-SED, G-BASIN and the findings (stripes, fans, deposition along the stem, depression depth by stage).

## What
- `region_scale` prints, after the sediment pass: the `stem` line (the largest-discharge mouth whose stem neither starts nor ends within 3 cells of the window border; its height, distance from the border and whether it is an outlet cell; the sediment pass's change of height along it by tenth, in lakes and outside them), the deposit split by lake and open cells over the window's land and over the region proper, and the `depth` lines (the depression depth along the stem and the region's land under the fill at the base, after 1 to 4 rounds and after the pass; the rounds are re-run, about 6 rounds more).
- Results section 6b and the raw outputs `item13-r3a-findings.txt`, `item13-r3b-findings.txt`, `item13-diag.txt` (the ad hoc stripe and fan diagnostics, not committed as code). The first stem of R3b ended in the window's cut; the section says so and gives the corrected stems.
- Example only: no library code, no requirement, no test.
