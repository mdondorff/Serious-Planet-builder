# Horizon shadows on the look-review poses (M8-5 step 6)

Inspection images of the shadow engine, not goldens. The terrain under them will change (relief work in M8); these judge the shadows only.

## Setup
| Item | Value |
|---|---|
| Sites | this directory: `look-review-1/poses.toml` (lowland river); `../horizon-shadows-hills/`: `look-review-1-hills/poses.toml` (steepest river, snapped to the channel) |
| Store | `C:\DEV\SeriousPlanetData\global16384` (16384 cells, coupled run 2048 cells), coarse horizon made with `planet generate --coarse-horizon <store> --global-res 16384 --threads 8`: 6 records in 7.7 s (512 observer cells per face edge, 16 azimuths, 50 km) |
| Commands | `cargo xtask capture stills --poses <poses.toml> --store <store> --out-dir <dir>` |
| Chunk horizon | 8 m texels (A-net chunks), range 4.8 km, 16 azimuths, baked as the second job per chunk; a still waits for the maps of the chunks within 12 km of the eye |
| Wait per pose | 4 to 5 maps; first pose 9.6 s (lowland) and 30.3 s (hills) including the bake, later poses 0.0 to 0.3 s (maps already baked) |

## Reading (looked at: lowland 300m-sunset, hills 300m-sunset, 1.8m-sunset, 30m-midday)
| Check | Result |
|---|---|
| Shadows follow the terrain | yes: canyons are in shadow at sunset with lit rims and sun-facing walls; a gorge of the hills site is dark at the 1.8 m eye (sun 17:30) |
| Soft edge | present; no flicker visible in still frames |
| Scalloped shadow edge | **seen** along canyon rims (hills 300m-sunset, lowland 300m-sunset left bank): the shadow boundary is a row of scallops (spacing not measured), probably the fans of 16 azimuths near an occluder (design.md question 3; not tested with 32); 32 azimuths or the two-component fit of SPIKE-32 would smooth it (storage and bake time double) |
| Coarse outline at sunset (20 km shadows) | **not seen**: no far-field shadow band is visible in the sunset poses (probably the terrain within 50 km is too flat for the sun at 17:30; not verified); the check needs a site with a ridge 10 to 30 km to the west of the eye |
| Face-edge chunk bias (up to 1.41x march length) | **not checked**: both sites lie on face 0 (the hills site about 17 % of the face half-width from its edge, bias about 1.3x there); a bias of a few degrees of horizon angle cannot be told from the scallops in these images |
| Pop when a map lands | not observed (stills wait for the maps) |
| Artifacts not from the shadows | hills 30m-midday: the vertical streaks in the canyon wall are the mesh walls of the chunk stage (explorer's wall-artifact item), not the shadow term |

Open for the owner's look review: the scallops (azimuth count), and a site for the far-field check.
