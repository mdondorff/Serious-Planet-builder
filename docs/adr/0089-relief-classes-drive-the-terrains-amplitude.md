# 0089 Relief classes drive the terrain's amplitude
Status: proposed (drafted by the spikes session for opus-planner, 2026-11-01; the owner decides: generator architecture and a store channel). The class source follows option C, decided by the owner on 2026-10-31 (`docs/briefings/m9-decisions.md` section 6).
Date: 2026-11-01
Source: SPIKE-55, SPIKE-65 to SPIKE-83 (`spikes/m9-*/results/results.md`); `docs/briefings/m9-decisions.md` sections 2, 5 and 6; ADR 0026, 0028, 0066, 0076, 0080, 0088; CON-06, CON-07, CON-14

## Context
- Relief below about 20 km comes from per-landform amplitude. Hills and mountains are 4-9x too gentle today (SPIKE-55, SPIKE-65, SPIKE-66).
- Today's weights are wrong:
  - the landform driver `L` and the hill band keyed to the range zone are low at hill sites;
  - the creases and the elevation ramps (`A(E)`, the detail amplitude) switch relief off on plains (SPIKE-73);
  - they cannot tell an arid plateau from a foothill (SPIKE-75);
  - the isotropic range zone makes blobs, not belts (SPIKE-79).
- Amplitude per relief class passes criterion 1 at the hill landforms and the Alps at re-matched sites, but only with a hill band keyed to the class (SPIKE-80, SPIKE-81).
- Rule-based default classes, with plate-margin belts and plateaus, give Earth's class shares (61 / 20 / 19 % against 61 / 14 / 25 %) and hypsometry from 0.5 to 5 km (SPIKE-77 to SPIKE-80). A derivation from physical drivers places badlands well but invents rock hardness and PET (SPIKE-81).
- Owner, 2026-10-31: classes and drivers may be painted, types never; option C (rule-based default now, physical derivation later behind the same interface).

## Decision
1. **Class weights per lattice point** are the interface: a pre-bake channel on a lattice like the landform lattice (GEN-062), with weights of a fixed, versioned class list summing to 1, read bilinearly. Every consumer reads only this.
2. **Classes:** plain, rolling plain, hills, mountains, high mountains, plateau, badlands, sand sea. A new class bumps the list's version.
3. **Default source (option C):** rules over the seed's inputs, a pure function of the seed (CON-06):
   - mountain strength from the convergent margins of a plate mask (SPIKE-79), replacing the range zone;
   - plateaus from an inland uplift field (SPIKE-77) and back-arc plateaus;
   - hills on belt flanks and old margins; plains elsewhere.
4. **Swappable source:** a derived source (SPIKE-81: plate history, rock hardness, aridity, height through an ADR 0076 rule table) replaces the default only after it is measured on the real bake. The swap is a channel version bump.
5. **Amplitude per class:** `k`, the fine-crease floor, the carve's `K t`, the deposition's `kc` and the hill band's amplitude are spec values per class, blended by the weights. The class blend is wide enough that `k` changes by at most a spec limit per lattice cell.
6. **The hill band stays, keyed to the class** (about 30 m on plains to 825 m in high mountains; SPIKE-80). Removed: the landform-keyed crease factor and the elevation ramps above 300 m. The coast taper stays.
7. **Base elevation:** the belt term on the plate mask (amplitude about 6,000 m), the back-arc and plateau uplifts, and the hill band per class enter the global bake (a `GLOBAL_VERSION` bump).
8. **Authoring:** class paint is an authored item (extends ADR 0066). It sets weights, feathered, and overrides every source. With the derived source, paint acts on its drivers (ADR 0076 point 7). No item names a type.
9. **The bake reads the classes:** ADR 0088's `k` and pass amplitudes come from the class weights.

## Consequences
- Supersedes ADR 0080 points 2 and 3 (landform-keyed creases, the landform driver as the weight). Amends:
  - ADR 0080 point 1 (the hill band's amplitude from the class);
  - ADR 0028 point 6 (the detail amplitude from the class, not the base elevation);
  - ADR 0026 (a new channel and the base-elevation rule).
  Extends ADR 0066.
- Store: the class-weights lattice is under 1 GB; the global bake and the coupled run are re-baked once (`m9-decisions.md` section 4). `global16384` stays until the new store passes the gate.
- A whole-planet class preview takes 1-2 s at 16 km, before any bake (SPIKE-77).
- R7 tunes the class amplitudes on the scorecard. Until the re-bake, the site evidence is a stand-in (the new elevation on today's field; SPIKE-80).
- Steep classes need ADR 0088's min/max pyramid, or they stop refining at level 13-15 (SPIKE-82, SPIKE-83).
- Stop rule (R4 PR 1): a smoke-region bake, the scorecard and a plan check at production parameters. Stop if a hill landform falls under 0.6 of real or a mountain pose caps a tile.
- Reversible: the default source is one switch and the channel has a version; reverting is a re-bake.

## Licences of adopted code or techniques
None. The plate mask is a spherical Voronoi tessellation with Euler-pole rotations, a textbook construction; no code adopted.
