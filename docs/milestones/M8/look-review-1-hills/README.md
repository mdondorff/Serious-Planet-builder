# Look review 1, second site (M8-3): the same poses over a river in hilly ground

Second site of the owner's review of the base layer's landforms (`docs/milestones/M8/look-review-1/` is the first, a plain). Not goldens. Calibration of incision depth and hillslope diffusion waits for the owner's read of both sites.

## Setup
| Item | Value |
|---|---|
| World, chunk stage, image, not drawn | as `look-review-1/README.md` (store `target/accept/global16384`, A-net 8 m chunks, 2560x1440, lit path, no horizon shadows, no water, trees or ground cover) |
| Anchor rule | the land river cell at least 150 m high with the largest drainage area among the cells whose surroundings within 20 km span 1000 to 2500 m of coupled-run height (`relief_radius_m = 20000`, `relief_min_m = 1000`, `relief_max_m = 2500`; a query over the coupled run before any chunk is drawn, `local_relief_m` in `crates/app/src/stills.rs`) |
| Rule history | within 10 km (about two coupled cells of 4.9 km) the 1000 m range found no cell; 800 m found one at latitude 12.3148, longitude -13.1177; the radius was widened to 20 km to keep the 1000 m floor |
| Command | `cargo xtask capture stills --poses docs/milestones/M8/look-review-1-hills/poses.toml --store target/accept/global16384 --out-dir docs/milestones/M8/look-review-1-hills` |
| Index | `stills.txt`; every chunk landed (6 chunks per pose) |

## Poses
Anchor: latitude 11.6652, longitude -13.2495 (ground 680.7 m); the river flows towards bearing 267 degrees there and all poses look downstream (heading 267). Same six poses as look review 1: eye 300 m (1500 m behind, pitch 15), 30 m (400 m, 8) and 1.8 m (60 m, 0) over the ground; sun 17:30 and 12:00. Files `300m-*.png`, `30m-*.png`, `1.8m-*.png`, each `-sunset` and `-midday`.

## What to look for
As look review 1, and in addition: whether the ground reads as hills at all, how the river sits against slopes, slope and diffusion at the valley sides.

## First look (coder's read, not a review)
- The site does not read as hilly. At 300 m and 30 m the ground is a smooth plain to the horizon with no visible slope; the 20 km relief of the coupled run is a spread over several cells and does not show as relief at the eye.
- The only incision is a thin line of pale spikes (the channel) that runs from the left edge and ends abruptly near the middle of the image, in `300m-midday.png` and `30m-midday.png`; beyond its end the plain shows no channel. Whether that end is the chunk coverage or the channel is unknown.
- `1.8m-sunset.png` shows low rounded hills on the horizon (some kilometres away) in shadow, the only landform of the set; the foreground is flat.
- Against look review 1 this site shows less, not more: the first site's walls are the larger landforms. Cell size matters: the coupled run's cell is 4.9 km, so relief inside 10 km is two cells.
