# O16: scoping the three base fixes and RG's own items (lead #184x, 2026-11-25, spikes)

Status: lock-free (code reading on origin/main 88bde7c5e, existing dumps, counterfactuals on dumps). No chain runs, no fixes.
- Scripts: `tmp/g1f6/spikes/s184/attr-184w/scripts/` (`fix1.py`, `fix2.py`, plus #184w's).
- **Correction to #184w item 4:** W's "21-30 cell" wavelength came from an unwhitened annulus whose red spectrum peaked at its edge.
  - Whitened, W's orientation lock sits at 10-18 cells (1.0-1.8 km): SPIKE-139's diagonal comb (814 m x sqrt 2 = 11.3 cells) and its 2-cell harmonic (16 cells).
  - There, 15-22 % of the band's power lies in one 10-degree bin at 45 or 135 degrees (uniform 5.6 %). F1's crests are at 135 degrees.

## 1. K7's C1 read of K5's change (O12 option A)
**(a) Site and rule.**
- `crates/generators/src/bake/k7.rs:39-56` `RegionField::at`: a Catmull-Rom (C1) 4 x 4 read of the 814 m field, clamped to the min/max of the four cells around the point (`:54-55`), the window edge clamped (`:45-50`).
- Consumers:
  - `crates/cache/src/region_bake.rs:259` (the regional field), `:305` and `:439` (S0 = B + read(z_s - z_b));
  - RG reads S0 from it (rg_chain `start`);
  - the same clamped Catmull-Rom is `map::change_in_face` (`crates/generators/src/bake/map.rs:12-35`, used by `layer_tiles`).

**(b) Candidates** (counterfactual: S0 rebuilt from the dumped z_s and z_b at the H1 centres; today's read reproduces the dumped S0 to 0.02-0.09 m rms):

| read | R1h d135 patch | F1 river patch | F1 d45 patch |
|---|---|---|---|
| today, CR clamped: X8 / along-trunk H1-locked rms floor, edge (m) | 1.00 / 11.3, 4.8 | 1.02 / 4.3, 2.6 | 0.89 / 2.0, 1.1 |
| CR unclamped | 1.05 / 11.3, 4.8 (p90 relief +18 m) | 1.09 / 4.3, 2.6 | 0.92 / 2.0, 1.1 |
| C2 interpolating B-spline | 0.81 / 9.5, 4.7 (p90 relief +39 m: overshoot) | 0.85 / 3.6, 2.4 | 0.72 / 1.6, 1.0 |
| **C2 quasi-interpolant** (B-spline, prefilter [-1/6, 4/3, -1/6]) | **0.56 / 4.4, 2.1** | **0.60 / 1.6, 1.1** | **0.40 / 0.7, 0.4** |
| relief p50 / p90 change, qi | -3.8 / -8.3 m (-2.5 %) | -5.8 / -2.5 m (-6.4 %) | -2.3 / -2.4 m (-3.8 %) |
| cells moved by qi, rms / p99 | 6.9 / 34 m | 2.8 / 13 m | 1.2 / 4.7 m |
| approximating B-spline (smooth bound) | 0.35 / 2.0, 1.1; relief -7.6 / -55 m | 0.41 / 0.7, 0.4; -14 / -22 m | 0.19 / 0.4, 0.2; -7 / -9 m |

- B alone: X8 0.02-0.03; GLO-30's threshold 0.10.
- SPIKE-143 for scale:
  - S0's X8 is 0.74 (R1h), 0.98 (F1), 1.07 (R3b);
  - KH with `qi`: X8 off the zone 0.355 -> 0.094, relief lost 1.6 m, `bar_relief_lost` fails by +0.77 m.

**(c) Effect.**
- **Item 1:** qi removes 58-63 % of the along-trunk H1-locked rms on the floor and 55-66 % on the edges.
- **No read reaches X8 0.10.**
  - The clamp is not the cause: unclamped is worse.
  - The H1-scale content is in z_s itself, K5's checkerboard relief (SPIKE-139; F1's checkerboard P rms is 3.7 m). A read can lower it but not remove it: no read gets below 0.19 without losing 7-14 m of relief p50.
- **Items 2-5:** none directly. SPIKE-139 reading 4: a read lowers the comb's amplitude, not its diagonal share.

**(d) Effort and risk.**
- **Effort:**
  - code: about 0.5 lane-day (a qi read beside `at`, the generators spec delta, tier-A continuity and determinism tests);
  - review: 0.5 lane-day (numerics review);
  - rows: five windows, chain plus gate, about 1 h of lock each.
  - Total about 2 lane-days and 5 h of lock.
- **Risk:**
  - qi moves cells by up to 34 m at p99 on K5's one-cell channels (it partly fills them). Corridor, agreement and closed basins can move, and this is unmeasured.
  - It is a partial fix (X8 0.40-0.60).
  - The bake's C1(z) in G-SURF (`k7.rs:867`) and `map::change_in_face` must follow, or the runtime and the bake disagree. Unchecked.

**(e) Rebaselines.**
- S0 everywhere: every RG reference row (defaults and step 1's sign-off rows on all windows), the G3 rows and the gate's references read from S0.
- Not B.
- Goldens on main: probably none, since `RegionField` feeds region bakes and commits, not the tier-C scenes. Unconfirmed: `test gpu` on the change settles it.
- Explorer's flight stores (commits baked through `region_bake`): rebaked.

**(f) Post-port-able:** yes. It is a main read under S0; no RG stage changes. The gate's 15 windows rerun as acceptance.

## 2. K5's washboard (SPIKE-139; O5)
**(a) Site and rule.**
- `crates/generators/src/bake/round.rs:45-86` `catchment_round`:
  - a breach (`:55-69`);
  - then `steps` implicit stream-power sweeps along the D8 receivers, `h = (z + U + f h_r) / (1 + f)` with `f = k_dt s sqrt(A) / d` (`:70-85`).
- There is no hillslope term.
- Routing: jittered D8 on the filled surface with MFD area (`route.rs:71-75`, `:224-230`).

**(b) Which term makes it.**
- Stream power with no hillslope coupling. On a diagonal side slope, every form sends all flow to the diagonal neighbour, so the two cell parities never couple (SPIKE-139 reading 3).
- It shows as 814 m checkerboard relief, 3.5-16 m rms against 0.3 m in the base, which the read shows as a comb along diagonals.
- Wavelength 1.0-1.8 km (11-16 L cells), crests on the lattice diagonals (F1 135 degrees).
- F1's stage-1 breach trenches seed it: band share 0.121 before K5, 0.189 after.

**(c) Candidates.**
- **A model change, not a parameter.**
  - The parameter (`--k5-diffuse 0.02`, spike only) removes most of it: band share 0.139 -> 0.086 (R1h), 0.189 -> 0.076 (F1).
  - Its cost:
    - 14-22 % of relief;
    - it closes carved channels: R1h's post-K5 drain meets 241 -> 2,766 depressions and lowers 0.27 -> 22.1 km3, and deep basins rise 32 -> 97;
    - k_dt compensation makes axis-aligned blocks.
  - The owner turned it off for G2.
- **(i) Parity-coupling hillslope term:** diffusion only between cells not on the same receiver chain, so one-cell channels stay open.
- **(ii) Transport-limited deposition** in place of linear creep.
- Either one needs K and diffusivity calibrated together against GLO-30 (relief, spectrum), and wider stage-1 trenches (SPIKE-139's G3 notes).

**(d) Effect.**
- **Items 3 and 4 (and the hump of item 2, if it is a crest):** #184w's counterfactual (S0 smoothed along the valley) removes 50-92 % of KH's roughness. SPIKE-139's diffusion removes 38-60 % of the comb band. A coupled term should land between the two. Unmeasured.
- **Item 1:** partly, since the checkerboard is what the read turns into the H1 pattern.

**(e) Effort and risk.**
- **Effort:** design 1-2 lane-days, then implement and review 1-2. Calibration rows on the five windows take 2-3 rounds of about 5 h of lock each.
- **Total:** 4-7 lane-days and 10-15 h of lock.
- **Risk:**
  - open-ended calibration: no value of the spike's term removed the comb at no relief cost;
  - the channel filling must be shown gone (the post-K5 drain's line);
  - the relief and spectrum bars.

**(f) Rebaselines and post-port-able.**
- Rebaselines z_s, so H1, the trunks, S0, everything downstream and the flight stores. Not B.
- Post-port-able: yes by the owner's test (a K5 change, no RG code change). It re-routes H1, so the gate's 15 windows rerun.

## 3. K5's straight H1 trunks (O13)
**(a) Site and rule.**
- `route.rs:224-230` `receiver_of`: the steepest neighbour by slope x (1 + 0.5 u), with u per-cell white jitter in [-1, 1).
- K5's four rounds of five steps carve along these D8 receivers (`round.rs:70-85`), and K6 routes the carved z_s again by D8 at 814 m. A carved valley is a chain of straight D8 segments.
- Evidence:
  - S0's D8 sinuosity p50 is 1.117 against GLO-30's 1.251;
  - KG-trunk length in straight runs over 2 km is 35-52 % against GLO-30's 7.3 % (SPIKE-143);
  - F1 d135: 13 of 17 H1 channel cells on one diagonal (#184w).

**(b) Candidates.**
- **(i) Routing (Dinf, LTD):** these exist only as spike switches (`--k5-route ltd | dinf`), not on main.
  - SPIKE-139 measured them: the diagonal channel step share stays 60.1 -> 60.6 %, steps in runs of 8 or more 5.1 -> 5.2-5.4 %, the comb stays.
  - Cheap to rerun but no effect: the straightness is carved in by the stream-power feedback, not chosen by the routing rule.
- **(ii) Planform noise.**
  - A spatially coherent term in the receiver choice: a low-frequency, integer-hashed noise field (2-8 km) biasing the slope, in place of or beside the white jitter.
  - Or a coherent erodibility field, so valleys bend at km scale.
  - Cheap to build (a switch in `receiver_of`, 1-2 lane-days with review, then R1h and F1 rows, 2-4 h of lock).
  - Open-ended to calibrate against GLO-30's 1.25 and 7.3 % without new artefacts: dams where the bias fights the slope, a wiggle that reads as noise. Estimate 5-10 lane-days, unknown.

**(c) Why K5' failed, and why this differs.**
- K5' (SPIKE-131; r1h-k5k 8/18 bars against today's 14/18) changed what K5 routes on: sill edges of r plus the round's z_s.
- That moved the whole erosion and drainage: mean erosion 101 -> 78 m, kept H1 basins 69 -> 337, and 11.8 % of land draining to an outlet over 8 km away. It did not target the planform.
- Planform noise keeps K5's erosion law and the heights it routes on, and changes only which downhill neighbour wins.
- Its rebaseline is as wide as K5''s, because the trunks move. Its erosion side effects should be smaller, but that is unmeasured.

**(d) Effect.**
- **Item 5 and O13's sinuosity bar:** directly.
- **Item 1:** indirectly. The H1-locked pattern is strongest along diagonal trunks, and fewer straight diagonal runs mean less of it.
- **Items 3 and 4:** none (the washboard is fix 2's).

**(e) Rebaselines and post-port-able.**
- Rebaselines: as fix 2.
- Post-port-able: yes by the owner's test. RG's curve and meanders follow whatever H1 gives, though the curve's sigma and KH's corridor were tuned on straight trunks.

## 4. RG's own items (spike branch, rg_chain stages)

| item | finding (#184w) | candidate fix | effort |
|---|---|---|---|
| band6's entry breach | sets the curve floor through a 17 m hump to one level (674.1 m over 31 cells): 41 % of the dip's depth added after S0 | breach to a graded line between the hump's ends (the bed's own gradient), not a flat level | 0.5-1 lane-day; R1h and F1 rows, about 2 h of lock |
| band6's connect 4 | lowers a corner link's edge cell to the pair's mean on the floor line: 40 % of the dip | skip the curve's floor cells (the curve's raster is 4-connected already), or connect by the graded line | 0.5 lane-day, same rows |
| KV's F1 roughness | F1 d45: KV lowers 86 % of the floor cells and 53 % at 1.5-4 cells, 6,728 m.cells (more than KH's 5,644); floor roughness 0.91 -> 1.68 m. Not on R1h (0.7 % of KH's) | first a diagnosis: which KV rule (`--kv-wall-s0`, the walls against S0's slope) acts on F1's low relief (relief p50 55 m) | 0.5 day diagnosis, then 0.5-1 day fix; rows 2 h of lock |

- These are under the freeze: rows need the owner's release.

## Summary

| fix | effort | risk | effect on the flagged defects | post-port-able |
|---|---|---|---|---|
| 1. K7 read: C2 quasi-interpolant | about 2 lane-days, 5 h lock | low-medium: fills K5's one-cell channels (p99 5-34 m); relief -2.5 to -6.4 %; partial (X8 0.40-0.60 against 0.10) | item 1: H1-locked rms -58 to -63 %; items 2-5: none | yes |
| 2. K5 hillslope term (parity coupling) | 4-7 lane-days, 10-15 h lock | medium-high: open-ended calibration; channel filling; relief bars | items 3 and 4 mostly (comb band -38 to -60 % for the spike's term), item 1 partly | yes (re-routes H1: 15 windows rerun) |
| 3a. K5 routing (Dinf, LTD) | about 1 lane-day | low, but no effect measured (SPIKE-139) | none | yes |
| 3b. K5 planform noise | 1-2 lane-days to build plus 5-10 to calibrate; 2-4 h lock first, more later | high: unknown; dams, noise look | item 5 and O13; item 1 partly | yes (re-routes H1) |
| RG: band6 breach and connect 4 | 1-1.5 lane-days, 2 h lock | low | item 2: up to 81 % of the dip's added depth | (RG's own: in G3) |
| RG: KV on F1 | 1-1.5 lane-days, 2 h lock | low-medium (diagnosis first) | F1 d45's KV roughness | (RG's own: in G3) |

- **Reading for B against C:**
  - Fix 1 is small and safe enough for G3, but partial.
  - Fix 2 is a model change with open-ended calibration.
  - Fix 3's cheap form (routing) has no effect; its effective form (planform noise) is open-ended.
- So by the lead's rule: B for fix 1 if the owner wants the partial gain now, and C (after the port) for fixes 2 and 3. RG's own items go in G3 either way.
