# SPIKE-124: protect intentional lakes through the D-1 stage (P3 E5, variant A)

Opened: 2026-11-15 (lead #147)
Question: where is a lake "intentional", and can variant A's chain (one round at L, then K7) keep such a lake through the D-1 stage?
Why it matters: SPIKE-123 found that the round at L drains every lake; D-4 needs a rule that keeps a lake, and G-LAKE needs it to pass.
Time box: 0.5 day of agent work (used).
Success criteria: the G-LAKE lake survives A on R1h; G-DRAIN with protected lakes counted as lakes; no new canyons or basins in a 5 km ring.
Fallback if it fails: name the stage that drains the lake and the fix it needs.
Code location: branch spike/lakes-protect (local, never merged), cut from explorer's spike/e3-variants-bc (b8c16e25) so B and C reuse the switch: `bake_eroded` `PLANET_PROTECT_LAKES` (=1 or =dam); `river_gate --h1-lakes --protected --ring-lake K --lakes`.

## 1. Where "intentional" is defined
- **Rule:** a lake is intentional when H1 keeps it on the regional solve: `front.lakes`, the kept basins of ADR 0111 and GEN-145 (class Lake, endorheic or intentional per GEN-146), with their water cells, ring and spill level. This is the earliest stage that names a lake (owner, 2026-11-12: fix at the source stage).
- **Consequence for the fixture:** SPIKE-123's 130 km2 lake (L cell (65823, 7542)) is not an H1 lake: it is a basin of the 814 m to L hand-over (the settled diagnosis of P3), not a lake H1 named. The fixture is now the largest H1 kept lake inside R1h's core:

  | Property | H1 lake 0 |
  |---|---|
  | id | 0563b44fc93a2d1d |
  | Regional centroid | (8268, 1094) |
  | Floor / spill = level | 1.89 m / 4.60 m |
  | Water | 107 H1 cells (49.9 km2); 5,317 L cells below the spill |
  | Inflow / outflow | 484.1 / 483.9 m3/s (the main river runs through it) |

- H1's inland lakes of R1h's window (up to 4,317 km2, floors 300-750 m) all lie in the margin; the core holds only low lakes (floors 0-1.9 m, spills up to 5.1 m).

## 2. The protection (variant A's chain only; off: the same bits)
- **Footprint:** the L cells under a lake's H1 water cells that stand below its spill on the start surface `z0`. They are outlets for the round and for C's diffusion (an outlet keeps its height): no breach runs through the lake, and rivers end at its shore.
- **Rim area:** the L cells under the lake's H1 water and ring cells. After the round and the diffusion none stands below `min(z0, spill)`: the spill is not lowered.
- **Dam** (`=dam`): the rim area's outer-edge cells below the spill are raised to it, because the L detail under H1's water leaks below the spill.
- **No channel in a lake** (GEN-163 for the L rivers): rim-area cells below the spill hold no L channel, so K7 burns no line through the lake or its rim; the outflow starts at the spill with its own level.
- Restores go to `z0`, the round's start, after the diffusion (explorer's points, 2026-11-15).

## 3. Results on R1h (face 1, seed 14, i 8056-8675, j 760-1379)

| Store | Kept lakes at H1 lake 0 (level, area) | Artefacts at lake 0 | Basins (gate) | G-DRAIN, protected lakes as lakes | Edge | Bake |
|---|---|---|---|---|---|---|
| A, no protection (`r1h-round-k8`) | none | 112 | 0.58 % | 0.14 % | 34 of 44 | 520 s |
| footprint + rim restore | 2.62, 2.30, 0.90 m (1.1, 0.4, 0.5 km2) | 47 | 1.26 % | 0.17 % | 34 of 44 | 454 s |
| + dam | the same three | 47 | 1.25 % | 0.17 % | 34 of 44 | 489 s |
| + dam + no channel in lakes | **4.69 m (11.9 km2)** and 1.55 m (13.8 km2) | 151 | 1.40 % | 0.19 % | 34 of 44 | 451 s |

- **Survival: partial.** With all three parts, a kept lake stands at 4.69 m (H1: 4.60 m) over 11.9 km2, and a second part at 1.55 m over 13.8 km2: 25.7 of H1's 49.9 km2. The low part leaks below the spill somewhere the dam does not reach (not traced in the time box).
- **G-DRAIN:** river length in closed basins with protected lakes counted as lakes is 0.19 % (bar 10 %). Counted as basins it is 1.40 %, because the kept lakes are closed by design.
- **5 km ring around lake 0** (48,182 cells): valley depth at ring 1-2 is 0.00 % over 50 m (469 river cells) on every store. The protection opens no canyon there.
  - It does add shallow pits inside the footprint: 151 artefacts against 112.
  - The gate's own ring count reads 0 river cells (a tool limit of the ring as an `inside` mask, not traced); the river count here is the spike's own.
- **The river does not run through the L lake:** the 4.69 m lake receives 1.14 m3/s, while H1 routes 484 m3/s through lake 0. The L rivers take another path past it. Keeping H1's trunk through its lake at L is D-2's (E4: H1 owns trunk topology).
- The whole window keeps 41 lakes (39.3 km2) against 35 (11.8 km2) unprotected. Most protected H1 lakes are in the margin, outside the counted box.

## Findings
1. "Intentional" is defined at H1 on the regional solve; SPIKE-123's fixture was a hand-over basin and is replaced by H1 lake 0.
2. Three stages drain a lake, and each needs its own part of the rule:
   - the round's breach: the footprint as outlets;
   - the L detail under H1's water, which is no closed basin at L: the dam on the rim area's edge;
   - K7's burn of L rivers through the lake: no channel in a lake (GEN-163 for L lines).
   Protecting heights alone keeps nothing.
3. With all three, the lake level survives (4.69 against 4.60 m) on half of H1's area. Full survival needs the remaining leak traced, and H1's trunk routed through the lake at L.
4. Key: the protected stores have the same commit id as the unprotected one (39038c59), because the inputs identity does not carry the switch. A production form must put the protection into the key (CON-07).
5. For B and C: the switch sits on a branch cut from spike/e3-variants-bc. Explorer reuses it as one switch, and the rule is the same for every variant.

## Visuals
None: counts only. The G-LAKE flight (#146) waits for E3's winner and full survival.
