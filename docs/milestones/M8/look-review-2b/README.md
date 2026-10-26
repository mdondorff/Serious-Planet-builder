# Look review 2b: review 1's exact coordinates, and a snow-free hilly site

Follow-up to `look-review-2/` (planner's request, 2026-10-26). Same poses, same two looks, same store and run (`target/accept/global16384`, input-plus-fill coupled run #681), base commit as `look-review-2/README.md`. Not goldens.

## Sites
| Directory | Place | Heading | Ground on the rebuilt store | Why |
|---|---|---|---|---|
| `river-r1/` | lat -11.1657, lon 23.0493 (look review 1's river anchor) | 131 | 286 m (279 m in review 1) | the side-by-side with `look-review-1/` over the same ground |
| `hills-r1/` | lat -37.6958, lon 3.1397 (look review 1's hills anchor, after the snap) | 145 (its downstream bearing) | 3,810 m (2,907 m in review 1) | the side-by-side with `look-review-1-hills/`; the rebuilt run's ground there is 900 m higher and snow-covered |
| `hills-b/` | lat 8.972, lon -16.238 (hills B) | 90 | 3,755 m | snow-free, rolling hills with a river valley and a cut channel in the near field |

Hills A (lat 37.72, lon 134.03) was scouted and dropped: 4,725 m, snow-covered at 500 m eye height in all four headings. Hills B at the same test is green hill country with a river valley, which `hills-b/` shows.

Place by latitude and longitude (`[place] lat, lon`): the anchor is the ground there, not a river cell, so the poses stand over the given point and look along the heading. The river at `river-r1` is the one the review 1 coordinates lay on; the rebuilt run's channel may not pass exactly through the point.

## Looks
Each site has `default/` and `sample-driver-look/` with its `poses.toml` (the sample look is the one of `look-review-2/README.md`: ground detail 0.6 and the soil and rock curves; a proposal).

Command (per directory): `cargo xtask capture stills --poses docs/milestones/M8/look-review-2b/<site>/<look>/poses.toml --store target/accept/global16384 --out-dir docs/milestones/M8/look-review-2b/<site>/<look>`

## First look (coder's read, not a review)
- `hills-b/`: the sample look turns the default's olive-brown grass to a fresher green on the lit slopes (the moisture curve at work); relief and valley are the same in both. This is the site where the curves show.
- `hills-r1/`: snow again (3.8 km); the looks are indistinguishable.
- `river-r1/`: the 300 m views show the incised channel floor as flat stepped benches, a few hundred metres long, with a dark grid of fine ridges across them (`300m-sunset.png`); the channel walls are sharp. The steps and the grid look like an artefact of the channel bed, not terrain. Flagged to the planner.
