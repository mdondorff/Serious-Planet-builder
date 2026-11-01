## Why
M9 criterion 1 asks hill and mountain relief of at least 0.6 of real (scorecard, TEST-079); today it is 0.11-0.35 (SPIKE-55). SPIKE-61 to SPIKE-66 found where the relief comes from and how to bake it within 10 h:
- the amplitude of the base field below about 20 km, per landform (SPIKE-65, SPIKE-66);
- a transient carve with a breach before the fill;
- thermal at repose and one deposition sweep (SPIKE-64, SPIKE-65, SPIKE-66);
- routing at a reduced cadence (SPIKE-61, SPIKE-62).

ADR 0088 (proposed) fixes the format. This change builds it as M9 item R4.

## What Changes
- **Baked channels:** the relief driver `k` (per landform, tapered at the coast) and the ring mean `m` (10 km) beside the global bake, with min and max per block.
- **Runtime base field:** `m + k (e - m) + k creases`, each term one read at the point; bounds per cap from the channels' block ranges; behind a switch, off by default until the owner's look.
- **Bake kernels** in `planet_generators` (f64, `libm`, thread-count independent):
  - the improved priority flood (Barnes);
  - jittered D8 receivers and the topological order from them;
  - an implicit stream-power carve with uplift, with a breach before the fill;
  - an active-cell thermal pass at repose;
  - one deposition sweep.
- **The erosion layer:** a tile driver over the 100 m grid of SPIKE-61 (256-cell halo, core stored, the coupled network's inflow). It checkpoints per tile and writes the change against the sampled base as a separate layer beside `global16384` (M9 draft decision 7). A `generate` command bakes it; `--resume` continues.
- **Runtime layer:** the field adds the layer bilinearly at 100 m, with its block ranges in the bounds; behind the same switch.
- **Scorecard report** on the baked field at the four hill sites and the other landforms.

## Capabilities
### New Capabilities
- none

### Modified Capabilities
- `generator-pipeline`: the relief classes and their default source, the base elevation, the class amplitudes and hill band, the relief channels and the runtime base formula, the two bake passes, the cap bounds (GEN-093 to GEN-102, planned)
- `streaming-cache`: the erosion layer's storage and keys, its min/max pyramid, the class-weights and relief channels (STRM-065 to STRM-068, planned)
- `testing`: the scorecard run on the baked field, the level-step and seam checks (IDs TBD)

## Impact
- Code:
  - `generators/src/erosion.rs` (the base formula and bounds);
  - `generators/src/global.rs` (the channels);
  - `generators/src/bake/` (new kernels);
  - `cache` (the layer);
  - `app` (the `generate` command, the switch);
  - `xtask` (the command under the heavy lock).
- Data: a new layer of about 7-10 GB beside `global16384` and channels under 1 GB; `global16384` is not rewritten.
- Behaviour: none by default (the switch is off) until the owner's look at R4; then every consumer reads the baked field.
- Runs: one overnight bake (estimated 2.5-4 h, criterion 7 at most 10 h), BelowNormal, machine reserved.
- Goldens: none while the switch is off; turning it on is a declared look change through `golden-review`.
