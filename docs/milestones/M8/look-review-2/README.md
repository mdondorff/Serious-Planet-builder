# Look review 2 (M8-6 follow-up): the rebuilt store, default look and a sample driver look

Candidate stills for the owner's comparison with `look-review-1/` and `look-review-1-hills/`. Not goldens.

## Setup
| Item | Value |
|---|---|
| Store | `target/accept/global16384` (16384 cells per face edge), rebuilt with the input-plus-fill coupled run (#681) |
| Base commit | `9b646004` (main after #722: local routing, driver maps, chunk fade) |
| Chunk stage | A-net, 8 m cells, local main-stem routing (ADR 0081), driver maps (ADR 0076); every tile of every image has its chunk landed |
| Image | 2560x1440, lit path with atmosphere, adaptive exposure, no horizon shadows beyond the stills tool's maps, no water, trees or ground cover |
| Poses | those of review 1 (300 m, 30 m, 1.8 m; sunset 17:30 and midday), same eye offsets, pitches and headings |

## The sites moved
The rebuilt store has a new coupled run, so the same anchor rules land elsewhere.

| Site | Rule | Review 1 | Review 2 |
|---|---|---|---|
| River (`look-review-2/`) | `largest-river`, `min_height_m` 150 | lat -11.17, lon 23.05, ground 279 m | lat -16.48, lon 33.07, ground 118.5 m, flows to bearing 80 |
| Hills (`look-review-2-hills/`) | `steepest-river`, snap to the channel within 1200 m | lat -37.70, lon 3.14, ground 2907 m, 2.9 degrees | lat 34.16, lon 140.84 (snapped), ground 3173 m, steepest slope 40.8 degrees, relief 852 m, downstream bearing 130 |

The two sites are different ground from review 1; the comparison is of the same rules and poses, not of the same spot.

## Looks
| Directory | Look | Command |
|---|---|---|
| `default/`, `../look-review-2-hills/default/` | the default look (ground detail 0, `MaterialParams::EARTH`: rock on steep slope, sand and grass by precipitation, soil off) | `cargo xtask capture stills --poses <dir>/poses.toml --store target/accept/global16384 --out-dir <dir>` |
| `sample-driver-look/`, `../look-review-2-hills/sample-driver-look/` | sample driver look: ground detail 0.6 and the curves in its `poses.toml` `[look]` table | same |
| `views/` | J views at 300 m midday: `soil`, `moisture`, `insolation` (the debug views of the driver maps) | same |

The sample driver look is a proposal, not a decision. Its curves (see `sample-driver-look/poses.toml`):
- soil: moisture 0.55 to 0.85 rising, soil depth 0.5 to 1.2 m rising, slope cosine 0.7 to 0.9 rising (soil in wet, deep, flat hollows);
- rock: moisture 0.8 to 0.3 falling, soil depth 1.6 to 0.8 m falling, on top of its slope curve (bare rock on dry, thin, steep flanks);
- the neutral driver values (1 m, moisture 0.5) give soil no weight and rock a reduced one, so ground without a chunk keeps the default look.

Each directory has `poses.toml` (the exact poses and look), `stills.txt`, `capped.txt` and `sections.txt`.

## First look (coder's read, not a review)
- River site (arid plain, 118 m): the sample look is slightly greener-olive with fine streaking from the detail at 30 m and 1.8 m; the default is smooth. The canyon walls read the same in both.
- Hills site (3.2 km, snow-covered): the snow albedo dominates, so the two looks are nearly indistinguishable at 300 m. The curves do not show here; a snow-free steep site would be needed to judge them.
- The driver views at the river site show smooth, low-contrast fields at 300 m (a plain; the chunk drivers fade to the neutral values beyond the chunk).
- The 300 m default view shows hard-edged tile-sized steps in the canyon rim (the 8 m chunk against the base field), the same limit as review 1.
