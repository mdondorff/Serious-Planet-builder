## Why
The owner and the planner judge the SPIKE-80 world map "generally wrong": the mountain belts are thin, sharp and continuous, and the continents are unchanged (SPIKE-91). The class source and the plate mask exist only in spike code (Python prototypes). The look of the generator has to be iterated with images before any planet bake (the bake waits for the owner's approval of the preview).

## What Changes
- **planet-generators `classes`:** the default class source of ADR 0089 decision 3 behind the class-weights interface of decision 1: the plate mask (GEN-094), the memberships (GEN-095), the `u8` weights on the landform lattice (GEN-093), the blended `k` (GEN-097) and the class hill band's amplitude (GEN-098), the base elevation of GEN-096 as a function beside the bake's. CPU-canonical (`libm`, integer hashing, thread-count independent). Not wired into the bake: `GLOBAL_VERSION` and every stored field are unchanged.
- **planet-generators `preview` and the `preview_world` example:** equirectangular images from those functions.
- **xtask:** `cargo xtask preview world --seed N [--out DIR]`.
- **Spec:** GEN-103 (the source and the preview), TEST-086 (the checks).
- Not in this change: wiring the source into the bake (`m9-bake`), tuning the constants (R7), the Earth-referenced shape metrics of SPIKE-91, a class paint item.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-103
- `testing`: TEST-086
