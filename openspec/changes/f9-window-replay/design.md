## Decisions
- Reuse the gate's window path replay (`PLANET_GATE_PATH`, TEST-061): it already drives the camera from a path file in real time on the window's clock after the settling frames and ends the run at the path's end. `repro --window` only starts it over the right world; no second replay is built.
- The world is reopened from options, not from the bundle's definition text: the viewer opens a commit over the in-memory production base (VIEW-069), which the bundle's 1,024-cell rebuild cannot. The bundle therefore records the two options that decide the world beside the seed: `open_commit` (absolute path, escaped like the world text) and `open_site`. The version stays 2: the lines are optional and an old build refuses a bundle that carries them ("cannot parse bundle line"), which is the safe direction; a bundle without them reads in every build.
- The stall count is a counter of the gate, not a new probe: a frame whose phases, GPU phases included, add up to the stall limit (`stall::limit`, 1 s) or more. The probe's reports are rate-limited to one in 10 s, so counting files undercounts; the gate sees every frame. The count is reported and never fails the gate (its frame limit leaves the GPU phases out on purpose).
- Options after `--` go to `planet editor` unchanged, so the same command measures VIEW-070 before and after (`-- --resident-cap 1000000`, then the default) without a flag of its own.
- `--real` ends as NOT RUN on a locked desktop (TEST-073); without it the software adapter runs and its times are not judged.

## Tests
- Tier A: the bundle round trip with and without the lines (frame `terrain.rs`); `open_args` on fixture text, the stall count in the gate line, the verdict line (xtask `windowrepro.rs`); the gate counter with a 4.5 s render submit (app `gate.rs`); the refused option combinations (xtask `testcmd.rs`).
- The window run itself needs the desktop and the store: it is measured for E2's G-RT, not asserted.
