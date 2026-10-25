# Look review 1, second site (M8-3): the same poses over a river in hilly ground

Second site of the owner's review of the base layer's landforms (`docs/milestones/M8/look-review-1/` is the first, a plain). Not goldens. Calibration of incision depth and hillslope diffusion waits for the owner's read of both sites.

## Setup
| Item | Value |
|---|---|
| World, chunk stage, image, not drawn | as `look-review-1/README.md` (store `target/accept/global16384`, A-net 8 m chunks, 2560x1440, lit path, no horizon shadows, no water, trees or ground cover) |
| Anchor rule | the land river cell of the coupled run with a drainage area of at least 1e10 m2 (10,000 km2), ground 1000 to 2500 m high on the base field (the 16384 bake), whose surroundings within 2 km have the steepest slope on the base field (a grid of the bake's cell spacing, about 610 m; the steepest rise over run of two samples) |
| Measured at the anchor cell | ground 1447 m; steepest slope 2.7 degrees; relief 78 m within 2 km |
| What the rule can find | no river cell in that ground band is steeper than 3.1 degrees over 2 km on the base field (with 1e9 m2 the steepest is 3.1 degrees, relief 119 m, at latitude -38.4987, longitude 2.1313): the base field has no steep ground there; the relief that shows in the stills is the chunk stage's channel |
| Snap | the chunk stage cuts its channel 642 m from the anchor cell (a coupled cell is 4.9 km). `snap_to_channel_m = 1200` moves the anchor onto the lowest chunk ground within 1200 m once the chunks under the first pose have landed (ground 1136 m, latitude 10.4575, longitude -19.1001) and reads every heading from the channel's downstream bearing (250 degrees; the coupled run's bearing is 266) |
| Rule history | the first site rule (`largest-river` with a 20 km relief span of 1000 to 2500 m, anchor latitude 11.6652, longitude -13.2495) found a plain: no landform within view and the channel far from the view axis; replaced |
| Command | `cargo xtask capture stills --poses docs/milestones/M8/look-review-1-hills/poses.toml --store target/accept/global16384 --out-dir docs/milestones/M8/look-review-1-hills` |
| Index | `stills.txt` (eye, chunks), `sections.txt` (cross-sections of the chunk heights across the channel) |

## Poses
Same six poses as look review 1 (eye 300 m, 1500 m back, pitch 15; 30 m, 400 m, 8; 1.8 m, 60 m, 0; sun 17:30 and 12:00), looking downstream (bearing 250). The 30 m eye is 30 m over the rim ground (1439 m), the 1.8 m eye stands on the channel floor (1140 m).

## Checks asked for
- (a) Channel against its banks: `sections.txt` has a section every 100 m for 2 km downstream, 8 m apart over 2 km across. The channel is a cut about 300 m deep (median ground about 1440 m, floor 1136 to 1150 m) and 130 to 300 m wide (the width more than 20 m under the median); the floor is 1136 m at the anchor and 1144 to 1150 m further down (it rises along the course, not monotonic). The depth is the chunk stage's 300 m bound (`bound_m`) every time. Over the median the sections show at most 3 m of rise beyond 1 km and 5 to 9 m near the anchor (the plain's slope at the section ends, 1 km out), so the heights hold no spike above the bank line at the 8 m cell. The bright thin spikes along the left rim in `300m-midday.png` and the spiky foot of the right wall in `1.8m-midday.png` are not in the heights at that resolution: they are finer than 8 m or made by the mesh (not investigated).
- (b) Anchor and channel: at the first site the lowest chunk ground within 2600 m of the anchor was 15 m under the median (no channel in the landed chunks), so the poses looked past the channel; at this site the channel is 642 m from the anchor cell. The channel does not run along the line from the anchor to the next cell: the section offsets of the lowest point stay within 40 m for 700 m, then drift to 700 to 900 m right over the next 800 m and return. The view axis follows only the first stretch; the snap fixes the anchor, not the later bends.

## What to look for
As look review 1; in addition the depth against the width of the cut and the wall shape (`30m-sunset.png`).

## First look (coder's read, not a review)
- A slot canyon about 300 m deep and 150 to 300 m wide cut into a flat plain, with vertical walls and a sharp rim; at 300 m the channel winds across the whole view.
- The walls are vertical, not eroded valley sides: no talus, no slope break at the rim, a uniform depth along the course (the 300 m bound).
- The plain is flat to the horizon (relief 78 m within 2 km): the site is hilly in ground height (1447 m) but not in shape.
- At 1.8 m the floor is a green strip a few metres wide between walls that fill the frame.
