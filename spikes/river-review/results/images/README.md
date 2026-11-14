# River review: picture set (the owner's external review of ADR 0123)

Made by spikes for lead order #118 (2026-11-14). The numbers for the same windows and variants go into crusader's `../results.md` and CSV (lead order #138 to crusader, a separate PR).
Scripts that made the pictures: `tools/` (Python; they read the probe's dumps in `target/river-review/` and the stills' output, not committed).

## Variants (the same five on every sheet)
| Label | Surface |
|---|---|
| today | `B + regional`: B's lattice (611 m) plus the 814 m regional solve, read at L |
| one round at L only | today plus one round of the regional kernel at L (SPIKE-115/117: steps 5, re-route every 5, default round rules) |
| 4-neighbour drain only | SPIKE-116's hand-over: the 814 m surface breached over 4 neighbours, bilinear at L, B's residual damped near channels over 400 m |
| 4-neighbour drain + one round at L | the hand-over, then the round above (ADR 0123 as drafted) |
| Schott 2024 fair | the hand-over, then SPIKE-118's per-scale pipeline (407 / 204 / 101.8 m, 2 rounds a scale, retargeting, multi-scale breach) |

All five are **start surfaces from the `drain_base` probe (spike code), not the production path and not baked**. Branch `spike/schott-fair` (f15b0e1d), switch `PLANET_PICS`.

## Windows (canonical names, lead #119)
| Name | Face, window, seed | Shown |
|---|---|---|
| R1h | face 1, i 8056-8675, j 760-1379, seed 14: the E1 region (stores r3a-e1, r3a-p2, r3a-p2e) | the 620-cell core |
| R3a | face 4, SPIKE-111 window i 168-1187, j 4520-5539, seed 15 | its 620-cell core i 368-987, j 4720-5339 |
| R3b | face 2, SPIKE-111 window i 9768-10787, j 8104-9123, seed 15 | its 620-cell core i 9968-10587, j 8304-8923 |

## Top-down sheets: `r1h-topdown.png`, `r3a-topdown.png`, `r3b-topdown.png`
- Columns: the five variants, then GLO-30 N40 W078 (Appalachians) at about 94 m.
- Rows: a 40 km crop (393 L cells) and a 10 km crop (98 cells), the 10 km crop marked by a red box in the 40 km one.
- Crops: picked on today's surface for relief and river cells, then the same cells for every variant; the crop's core cell origin is in each sheet's header.
- Hillshade, light from the upper left at 45 degrees; the face's v axis up (north on faces 1 and 2; R3a lies on face 4, the polar face). GLO-30 north up.
- Vertical exaggeration per window, the same for all five variants (R1h x1, R3a x5, R3b x2); GLO-30 x1. Stated in every panel title.
- Rivers: blue, the L route's cells with drained area at least 75 km2 (`HydroParams::default().initiation_m2`). GLO-30: the same rule on its crop (priority flood, D8).
- Scale bars: 5 km on the 40 km crops, 2 km on the 10 km crops (drawn in the panel, lower left).

## Obliques: `oblique-f43680.jpg`, `oblique-f33547.jpg`
- Poses: the owner's F9 captures, both in the E1 region (R1h window), in its margin outside the 620-cell core; low sun (17.5 h local, declination 0). Pose files: `poses-f43680.toml` (from SPIKE-113) and `poses-f33547.toml` (from the last sample of `frame-33547-path.txt`: eye 102 m above ground, heading 38.96, 4.78 degrees down; the anchor 1,200 m ahead where the view ray meets the ground).

| Panel | Path | Build |
|---|---|---|
| store r3a-e1 | **production path**: `planet test-render --scene capture-stills --adapter hardware`, RTX 3080 Ti, 1920 x 1080 scaled to 960 x 540 | main 4b4dd86a, before the GEN-173 cap (kernel 7/9) |
| store r3a-p2e | **production path**, same command; SPIKE-115's spike bake (one round at L, rivers routed at L; no channel graph stored, so no river water is drawn) | `spike/erosion-at-l` 1000977a, the branch that baked it (main's reader refuses its tile keys) |
| the five variants | start surface, no bake: the probe's whole-window dump at 101.8 m, ray-marched in Python with the capture-stills camera (fov 60 degrees), bilinear heights, Lambert plus sky light plus cast shadows, no materials, no sub-cell detail; river cells blue | `spike/schott-fair` f15b0e1d |

- Scale: the production stills carry the renderer's scale bar (500 m and 200 m); the start-surface renders have none (same camera and field of view).
- The rivers differ in kind: the production stills draw K7's burned H1 lines; the start-surface renders show the L route's cells.

## What the sheets show
- The 4-neighbour variants (drain only, drain + round) show the 814 m grid as bilinear blocks at 10 km and rivers in axis-aligned steps (SPIKE-117: 66-69 % of channel steps along a grid axis on R1h and R3b); one round at L alone keeps today's directions (43-47 %).
- Schott fair carves deeper valleys on R1h's hills and keeps part of the axis steps (46-57 %).
- R1h and R3b show a long straight step or ridge in every variant: it is in the start surface (B + regional), not made by a variant.
- frame-43680: r3a-e1 shows the box trench of the canal-like river (F3); r3a-p2e has none (no river is drawn there).
- frame-33547: r3a-e1 shows the gorge's stepped dark wall (F2); r3a-p2e has a smooth valley floor.

Produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus 2014-2018, provided under COPERNICUS by the European Union and ESA; all rights reserved.
