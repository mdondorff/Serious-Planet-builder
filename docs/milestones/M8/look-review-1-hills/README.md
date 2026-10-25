# Look review 1, second site (M8-3): the same poses over the steepest river

Second site of the owner's review of the base layer's landforms (`docs/milestones/M8/look-review-1/` is the first, a plain). Not goldens. Calibration of incision depth and hillslope diffusion waits for the owner's read of both sites.

## Setup
| Item | Value |
|---|---|
| World, chunk stage, image, not drawn | as `look-review-1/README.md` (store `target/accept/global16384`, A-net 8 m chunks, 2560x1440, lit path, no horizon shadows, no water, trees or ground cover) |
| Anchor rule | the land river cell of the coupled run with a drainage area of at least 1e10 m2 whose surroundings within 2 km have the steepest slope on the base field (the 16384 bake, a grid of its 611 m cell spacing, the steepest rise over run of two samples), anywhere on the planet |
| Measured at the anchor cell | latitude -37.6853, longitude 3.1421, ground 3243 m, steepest slope 2.9 degrees, relief 126 m within 2 km (`stills.txt`, first line) |
| Snap | the chunk stage cuts its channel 1186 m from the anchor cell (a coupled cell is 4.9 km): `snap_to_channel_m = 1200` moves the anchor onto the lowest chunk ground within 1200 m (latitude -37.6958, longitude 3.1397, ground 2907 m) and reads every heading from the channel's downstream bearing (145 degrees; the coupled run's is 180) |
| Rule history | first rule: `largest-river` with a 20 km relief span of 1000 to 2500 m (latitude 11.6652, longitude -13.2495), a plain; second: steepest slope in a 1000 to 2500 m ground band (latitude 10.4580, longitude -19.0942, slope 2.7 degrees, relief 78 m), a plain with a slot canyon; replaced by the band-free rule on opus-planner's decision |
| Command | `cargo xtask capture stills --poses docs/milestones/M8/look-review-1-hills/poses.toml --store target/accept/global16384 --out-dir docs/milestones/M8/look-review-1-hills` |
| Files | `stills.txt` (anchor, snap, eye, chunks), `sections.txt` (cross-sections of the chunk heights across the channel), `slopes.txt` (the table below) |

## Slope of the base field by ground band
`slopes.txt`, made with `slope_table_stride = 8` (every 8th cell of the 2048-cell coupled lattice over the sea for the land columns, every river cell of at least 1e10 m2 for the river columns; 456 s). Slope: the steepest rise over run within 2 km of the cell's centre on the base field, in degrees. Shares are of the land samples (or of the river cells) of the whole planet.

| Ground band (m) | Land share (%) | Land cells | Land p50 | Land p90 | Land p99 | River share (%) | River cells | River p50 | River p90 | River p99 |
|---|---|---|---|---|---|---|---|---|---|---|
| 0-500 | 60.8 | 62877 | 0.83 | 1.27 | 1.65 | 65.8 | 125516 | 0.82 | 1.26 | 1.64 |
| 500-1000 | 28.7 | 29672 | 1.05 | 1.41 | 1.77 | 27.4 | 52169 | 1.04 | 1.40 | 1.76 |
| 1000-2500 | 8.6 | 8889 | 1.13 | 1.56 | 1.98 | 6.0 | 11423 | 1.11 | 1.53 | 1.97 |
| 2500-5000 | 1.8 | 1869 | 1.33 | 1.86 | 2.40 | 0.8 | 1542 | 1.24 | 1.72 | 2.25 |
| over 5000 | 0.0 | 45 | 1.30 | 1.77 | 2.43 | 0.0 | 5 | 1.72 | 2.15 | 2.15 |

Reading: the base field is smooth at 611 m everywhere: the 99th percentile of the 2 km slope is 2.4 degrees or less in every band, the steepest river cell is 2.9 degrees, and 89.5 % of the land is under 1000 m. Slope grows slowly with ground height. The 611 m sample spacing cannot see slopes between samples (a mountain flank steeper over 100 m shows as its average over 611 m).

## Poses
Same six poses as look review 1 (eye 300 m, 1500 m back, pitch 15; 30 m, 400 m, 8; 1.8 m, 60 m, 0; sun 17:30 and 12:00), looking downstream (bearing 145). The 300 m eye stands 3572 m high, the 1.8 m eye 2915 m (`stills.txt`).

## Checks asked for
- (a) Channel against its banks (`sections.txt`: a section every 100 m for 2 km downstream, 8 m apart over 2 km across): the channel is a cut 292 to 302 m under the median ground (the chunk stage's 300 m bound), and 96 to 792 m wide measured at 20 m under the median (the widths over 700 m are the channel crossed obliquely at its bends). The highest point over the median is 4 to 19 m, mostly at the window ends (the plain's own slope), so no spike over the bank line of 20 m or more exists in the heights at the 8 m cell. The images show jagged spikes on the canyon walls (`30m-midday.png`, `30m-sunset.png`, `1.8m-midday.png`: sheets of stretched triangles along the right wall, spikes at the foot of the walls). They are not in the 8 m heights: they are finer than 8 m, or made by the mesh at near-vertical walls. Not investigated; this is the open finding of this review.
- (b) Anchor and channel: the channel lies 1186 m from the anchor cell (at the first site there was none within 2.6 km in the landed chunks). Along the course the lowest point stays within 120 m of the heading for 900 m, then drifts 256 to 1000 m to the right (the window ends at 1000 m) for the rest of the 2 km: the channel bends away from the straight view axis after about 900 m. The snap fixes the anchor, not the later bends.

## What to look for
As look review 1; in addition the depth against the width of the cut, the wall shape, the spikes on the walls, and the plain around the canyon (at 3200 m, snow-white in the lit view).

## First look (coder's read, not a review)
- A slot canyon about 300 m deep with vertical walls and a sharp rim, winding across the view at 300 m; the plain around it is rough (small ridges and hollows, visible in `300m-sunset.png`) and not hilly.
- Uniform depth along the course and vertical walls: no talus, no valley sides.
- The walls carry stretched triangle spikes in the near views (`30m-midday.png`, `1.8m-midday.png`): a rendering artifact, not terrain.
- The 1.8 m eye stands on the canyon floor in a flat white strip, walls filling the frame.
