# SPIKE-79: belts, not blobs

Opened: 2026-10-31
Question: the generator's high ground forms compact blobs (SPIKE-78); real orogens are long, narrow, often arc-shaped belts along plate margins.
- What makes the blobs?
- Does a belt-shaped range mask give long belts (aspect ratio of land above 2 km, against Earth's ranges) and keep SPIKE-78's hypsometry (7.1 / 3.2 / 1.4 % above 2 / 3 / 4 km)?
Why it matters: the relief-class map (SPIKE-76, 77) puts its mountain classes where the base field has high ground; that ground should have the shape of mountain belts.
Time box: 2.5 h (opus-planner, 2026-10-31); used about 1 h 10 min, with 2 min of machine time.
Success criteria:
- the blob cause named;
- a belt mask proposed and scored on shape and hypsometry;
- global maps before and after.
Code location: a Python prototype (`proto/beltshape.py.txt`) on SPIKE-69's 0.5-degree map of the generator's inputs (`target/s69/map13.f32`), resampled bilinearly to 0.25 degree. Spike code, never merged.
Closed: 2026-10-31

## Method
- **Today** (with SPIKE-78's amplitude): belt term `7000 ridge^2 zone inland`, plus base, hill band and SPIKE-77's plateau rule.
- **Proposal:** belt term `A B (c + (1 - c) ridge^2)` with a belt mask `B` in `[0, 1]` in place of `zone inland`:
  - **(a) lines:** the zero lines of a smooth fBm (frequency 2.2, 3 octaves), a Gaussian cross-strike profile `exp(-(d / w)^2)` of the distance `d = |n| / |grad n|`, cut into long segments by a second fBm (frequency 1.1);
  - **(b) plates:** the convergent margins of a spherical Voronoi tessellation of 14 plates. Each plate has a random Euler-pole rotation; the convergence (relative velocity against the margin normal) gates the margin through a smoothstep. The direction is warped (0.35 of a 2-octave fBm) so the margins curve. `d` is half the angular difference between the nearest and the second-nearest plate centre;
  - both add **coastal arcs** (Andean margins): the line at land depth 0.10 inland of the coast, on part of the coasts (a third fBm picks them);
  - the mask is gated at the coast only (`smoothstep(0, 0.04)` of the land depth), not by `inland`, so a coastal arc reaches the coast;
  - SPIKE-77's plateau rule is kept, placed away from `B` instead of away from the range zone.
- **Tuning:** half-width `w` 70-110 km, the ridge share `c` 0.35-0.65 and the amplitude `A` 5,000-7,000 m, scored on hypsometry (SPIKE-78's log error over 2, 3, 4 and 5 km against Earth without Antarctica).
- **Shape score:** connected regions of land above 2 km (8-connected, longitude wraps, at least 50,000 km²), land under 60 degrees. Feret length `L` (the largest great-circle distance in the region), width `W = area / L`, aspect `L / W = L^2 / area`. Also: median and area-weighted median aspect, the area share in regions with aspect 4 or more, the longest region.
- **Earth's ranges** (English Wikipedia infobox or article text, 2026-10-31; aspect = length / stated width):
  | Range | Length | Width | Aspect |
  |---|---|---|---|
  | Andes | 8,900 km | 330 km | 27 |
  | Rocky Mountains | 4,800 km | 110-480 km | about 16 |
  | Ural Mountains | 2,500 km | 250 km (Southern Ural) | 10 |
  | Himalayas | 2,400 km | 151-350 km | about 10 |
  | Caucasus | 1,200 km | 160 km | 7.5 |
  | Zagros | 1,600 km | 240 km | 6.7 |
  | Alps | 1,200 km | 200-250 km | about 5 |
  | Carpathians | 1,500 km | up to 500 km | 3 or more |
  | Tibetan Plateau | 2,500 km | 1,000 km | 2.5 |
  - Median aspect 7.5 (range 2.5 to 27). A named range is not a region above 2 km: Tibet and the Himalaya form one region above 2 km, the Appalachians and the Urals mostly lie under 2 km. The comparison is of shape, not a one-to-one match.
- **Runs:** BelowNormal (`raw/beltshape.log` for lines, `raw/plates/*.log` for plates with 10, 14 and 20 plates and warps 0.12, 0.25 and 0.35).

## Results
### What makes the blobs
- `zone` is a 3-octave fBm of frequency 1.7 through a smoothstep: an isotropic noise, so its high areas are round patches about 1,000-2,000 km across (`images/mask-zone-vs-belt.png`, left).
- `inland` is near 1 on most land, and `ridge^2` breaks the patch into spots. The product follows the patches: today's largest region above 2 km is 3,644 x 1,081 km (aspect 3.4).
- Nothing in the rule has a direction: there is no strike.

### Shape and hypsometry
| Variant | 2 / 3 / 4 / 5 km | Log error | Regions | Aspect median (area-weighted) | Area in aspect >= 4 | Longest | Highest |
|---|---|---|---|---|---|---|---|
| Today (zone blobs, 7,000 m) | 7.1 / 3.1 / 1.4 / 0.75 % | 0.44 | 11 | 3.5 (3.4) | 21 % | 3,644 km | 7,974 m |
| (a) lines, `w` 90 km, `c` 0.35, 6,500 m | 7.1 / 3.1 / 1.5 / 0.67 % | 0.47 | 17 | 9.6 (9.6) | 94 % | 7,712 km | 7,210 m |
| **(b) plates, 14, warp 0.35, `w` 90 km, `c` 0.5, 6,000 m** | **7.1 / 3.4 / 1.8 / 0.83 %** | **0.29** | 10 | 14.2 (15.4) | 96 % | 7,707 km | 6,828 m |
| (b) plates, 14, warp 0.35, `w` 110 km, `c` 0.35, 6,000 m | 7.4 / 3.4 / 1.7 / 0.77 % | 0.30 | 11 | 7.5 (13.6) | 95 % | 7,691 km | 6,802 m |
| Earth without Antarctica / named ranges | 6.7 / 3.2 / 1.9 / 0.8 % | - | - | 7.5 (ranges) | - | 8,900 km (Andes) | 8,849 m |
- **Both masks make belts:** 94-96 % of the land above 2 km lies in regions with aspect 4 or more, against 21 % today. The longest region grows from 3,644 to about 7,700 km.
- **Hypsometry holds:** the plates mask matches Earth better than today (error 0.29 against 0.44, 4 km at 1.8 % against 1.9 %). The amplitude it needs is 6,000 m, not SPIKE-78's 7,000 m: `B` spends less area at low weight than `zone inland`.
- The largest plates regions (`w` 90 km): 7,707 x 501 km (aspect 15), 4,144 x 527 km (8), 4,407 x 238 km (19), 4,063 x 157 km (26), 2,697 x 206 km (13). One 764 x 438 km region (aspect 1.7) is the plateau rule plus ridge spots, not a belt.
- **Lines vs plates** (images):
  - (a) the zero lines of a noise **meander and close into loops** (a ring of about 1,500 km and a small oval in the middle continent). Real orogens do not close on themselves.
  - (b) plate margins are **long, gently curved and never close**: they end at triple junctions or where the convergence fades. With warp 0.12 they are too straight; 0.35 curves them into arcs.
  - The coastal arcs carry much of the shape: without them the lines mask (`w` 70 km) drops to 5.0 % above 2 km (error 1.12), and the plates mask to 4.7 % (error 0.86).
- **Plate count:** 10 plates leave too few margins on land (5.6 % above 2 km at 7,000 m); 20 plates cut the belts into shorter pieces (area-weighted aspect 13, median 4.2). 14 is the middle.

## Findings
1. **The blobs come from an isotropic range mask.** `zone` is a smoothstepped fBm without a strike, so high ground forms round patches (aspect 3.4); `ridge^2` breaks them into spots.
2. **A plate-margin belt mask gives belts and keeps the hypsometry.**
   - It uses 14 Voronoi plates with Euler-pole rotations, convergent margins only, a warp of 0.35, a 90 km Gaussian half-width, coastal arcs, and `A` 6,000 m with half the amplitude from the ridge.
   - Land above 2 / 3 / 4 km is 7.1 / 3.4 / 1.8 % (Earth 6.7 / 3.2 / 1.9 %).
   - 96 % of the high ground lies in belts with aspect 4 or more; the area-weighted aspect is 15 (Earth's named ranges: median 7.5, 2.5 to 27).
3. **Noise isolines are the cheaper mask but loop:** (a) gives aspect 9.6 and the same hypsometry, with rings and meanders that no orogen has.
4. **The belts are too uniform:** one Gaussian width everywhere, and no broad high ground behind them (Tibet, the Altiplano). Earth's ranges vary in width (Himalaya 151-350 km) and some sit on plateaus. A width that varies along strike and a back-arc plateau tied to the strongest convergence are the next steps; neither is tested.
5. **The highest land drops to about 6.8 km** (Earth 8.8 km), because `B` and the ridge rarely peak together. More ridge weight at the margin's crest (`c` lower at `B` near 1) is the knob; it is not tested.
6. The amplitude in the store re-bake is 6,000 m with this mask, 7,000 m with today's mask (SPIKE-78). Both are a `GLOBAL_VERSION` bump.

## Open points
- **The plates are a mask only:** no plate ages, no ocean-floor depth, no continents placed by plates. The continents stay the generator's fBm, so margins cross continents at random. A margin through the middle of a continent reads as a suture (Urals), which is plausible but not chosen.
- **The site matches move:** SPIKE-55's Alps and Himalaya sites lie in today's blobs; with belts the matched sites need re-matching (scorecard), not done here.
- **Class shares** (SPIKE-77) were not recomputed with the belts.
- **Resolution:** 0.25 degree (28 km), resampled from a 0.5-degree map; belt widths of 150-250 km are 5-9 cells. One seed; the prototype's noise and plates are not the generator's.
- The Earth figures are named-range dimensions from Wikipedia, not regions above 2 km from a DEM.

## Visuals
- `images/elevation-blobs-vs-plate-belts.png`: elevation 0 to 6 km, today (left) and the plates mask (right), equirectangular, 0.25 degree, shown at 0.5 degree.
- `images/above-2km-blobs-vs-plate-belts.png`: land above 2 km (red), today and plates.
- `images/elevation-blobs-vs-belts.png`, `images/above-2km-blobs-vs-belts.png`: the same for the lines mask (a), with its loops.
- `images/mask-zone-vs-belt.png`: today's `zone inland` (left) and the lines mask `B` (right).

## Handover
- `python -I proto/beltshape.py.txt <SPIKE-69 map13.f32> <out>` (about 15 s). Knobs: `S79_MODE` (`lines` or `plates`), `S79_NP` (plates), `S79_WARP`, `S79_FI` (the lines' frequency), `S79_GI`, `S79_GC` (segment gates), `S79_DC` (the coastal line's land depth), `S79_PICK` (the variant drawn).
- Plates run: `S79_MODE=plates S79_NP=14 S79_WARP=0.35`.
