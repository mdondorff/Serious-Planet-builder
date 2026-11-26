# Step 1's flagged look defects: source stages (lead #184w, 2026-11-25, spikes)

Status: lock-free attribution on the existing dumps (r1h-kw, r1h-def, f1-kw, f1-def; chain 2e4a39a7). No new runs, no switch rows, no fixes.
- Images: `tmp/g1f6/spikes/s184/attr-184w/item1..5-*.png`; raw tables `q3-*.txt`; scripts `attr-184w/scripts/` (q1-q5, figs).

## Method
- KH's curve zone was rebuilt from `r1h-kw-trunk-curves.bin` with rg_chain's rule (distance to the segment less 0.71, weight 1 - smoothstep(d / 8)).
  - The bed z_b is taken from the curve's own floor cells (d = 0, where S_KH = target).
  - S_KH is recomposed as S0 - w max(0, S0 - target). It matches S_KH to 1.2 m rms in the zone (d135). The rest is R, the C1 read mismatch: KH reads C1(z_s) - C1(z_b), S0 reads C1(z_s - z_b), and the reads are clamped. Off the zone R is 0.25 m rms.
- **Along-valley residual:** a field minus its mean along the curve at a fixed signed distance (±8 cells of arc). This removes the cross-profile, so a straight valley reads 0 at any angle.
- **H1-locked part:** the residual's means over the 64 phases (x mod 8, y mod 8) of the global L cell.
- **Counterfactual:** one term of KH's formula is smoothed along the valley; the share of the residual energy that goes away is that term's.
- Bands by d: floor < 1.5 cells, edge 1.5-4, edge 4-8.

## The table

| defect | source stage | share (measure) | mechanism |
|---|---|---|---|
| 1. R1h d135, S_KH's residual chainsaw | **S0** (K5's channel, read by C1) | 88-93 % of S_KH's H1-locked energy (S0 smoothed along the valley); S_KH's pattern correlates with S0's at +0.60 / +0.86 / +0.99 (floor / 1.5-4 / 4-8) | S0 already holds it along the trunk: locked rms 4.4 / 2.9 / 2.6 m, against S_KH's 1.1 / 2.1 / 2.1 m. Not area-wide: S0's high-pass on 1024^2 has no H1 lock (share 0.000; off the zone 0.87 m) |
| | KH, the weight's edge | carries it: (1 - w) S0, mean w 0.71 at 1.5-4 and 0.20 at 4-8 cells | the edge blends S0's pattern in |
| | KH, the lower-only floor | 31 % of floor cells uncut (S0 at or under the target); their locked rms 2.4 m against 1.3 on cut cells; a blend that also raises would remove 75 / 60 / 22 % of the roughness | min(S0, target) on the floor lets S0's dips through |
| | KH, the target's slope term | 15 % of the floor's roughness, none of the locked energy (-1 %) | the target uses \|grad S0\| per cell |
| | KH, the bed | 8 % floor roughness, 11 % floor locked | |
| | R (C1 read mismatch) | locked rms 0.04-0.12 m | negligible |
| 2. R1h d135, line dip at the lower left (31 cells, 17 x 16 px) | **S0** makes it; **band6** amplifies it | depth against its neighbours 2-4 cells across, per stage: S0 -6.85, S_KH -7.52, KS0 -8.02, KS1+C -8.02, band6 breach -10.48, band6 connect 4 -12.87, KV -12.75, final -12.75 m. Of the 6.0 m added after S0: KH 11 %, KS0 8 %, breach 41 %, connect 4 40 % | the line is the curve's floor (signed distance 0-1.4 cells). S0 has a 17 m hump on it (673.7 -> 691.3 -> 678 m); band6 sets the 31 cells to one level, 674.1 m. The defaults show the same: S0 -4.6, after band6 -15.0 m, breach and connect 4 about half each |
| 3. R1h river, chaotic riverside bed | **S0** (the crossing ridges), passed by **KH**'s edge and lower-only floor | S_KH - S0's volume: floor 56 %, edge 43 %, R 2 %. Roughness removed: S0 smoothed 50 / 78 / 90 %; no lower-only 32 / 48 / 11 %; target slope 15 % (floor); bed 19 % (floor). Uncut cells 27 / 58 / 40 % | S0's ridges cross the valley (stripes in the residual map); the cut is a patchwork of cut and uncut cells |
| | KH, the meanders | the straightening (sigma 6 cells) moves 31 % of \|S_KH - S0\| (14.0k m.cells lowered, 20.4k raised): an upper bound, it also straightens the base path's H1 bends | the meander moves the cut; it is not a roughness source that can be measured here |
| | KV's walls | KV - band6: 819 m.cells (0.7 % of KH's); roughness 10.18 -> 10.22 m on the floor | not a source here |
| 4. F1 d45 and river, blocky beds in S_KH | **S0's washboard W** ((S0 - B) minus its 6-cell Gaussian), cut by **KH** | the lowering's along-valley variation follows W: corr +0.97 (d45), +0.96 (river) on the floor. S_KH's walls keep W: R2 0.78 / 0.91 (d45), 0.56 / 0.80 (river). S0 smoothed removes 87-92 % (d45), 52-92 % (river) | S_KH - S0 is a chain of pits, one per W crest crossing the trunk; the troughs already lie under the target and stay uncut (d45 floor 22 % uncut, edge 51-74 %) |
| | orientation | F1's W crests peak at 137.5 (d45 patch), 132.5 (river) and 137.5 deg (d135; map convention, counter-clockwise from east). The crest lines run NW-SE, so the wave runs SW-NE, as the owner sees. In the zones: 138 deg (d45), 144 deg (river). Wavelength 21-30 cells | weakly concentrated: concentration 0.06-0.09, the peak 5-deg bin holds 7-9 % of the 10-30 cell power (uniform 2.8 %). R1h's 1024^2 patches peak at 37.5 / 77.5 deg; the earlier 124-135 deg was S0's high-pass in crops |
| | **KV** (F1 d45 only) | KV - band6: 6,728 m.cells, more than KH's 5,644. Floor roughness 0.91 -> 1.68 m, edge 1.5-4 2.63 -> 3.49 m; KV lowers 86 % of the floor cells | a second source after KH in F1 d45 (visible at KV and final, not in S_KH) |
| 5. F1 d135, straight diagonal bed | **S0** (K5's H1 trunk, read by C1): **confirmed** | S0's lowest line lies at 45.0 deg (dv/du -0.006), offset rms 0.53 cells, depth 48 m. B has none (1.8 m deep; its lowest line wanders 8.5 cells rms). S_KH: 61 m, rms 0.62 cells; final 0.55 cells | 13 of the crop's 17 H1 channel cells lie on one H1 diagonal: K5's straight D8 run. The trunk curve runs along it |

## Reading for G3's decision
- **From S0** (K5's network and its C1 read): items 1, 3, 4 and 5. KH carries it into the bed through two parts of its formula:
  - the weight's edge, which blends (1 - w) S0;
  - the lower-only floor, min(S0, target), which leaves S0 wherever S0 lies under the target.
- **From RG's own stages:**
  - band6's breach and connect 4: item 2's dip, 81 % of the depth added after S0;
  - KV in F1 d45: blockier than S_KH there.
- **What KH's target and bed add:** 8-19 % of the floor's roughness, no H1 lock.
- **Not measured:** whether a smoother S0 (a K5 or C1 change) would leave KH's patchwork. The counterfactual says the patchwork's along-valley variation is S0's: smoothing S0 removes 50-93 % of it.
