# 0089 Relief classes drive the terrain's amplitude
Status: proposed (drafted by the spikes session for opus-planner, 2026-11-01; the owner decides: generator architecture and a store channel)
Date: 2026-11-01
Source: SPIKE-55, SPIKE-65 to SPIKE-83 (`spikes/m9-*/results/results.md`); `docs/briefings/m9-decisions.md` sections 2, 5 and 6; ADR 0016, 0026, 0028, 0030, 0045, 0066, 0076, 0080, 0088; CON-02, CON-06, CON-07, CON-14

## Context
- Relief below about 20 km comes from per-landform amplitude; hills and mountains are 4-9x too gentle today (SPIKE-55, SPIKE-65, SPIKE-66).
- Today's weights (the landform driver `L`, the hill band keyed to the range zone, the crease factor, the elevation ramps) are low at hill sites, switch relief off on plains (SPIKE-73) and cannot tell an arid plateau from a foothill (SPIKE-75); the isotropic range zone makes blobs, not belts (SPIKE-79).
- Amplitude per relief class passes criterion 1 at the hill landforms and the Alps at re-matched sites, but only with a hill band keyed to the class (SPIKE-80, SPIKE-81).
- Rule-based default classes with plate-margin belts and plateaus give Earth's class shares and hypsometry from 0.5 to 5 km (SPIKE-77 to SPIKE-80); a derivation from physical drivers places badlands well but invents rock hardness and PET (SPIKE-81).
- Owner, 2026-10-31: "Painting classes or other driver maps is intended. Painting types is poor." The owner chose option C (a rule-based default now, the physical derivation later behind the same interface; `m9-decisions.md` section 6).

## Decision
1. **Class weights per lattice point** are the interface: a channel on a fixed lattice per face edge (the mechanism of ADR 0080 point 3), weights of a versioned class list summing to 1, read bilinearly. Every consumer reads only this.
2. **Proposed:** the classes are plain, rolling plain, hills, mountains, high mountains, plateau, badlands and sand sea; a new class bumps the list's version.
3. **Proposed:** the default source is a rule set over the seed's inputs: plate-margin belts (SPIKE-79) as mountain strength in place of the range zone, plateaus (SPIKE-77), hills on belt flanks and old margins.
4. **Swappable source:** a derived source (SPIKE-81, through an ADR 0076 rule table) replaces the default only after it is measured on the real bake; the swap bumps the channel's version.
5. **Amplitude per class:** `k`, the fine-crease floor, the carve's `K t`, the deposition's `kc` and the hill band's amplitude, blended by the weights. Values and the per-cell limit on `k`'s change are in the generator spec.
6. **The hill band stays, keyed to the class** (SPIKE-80). The landform-keyed crease factor and the elevation ramps above 300 m are removed; the coast taper stays.
7. **Proposed:** the global bake's base elevation takes the belt term on the plate mask and the plateau uplifts (a `GLOBAL_VERSION` bump).
8. **Proposed:** class paint is an authored item (extends ADR 0066) that sets weights, feathered, and overrides every source. With the derived source, paint acts on its drivers (ADR 0076 point 7); no item names a type.
9. **The bake reads the classes:** ADR 0088's `k` and pass amplitudes come from the class weights.
10. **Canonical fields (CON-07, CON-14):** the plate mask and the class weights are CPU fields (`libm`, integer hashing, bit-exact for any thread count, no GPU twin). Their stored precision and the order of normalisation are in the generator spec.
11. **Crates (CON-02):**
    - the class paint item lives in `planet-world-def` with the other authored items (ADR 0066);
    - the class source, the weights and the amplitudes live in `planet-generators`, and the store reads them through a `planet-generators` trait that `planet-cache` implements, as `LatticeSource` today;
    - dependencies point downward only.

## Consequences
- Supersedes ADR 0080 point 2 (the landform-keyed creases), and ADR 0080 point 3 as the weight. Point 3's lattice mechanism stays (decision 1); its "not stored" ends, because paint items make the weights more than a function of the seed.
- Amends:
  - ADR 0080 point 1 (the band's amplitude from the class) and point 5 (the bounds rise by the class band's largest amplitude);
  - ADR 0028 point 6 (the detail amplitude from the class, not the base elevation);
  - ADR 0030 point 1 (the crease filter without the landform-keyed factor);
  - ADR 0016 (cap bounds carry the class band's and the detail's bounds, and with ADR 0088 the layer's pyramid);
  - ADR 0026 (a new channel and the base-elevation rule).
  Extends ADR 0066. Changes ADR 0045 (a stored channel) and ADR 0076 (the class as a consumer of drivers); both are proposed and are edited in place.
- Store: the class-weights channel is small against the erosion layer (SPIKE-76 section 4). The global bake and the coupled run are re-baked once (`m9-decisions.md` section 4); `global16384` stays until the new store passes the gate.
- A whole-planet class preview runs before any bake, in seconds (SPIKE-77).
- R7 tunes the class amplitudes on the scorecard. Until the re-bake, the site evidence is a stand-in: the new elevation on today's field (SPIKE-80).
- Steep classes need ADR 0088's min/max pyramid, or they stop refining at level 13-15 (SPIKE-82, SPIKE-83).
- Stop rule for R4 PR 1: a smoke-region bake, the scorecard and a plan check at production parameters; stop at a hill landform under criterion 1's bound or a capped tile at a mountain pose.
- Reversible: the default source is one switch, and the channel has a version; reverting is a re-bake.

## Licences of adopted code or techniques
None. The plate mask is a spherical Voronoi tessellation with Euler-pole rotations, a textbook construction; no code adopted.
