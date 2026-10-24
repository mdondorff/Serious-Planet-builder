# Look review 1 (M8-4): engine renders of landforms with the A-net chunk stage

Owner's review of the base layer's landforms at the fixed poses of `docs/briefings/m8-plan.md` (exit criterion 1). Not goldens.

## Setup
| Item | Value |
|---|---|
| World | default eroded world, store `target/accept/global16384` (16384 cells per face edge), coupled run 2048 cells, 100 steps (GEN-056) |
| Chunk stage | A-net, 8 m cells, top level 15; every tile of every image has its chunk landed (no stand-in) |
| Image | 2560x1440, lit path with atmosphere, adaptive exposure, 60 degree vertical field of view |
| Not drawn | horizon shadows (M8-5), water in the river channels, trees, ground cover |
| Command | `cargo xtask capture stills --poses docs/milestones/M8/look-review-1/poses.toml --store target/accept/global16384 --out-dir docs/milestones/M8/look-review-1` |
| Index | `stills.txt` (eye position and height, chunks read) |

## Poses
Anchor: the largest river of the coupled run at least 150 m high, latitude -11.1657, longitude 23.0493 (ground 279 m); the river flows towards bearing 131 degrees there. All poses look downstream (heading 131); the eye stands back from the anchor against the heading.

| File | Eye over the ground | Behind the anchor | Pitch down | Sun (local solar time) |
|---|---|---|---|---|
| `300m-sunset.png`, `300m-midday.png` | 300 m | 1500 m | 15 degrees | 17:30 (7.5 degrees over the horizon), 12:00 |
| `30m-sunset.png`, `30m-midday.png` | 30 m | 400 m | 8 degrees | 17:30, 12:00 |
| `1.8m-sunset.png`, `1.8m-midday.png` | 1.8 m | 60 m | 0 | 17:30, 12:00 |

The 1.8 m eye stands about 100 m below the rim, in the incised channel (eye height 183 m against 279 m at the anchor).

## What to look for
- River incision: depth, width and wall shape of the channel; whether the walls read as eroded valley sides or as cut-out trenches.
- Knickpoints at the pinned corridor cells every about 1.2 km: a slope break along the channel (`design.md` section 5 of `m8-anet-chunk`: if visible, smooth the pin over a few cells, do not drop it).
- Slopes: the angle of repose (0.9) and diffusion at the rims and walls.
- The plain beyond about 4 km: the base field only, no chunk detail; whether its smoothness against the incised ground is acceptable.
- The step where chunk detail enters (level 15) as a texture or shading change on the walls.

## First look (coder's read of the images, not a review)
- The channels are narrow canyons with steep pale walls and a sharp rim; in `300m-sunset.png` the walls are bright cream against dark green plain.
- The plain around them is smooth with little relief at 300 m.
- At 1.8 m the walls fill the frame and show streaky ridges running down the slope.
- No knickpoint was looked for in particular; the 300 m views show the channel for about 2 km.
