# SPIKE-67: G3 block grid at the feet

Opened: 2026-10-31
Question: What draws the axis-aligned block grid of REND-034's 3 cm octave at the feet (SPIKE-63 finding 3), and does one fix remove it within REND-034's +1.0 ms?
Why it matters: the most visible defect at 1.8 m (SPIKE-63); M9 R6 and criterion 4 (the owner's look at 1.8 m).
Time box: 1.5 h (opus-planner, 2026-10-31); used about 1 h, with 10 min of machine time.
Success criteria: the cause named; before/after stills at the SPIKE-63 feet poses; GPU time against REND-034's 1.0 ms.
Code location: branch `spike/m9-g3-grid` (pushed, no PR; commit 385706cb, on top of SPIKE-63's `spike/m9-ground-detail`; spike code, never merged).
- `render/src/terrain.rs`: `SPIKE67_G3=grad3|gradall|off3` rewrites the lit shader source before it is compiled.
Closed: 2026-10-31

## Cause
The noise itself, not the sampling, the hash, a mip or a derivative.
- `gd_plane` is planar **value noise** with a smoothstep fade; the shading normal comes from its analytic gradient.
- Along one lattice axis the gradient is `6u(1-u)` times a corner difference: a bump that is 0 on every cell edge. Across that axis it is a linear blend of two corner differences.
- In low sun the shading reads mostly the slope along the sun's direction. Each cell shows as a bar: flat lines at every edge along the sun, smooth across it. The result is a grid of stretched blocks aligned with the lattice, perpendicular to the sun.
- The 3 cm octave (k 3, odd) uses the plane's own lattice, so its grid is axis-aligned. The 25 cm octave has the same flaw on the 45-degree lattice, but at the feet it is faded and weighted less.
- Ruled out:
  - **The hash:** the corner values have no neighbour correlation (|r| < 0.03 at offsets 1-8, both axes; `raw/proto/hash.py.txt`).
  - **Precision:** the octave is anchored by the integer cell plus a tile-local f32 offset (CON-11); at the feet a 3 cm cell spans about 30 px, so there is no staircase.
  - **The fade:** at the feet the pixel covers about 0.03 of a cell, below the 0.06 where the fade starts.
- A CPU replica of the octave alone, shaded in low sun, reproduces the bars with smoothstep and with quintic value noise. Gradient noise does not (`images/cpu-replica-shading.jpg`: cubic, quintic, gradient).

## Fix (prototype)
- Gradient noise in place of value noise:
  - the same cell, shear, hash and world anchoring;
  - the corner value turned into a unit gradient at the angle `value x pi`;
  - quintic fade, analytic derivative.
- Value and slope are scaled by 1.23, so the RMS slope matches the value noise (1.08 lattice units; CPU replica). The look's strength stays as tuned.
- Variants:
  - `grad3`: the 3 cm octave only;
  - `gradall`: all four octaves;
  - `off3`: the 3 cm octave left out (reference).

## Method
- **Sites and poses:** SPIKE-63's alps (11.5, -18.5, heading 0) and hills-b (8.972, -16.238, heading 90). Eye 1.8 m, 60 m back, sun at 17.25 local hours: `eye-lowsun` (pitch 6 degrees down) and `feet-lowsun` (35 degrees down). G3 strength 1.0 (`raw/poses/`).
- **Captures:** `test-render --scene capture-stills` on the RTX (Vulkan), 2560 x 1440, one process per site and variant.
  - 12:08 to 12:15, plus `off3` again at 12:17, under the heavy lock at BelowNormal.
  - CPU load 0-15 % before each run (one 43 % reading at hills-b base; `raw/driver.log`).
- **GPU time:** SPIKE-63's `SPIKE63_COST=1`. After each still the terrain pass is timed with the product shader and with REND-044's compiled-out variants. Each variant is the median of 3 round medians of 41 frames. Indicative only: not a perf window and no readiness gate.
- The first `off3` run panicked in naga (`Expression [241] is not cached!`, a constant octave). Rewritten as `0.0 * octave` for look only; that variant has no timing.

## Results
### Look (`images/`)
- `alps-g3.jpg`, `hills-b-g3.jpg`: columns base, grad3, gradall, off3; rows feet-lowsun and eye-lowsun; crops of the lower middle at 1:1.
- `alps-feet-before-after.jpg`: alps feet, base against grad3, 1:1.
- **The grid is gone** with `grad3` at both sites: the ground reads as round grains and pits with no lattice direction.
- A grain along the sun's normal remains, because low sun lights only the slope along it. It is a property of lighting isotropic bumps, not a lattice.
- `gradall` looks like `grad3` at the feet. At eye level it also breaks the faint diagonal ripple of the 25 cm octave; the difference is small.
- `off3` shows what the octave adds: without it the feet are smooth blur.

### GPU (terrain pass, ms, 2560 x 1440; strength 1.0)
| Site | Pose | Full pass: base / grad3 / gradall | Ground detail share (compiled out): base / grad3 / gradall |
|---|---|---|---|
| alps | eye-lowsun | 2.82 / 2.78 / 2.79 | 1.07 / 1.10 / 1.11 |
| alps | feet-lowsun | 2.26 / 2.19 / 2.27 | 1.06 / 0.96 / 1.00 |
| hills-b | eye-lowsun | 3.37 / 3.49 / 3.50 | 1.31 / 1.35 / 1.40 |
| hills-b | feet-lowsun | 2.39 / 2.41 / 2.53 | 0.88 / 0.90 / 1.02 |
- **`grad3` costs no measurable time:** -0.10 to +0.04 ms against base, inside the round spread (up to 0.26 ms).
- **`gradall`** is +0.04 to +0.14 ms: four sin and cos pairs per octave and plane.
- The share was already at or over REND-034's +1.0 ms before the fix (0.88-1.31 ms here, as SPIKE-63 found). The fix does not change that.

## Findings
1. **The cause is value noise.** Its derivative is 0 on every cell edge along one axis and linear across it, so the bump shading draws the lattice. The hash, the precision and the fade are not the cause.
2. **Gradient noise on the same lattice and hash removes the grid,** with the RMS slope matched, so the look's tuning holds.
3. **The 3 cm octave alone as gradient noise costs nothing measurable;** all four octaves cost up to +0.14 ms.
4. **For R6:** switch the 3 cm octave to gradient noise (all octaves optional; the 25 cm octave has the same flaw, faded at the feet). Golden impact: lit views at ground level change, a declared look change through `golden-review`. The G3 budget question (SPIKE-63 finding 4) stays open.

## Open points
- Corner gradients use `cos` and `sin` of a hashed angle. A table of 8 or 16 directions picked by hash bits would avoid the trigonometry; that is R6's choice (it must not index an array by a loop counter, G2c).
- Only low sun was checked; midday shows the octave weakly anyway (SPIKE-63).
- The naga panic on a constant octave was not reduced to a report.

## Handover
- **Rerun** from the worktree root on branch `spike/m9-g3-grid` (build `cargo build --release -p planet`):
  ```
  SPIKE67_G3=grad3 SPIKE63_COST=1 target/release/planet.exe test-render --scene capture-stills --adapter hardware --store C:/DEV/SeriousPlanetData/global16384 --poses spikes/m9-g3-grid/results/raw/poses/alps-s67.toml --out-dir target/s67/alps/grad3
  ```
- Full stills: `target/s67/` in the spikes worktree (not in git). The driver is `raw/proto/run.sh.txt`.
