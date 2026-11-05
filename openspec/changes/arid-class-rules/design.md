## Context
- `class_weights(terms, e_smooth, rolling)` (GEN-095) gives six classes that sum to 1; badlands (k 2.0) and sand sea (k 0.35, carve factor 0.5) stay 0. `ClassLattice` holds the weights as `u8` on 257² nodes a face; `BakeInputs::weights_at` reads it, `base` and `carve_factor` use that read.
- `ClassSource::eval` gives `e0` before the class `k`; the orographic climate reads only `e0` (GEN-117), so a class weight cannot reach the rain, and the arid rules may read the rain.
- GEN-097 limits the step of `k` between lattice cells; the rules must keep that limit (the preview prints `largest_k_step` and the shares of steps over the limit before and after).

## Decisions
1. **Rules** (all `smoothstep` ramps at least 0.15 of aridity or 300 m wide, so no weight steps within a lattice cell):
   - `soft = smoothstep(0.55, 0.35, H)` (the lithology hardness, lowest where soft);
   - `semi = smoothstep(0.08, 0.25, AI) * (1 - smoothstep(0.40, 0.65, AI))` (semi-arid, the badlands' climate);
   - `moderate = smoothstep(100, 400, e0) * (1 - smoothstep(1800, 2800, e0))`, and `broken = max(hilly, flank, old_margin)` of the plate terms (relief the water can cut);
   - `badlands = soft * semi * moderate * broken`;
   - `arid = 1 - smoothstep(0.05, 0.22, AI)`, `low = 1 - smoothstep(300, 900, e0)`, `supply = max(basin, 0.5 * old_margin, 0.4 * rift)` of the province terms (sediment to blow);
   - `sand = arid * low * supply`.
   Each is capped at 0.8 so a neighbouring class keeps a share, and the two together at 0.85.
2. **Blend.** `weights = (1 - w_bad - w_sand) * base + w_bad e_badlands + w_sand e_sand_sea`, the sum 1; applied in `BakeInputs::weights_at` over the lattice's weights, so the stored class lattice does not change.
3. **No amplitude step.** The badlands class keeps `k` 2.0 (between plains and hills), sand sea 0.35 with the carve factor 0.5: both blend through the weights, which are smooth. The badlands effect comes from the soft rock (K of GEN-116, once the carve reads it) and from the hill band: a soft, semi-arid country with broken relief gets the badlands weight, and the K field makes it erode into crease and scarp; the class itself adds no abrupt amplitude.
4. **Inputs.** The aridity and `H` come from lattices built on first use (`BakeInputs::orography()`, a `Lithology` held by `BakeInputs`); the plate terms from the class lattice's source.
5. **Keys.** `ARID_RULES_VERSION` and the climate identity (`climate_id`, GEN-117) join `BakeInputs::id()`: the base elevation reads the weights, so every layer key changes with them. The stored read-only stores are never written.
6. **Preview** (`generate --arid-preview DIR [--seed N | --world F] [--km N]`): the class map (the eight classes), the weights of badlands and sand sea, `arid.txt`: the share of land by class against Earth's (61 / 14 / 25 % plain, hills, mountains and SPIKE-77's badlands 3.3 % and sand sea 7.9 %), `largest_k_step` and the share of steps over GEN-097's limit before and after, and the scorecard of the two arid sites (badlands SD, Erg Chebbi) from `terrainscore` before and after (the class weights at the sites and the `k` they give; "not worse" is the planner's acceptance).

## Earth-specific values
Earth is the default preset, not the model (owner, 2026-11-05): every threshold is a field of `AridParams`, whose `Default` is the Earth preset; the kernels read the parameters, never a literal.
- **Physics** (hold on any planet with liquid water and an atmosphere): the aridity index `P / PET` as the measure of dryness; the ordering semi-arid (the badlands' climate) between humid and arid; sand needs a dry climate, low relief and a supply of loose sediment; badlands need soft rock and relief the water can cut; every ramp smooth.
- **Earth calibration** (the preset's values; another planet sets its own): the aridity ramps (semi-arid 0.08 to 0.65, arid below 0.22; UNEP's breaks 0.05, 0.2, 0.5, 0.65); the height ramps (moderate 100 to 2,800 m, low below 900 m, from Earth's badlands and ergs); the hardness ramp (soft below 0.55 to 0.35); the supply weights (basin 1, old margin 0.5, rift 0.4); the caps (0.8 each, 0.85 together); the class `k` and carve factor of the two classes (GEN-097).
- The height ramps are in metres above the sea of the planet: a planet with another relief range scales them; `AridParams` is the place, and the class source's own constants (`BELT_AMPLITUDE_M` and its kin) are Earth values of the same kind.

## Risks
- The shares are start values: the owner's preview round tunes the ramps.
- Arid weights make the base elevation depend on the climate: the layer bake needs the 512-cell climate (about 30 s on 8 threads, once).
- A class edge where `k` changes quickly would raise the hill band's amplitude with it: the ramps keep the existing step limit, verified by the preview and a test.
