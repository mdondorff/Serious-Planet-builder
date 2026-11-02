Opened: 2026-10-28
Question: If the chunk stage cut rivers to a realistic hydraulic depth (depth from discharge, about 3 to 15 m, wider channels), how much do the chunk change's max and p99, criterion 3's level steps and the held-back tiles shrink?
Why it matters: input to the owner's D0 choice (ADR 0084): today the change max is 207 m, the step 15/14 over the 0.25 m bound, 112 tiles held back at frame-116591's pose.
Time box: half a day
Success criteria: (a) change max and p99 on 24 trunk chunks and 48 random chunks; (b) steps 15/14 and 16/15 on chunk 0x0b000000002cba11; (c) held-back tiles at frame-116591 for the down bound the new max allows
Fallback if it fails: none (measurement)
Code location: branch `spike/m8-channel-depth` (local, not pushed; on `change/m8-group-d`'s probes): `PLANET_ANCHOR` override in `ANetParams::spike`; patch in `proto/anchor_env.patch.txt`
Closed: 2026-10-28

## Method
- Store `C:/DEV/SeriousPlanetData/global16384` (read-only), in memory only, one process per probe; the probes of `change/m8-group-d`: `carve_threshold_report` (24 trunk-biased chunks, "no carve" row, level 16), `anet_change_report` (48 random land chunks, levels 11 to 16), `anet_steps_report` (the entry chunk), `held_back_report` (frame-116591, `PLANET_HELD_DOWN`).
- Variants are values of the existing `AnchorLaw` (GEN-047, GEN-059, GEN-060), set by `PLANET_ANCHOR`:
  - **base**: defaults (valley depth 3 m (A/1e8)^0.5 capped at 30 m lowland to 100 m by the relief factor; cut cap 200 m).
  - **d1**: hydraulic depth from SPIKE-40's law d = 0.27 Q^0.3 (as `depth_m` 0.27, `exponent` 0.3: 8.5 m at 1e7 km2), cap 15 m at every relief, cut cap 15 m.
  - **d2**: d1 with cut cap 30 m and a valley floor of 10 bankfull widths in lowland (`floor_low` 10, was 5).
  - **d3**: the old law, caps 15 m, cut cap 30 m.
- The bankfull width law (7.2 Q^0.5) is unchanged in all variants.

## Results
| Variant | trunk 24, level 16: p99 / p99.9 / max (m) | random 48, level 16: p99 / max (m) | steps, entry chunk: 15/14 p99 (max), 16/15 p99 (max) (m) | entry step at level 15 vs base: p99 (max) |
|---|---|---|---|---|
| base | 73.2 / 170.7 / 206.7 | 48.7 / 206.7 | 0.390 (9.60), 0.095 (3.31) | 110.4 (111.5) |
| d1 | 21.9 / 43.1 / 76.0 | 14.8 / 76.0 | 0.183 (9.51), 0.043 (3.28) | 63.0 (63.6) |
| d2 | 28.7 / 46.8 / 76.0 | 25.7 / 76.0 | 0.219 (10.06), 0.051 (3.47) | 78.0 (78.6) |
| d3 | 28.0 / 43.2 / 76.0 | 23.2 / 76.0 | 0.197 (9.51), 0.045 (3.28) | 78.0 (78.6) |

- Held-back tiles at frame-116591's pose (plan with the chunk stage; 0 of 1,805 with none): symmetric 300 m widening 112 of 1,340; asymmetric down 250 m / up 10 m 89 of 1,555; **down 80 m / up 10 m 26 of 1,881; down 60 m / up 10 m 24 of 1,858** (80 m is the bound the 76 m max allows; 60 m is below the max, shown for the floor of the count).
- Signed range after: -76.0 to +1.2/+1.5 m (base -206.7 to +1.2): the cut cap raises no dams that exceed the old +1.2 m.

## Findings
1. **Hydraulic depth cuts the change max from 207 m to 76 m, p99 on trunks from 73 to 22 to 29 m, p99 random from 49 to 15 to 26 m.** d1 (depth law 0.27 Q^0.3, cut cap 15) is the smallest; wider floor (d2) and the larger cut cap (d3) raise p99 by 5 to 10 m.
2. **The remaining 76 m is not the channel**: it is the same in all four variants (at chunk (4, 73, 1949), the largest of d1 to d3), so a cause outside the anchor law's depth and cut caps limits it (not identified here; the stage's incision and slump are untouched).
3. **Criterion 3**: the step 15/14 at p99 falls from 0.39 m to 0.18 m (d1), 0.22 (d2) and 0.20 (d3), under the 0.25 m bound; the step max (9.5 to 10 m) and 16/15 max (3.3 to 3.5 m) do not move, so those maxima are not channel depth either. The 0.71 m figure in the request does not appear in this probe's output (p99 0.390, max 9.60 at 15/14 for the defaults); the figure may come from another measure of the probe.
4. **The entry step against the base** (level 15 vs the base at the pyramid top) falls from 110 m to 63 to 78 m p99: still far over any per-frame fade, so hydraulic depth shrinks the disconnect by 30 to 43 % at p99 but does not remove it (the SPIKE-49 result stands: the base itself must carry the channel).
5. **Held back**: the down bound the new max allows (80 m) holds 26 tiles at frame-116591's pose against 112 (symmetric 300 m) and 89 (250/10); 60 m holds 24.
6. Limits: one law shape (the depth exponent 0.3, `d_coef` as SPIKE-40's recalled numbers, marked r there); no visual check of the narrower valleys (hillshade) and no check of lowland corridors against GEN-060's valley sides; the held-back count is independent of the law (it reads the bound only).

## Visuals
Nothing to look at: numbers of heights and counts only, no renders.
