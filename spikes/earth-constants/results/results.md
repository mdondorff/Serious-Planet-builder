# SPIKE-103: Earth-specific values in the generator kernels on main

Date: 2026-11-06. Read-only audit (ADR 0108 point 5); no code changed. Base: `origin/main` after #1219 (GEN-129 runoff), #1221.

## Method
- Read every non-test numeric literal, `const` and parameter default in `crates/generators/src` (`orography`, `global`, `climate`, `classes`, `continents`, `lithology`, `uplift`, `arid`, `shelf`, `runoff`, `drivers`, `erosion`, `erosion_layer`, `bake/*`), and the pass specs in `crates/cache/src/layer_tiles.rs`.
- Kind: **L** law or numerical method (stays in the kernel); **E** Earth planet state (a world parameter); **C** calibration (fixed per preset, ADR 0108 point 3); **T** tuning shape (class thresholds, noise scale; a preset default, ADR 0108 point 2); **S** structure (hash layer, format, lattice size; not Earth).
- "Moves output?": whether lifting the value into a parameter, with the same default, changes any baked bit. Always "no" for the value; the only risk is a key (`inputs_id`) that hashes the value or a version; noted where it applies.

## Summary
| Group | Section | Already a parameter |
|---|---|---|
| Climate, orography, planet state | 1 | none (module consts and inline) |
| Class map, plates, shelf, arid | 2 | `ShelfProfile`, `AridParams` |
| Continents | 3 | none |
| Lithology and uplift | 4 | none |
| Carve, pass 1 and pass 2 | 5 | `CarveParams`, `Pass2Params`, `ThermalParams` |
| Runoff and vegetation | 6 | none (consts; identity folded into the key) |
- Already parameter structs with the Earth value as `Default`, so the kernels read fields: `AridParams` (`arid.rs:30`), `ShelfProfile` (`shelf.rs:38`), `CarveParams` (`carve.rs:44`), `Pass2Params` (`tile.rs:111`), `ThermalParams` (`thermal.rs:33`), `BakeShape.radius_m`, `LayerSpec.sea_level`. These need no lifting, only a place in the world definition.
- The planet radius exists as four separate Earth constants (`classes.rs:44`, `orography.rs:91`, `core::EARTH_RADIUS_M`, `bake::tile` through `BakeShape.radius_m`): one world parameter must feed all.
- Largest single cluster: `orography.rs` (wind, season, moisture) and `global.rs:417-423` (temperature and rain bands), which hold Earth's rotation, tilt, lapse rate and zonal rainfall as literals.

## 1. Planet state (E)
| Name | Value | file:line | Kind | Proposed parameter | Moves output? |
|---|---|---|---|---|---|
| Planet radius | 6,371,000 m | `classes.rs:44`; `orography.rs:91`; `core` `EARTH_RADIUS_M` | E | `planet.radius_m` (one source; `BakeShape.radius_m` already carries it into the carve) | no |
| Sea level (continentalness) | 0.1 | `global.rs:42` | E | `ocean.sea_level_c` (fraction of land follows) | no |
| Land fraction | 0.29 | `continents.rs:19` | E | `ocean.land_fraction` | no |
| Elevation clamp | ±9000 m | `global.rs:47-48` | E | `planet.elevation_range_m` (a bound, a format limit as well) | no |
| Seasonal subsolar shift | 12.0 deg | `orography.rs:58` | E | `planet.axial_tilt_deg` derived (the 12 deg is a tuned fraction of Earth's 23.45) | no |
| Rain-band seasonal shift | 5.0 deg | `orography.rs:61` | E | `climate.rain_shift_deg` | no |
| Solar declination | 23.45 deg, 284/365 days | `drivers.rs:141` | E | `planet.axial_tilt_deg`, `planet.year_days`, `planet.equinox_offset_days` | no |
| Year length | 3.1557e7 s | `orography.rs:90` | E | `planet.year_s` | no |
| Wind speed cap | 15 m/s | `orography.rs:70` | E | `climate.wind_max_ms` | no |
| Monsoon strength and gradient | 6 m/s, 3.0 | `orography.rs:65-66` | E | `climate.monsoon_ms`, `climate.monsoon_gradient` | no |
| Polar fade latitude | 80 deg | `orography.rs:68` | E | `climate.polar_fade_deg` | no |
| Global wind bands | gauss at 15/45/78 deg, widths 14/15/12, amplitudes -8/12/-5 zonal, -2/2.5/-1.5 meridional, tanh(lat/4) | `orography.rs:195-198` | E | `climate.wind_bands` (table: centre, width, zonal, meridional); the Hadley/Ferrel/polar cells follow rotation rate and radius | no |
| Wind turn noise | 25 deg | `orography.rs:63` | T | `climate.wind_turn_deg` | no |
| Zonal rain bands | 700 + 1900 g(0,12) + 900 g(52,14) - 650 g(27,8) mm | `global.rs:423` | E | `climate.rain_bands` (table: centre_deg, width_deg, amplitude_mm; base) | no |
| Rain wetness noise and interior dry-out | 0.45 fbm, -0.25 interior, cap 4500 mm | `global.rs:408-409` | T | `climate.wetness_noise`, `climate.interior_dry`, `climate.rain_cap_mm` | no |
| Sea-level temperature | 30 - 55 sin²lat C | `global.rs:417` | E | `climate.equator_c`, `climate.pole_drop_c` | no |
| Lapse rate | 6.5 C/km (land only) | `global.rs:417` | L/E | `climate.lapse_c_per_km` (a law on a given atmosphere) | no |
| Temperature noise | 4.0 C | `global.rs:417` | T | `climate.temperature_noise_c` | no |
| Moisture reference column and growth | 25 mm, 0.07 per C, ref 15 C | `orography.rs:82-83,96` | E | `climate.column_ref_mm`, `climate.column_growth`, `climate.column_ref_c` (Clausius-Clapeyron near 7 %/K: a law, the reference is Earth's) | no |
| Land recycling | 0.55 | `orography.rs:84` | E | `climate.land_recycle` | no |
| Relaxation times | condense 1000 s, fallout 1000 s, sea 4e4 s, land 6e4 s | `orography.rs:76-80` | C | `climate.tau_*_s` | no |
| Orographic lift depth, efficiency, upslope threshold | 2000 m, 0.18, 0.02 m/s | `orography.rs:86-89` | C | `climate.lift_depth_m`, `climate.efficiency`, `climate.upslope_threshold_ms` | no |
| Land mask rings | 500 km, 1000 km, 8 samples, sum/17 | `orography.rs:167-174` | C | `climate.continental_rings_km` (a method; scales with radius) | no |
| PET law | 300 + 45 T mm, floor 150 mm | `orography.rs:355` (also `runoff.rs:22` through it) | C | `climate.pet_base_mm`, `climate.pet_per_c_mm`, `climate.pet_floor_mm` (SPIKE-81 regression on Earth) | no |
| Iteration setup | 96 steps of 1300 s, average of 8 | `orography.rs:72-74` | S | none (numerical method; the result must not depend on it beyond convergence) | no |
| Insolation day step | 73 days, 5-day step | `drivers.rs:136-137` | S | none | no |
| Neutral soil and moisture | 1.0 m, 0.5 | `drivers.rs:145-146` | E | `surface.neutral_soil_m`, `surface.neutral_moisture` | no |
| Soil and depth codes | 8.0 m soil max, 64 m depth max | `drivers.rs:15`, `drivers.rs:84` | S | format range, `DriverParams.soil_max_m` exists | no |
| Lens ranges (preview only) | -30..40 C, 3000 mm | `coupled.rs:681-683` | S | none (display) | no |

## 2. Class map and plates (T/E)
| Name | Value | file:line | Kind | Proposed parameter | Moves output? |
|---|---|---|---|---|---|
| Plate count, draws, rates | 14 plates, rate 0.3 + 0.7 u | `classes.rs:39,114` | E | `tectonics.plate_count`, `tectonics.rate_range` | no |
| Belt amplitude | 7800 m | `classes.rs:52` | C | `tectonics.belt_amplitude_m` | no |
| Plateau uplift | 600 m | `classes.rs:54` | C | `tectonics.plateau_uplift_m` | no |
| Back-arc, old-margin, flank heights | 2000 m, 200 m (flank 0.5) | `classes.rs:487-495` | C | `tectonics.back_arc_m`, `tectonics.old_margin_m` | no |
| Coast top and ramp | 620 m, depth 1.2 | `classes.rs:56-57,482` | C | `tectonics.coast_top_m`, `tectonics.coast_ramp_depth` | no |
| Belt min width, bend, buffer, ramps | 125 km, 120 km, 50 km, 300 km, 0.3, 0.2 | `classes.rs:59,68-75` | C | `tectonics.min_belt_km` and the rest | no |
| Mountain full core, flank membership | 0.4, 1.0 | `classes.rs:61-63` | T | `classes.mountain_full_core`, `classes.flank_membership` | no |
| High mountains from/to | 1000 m, 5500 m | `classes.rs:65-66` | T | `classes.high_from_m`, `classes.high_to_m` (snow-line-like; an Earth value) | no |
| Class thresholds | hills 0.1..0.35, plateau 0.3..0.6, rolling 0.45..0.6, craton 500..1100 km, foreland 0.2..0.5, basin 200..700 m / 0.3..0.7, rift -0.15..-0.35 / 100..200 km, margin gate 0.15..0.4, convergence 0.05..0.35 and 0.7..0.95 | `classes.rs:137,392,439,455,564-570`; `lithology.rs:93-97` | T | `classes.thresholds` (one table of ramps, like `AridParams`) | no |
| Class relief driver k | `[1, 1, 5, 6, 7.5, 1, 2, 0.35]` | `classes.rs:30` | T | `classes.k` | no |
| Carve factor by class | `[1, 1, 1, 1, 1, 1, 1, 0.5]` | `classes.rs:33` | T | `classes.carve_factor` | no |
| Hill band | 30 + 145 sm(0,0.25) + (1000-175) sm(0.25,0.7) m; frequency 796; 3 octaves | `global.rs:319-329`; `classes.rs:558` | C/T | `relief.hill_band_max_m`, `relief.hill_band_freq` | no |
| Mountain band scale, centre | 4.5, 0.45 | `classes.rs:535-537` | C | `relief.mountain_band_scale`, `relief.mountain_band_centre` (SPIKE-101) | no |
| Global base shape | ocean -4500 m, land 1000 m, ridge 5200 m | `global.rs:372-378` | C | `relief.ocean_floor_m`, `relief.land_base_m`, `relief.ridge_m` | no |
| Min land | 1.0 m | `classes.rs:532` | S | none (a guard) | no |
| Shelf | break 130 m, abyss 4500 m, widths 150/30/90 km, active 150..450 km, convergence 0.05..0.35, blend 0.5..0.9, coast ramp 0.06, min sea 5 m | `shelf.rs:41-51` | E | already `ShelfProfile`; add to the world definition | no |
| Arid rules | soft 0.55..0.35, semi-arid 0.08..0.25 and 0.4..0.65, height ramps 100..400 and 1800..2800, arid 0.05..0.22, low 300..900, land -50..50, core 0.6, supply (1, 0.5, 0.4), caps 0.8 / 0.85 | `arid.rs:50-64` | T | already `AridParams` | no |

## 3. Continents
| Name | Value | file:line | Kind | Proposed parameter | Moves output? |
|---|---|---|---|---|---|
| Continent sizes | `[0.50, 0.26, 0.10, 0.06, 0.03, 0.02, 0.01, 0.01]` | `continents.rs:27` | E | `continents.sizes` | no |
| Lobes, turn, spacing, aspect | 3, 1.2, 0.6, 1.5..2.5 | `continents.rs:29-35` | T | `continents.*` | no |
| Shape noise | 1.6 freq, 8 octaves, gain 0.72, warp 0.6 | `continents.rs:37-40` | T | `continents.shape_*` | no |
| Smooth/coarse octaves, bias, craton warp, deep | 4, 3, 0.8, 0.3, 0.65 | `continents.rs:42-49` | T | `continents.*` | no |
| Islands | amplitude 1.3, freq 3.0, warp 0.3 | `continents.rs:52-54` | T | `continents.island_*` | no |
| Masses, largest | (4, 7), (0.47, 0.60) | `continents.rs:58-59` | E | `continents.mass_range`, `continents.largest_range` (Earth's distribution, SPIKE-91/92) | no |

## 4. Lithology and uplift
| Name | Value | file:line | Kind | Proposed parameter | Moves output? |
|---|---|---|---|---|---|
| Band nominal thickness, jitter | 100 m, 0.35 | `lithology.rs:27-29` | E | `rock.band_m`, `rock.band_jitter` | no |
| Caprock hardness, carbonate share | 0.92, 0.25 | `lithology.rs:34-36` | E | `rock.caprock_hardness`, `rock.carbonate_share` | no |
| Erodibility K soft/hard, exponent | 1.8, 0.12, 1.5 | `lithology.rs:38-39,174` | E | `rock.k_soft`, `rock.k_hard`, `rock.k_exponent` | no |
| Strata offsets | craton 150 m, orogen 1200 m | `lithology.rs:41-42` | E | `rock.offset_craton_m`, `rock.offset_orogen_m` | no |
| Tilt and fold frequency | 60, 120 | `lithology.rs:43-44` | T | `rock.tilt_freq`, `rock.fold_freq` | no |
| Band blend | 0.08 | `lithology.rs:46` | T | `rock.band_blend` | no |
| Hardness formula | 0.45 + 0.4 craton (1-basin) + 0.35 core - 0.4 foreland - 0.35 basin + 0.25 noise | `lithology.rs:239` | E | `rock.hardness_weights` | no |
| Soluble gate and fractions | 0.35..0.7, 0.6/0.8/0.3, -0.15..0.3 | `lithology.rs:257-258` | E | `rock.soluble_*` | no |
| Band offsets | 0.45 hard offset, 0.15 region | `lithology.rs:281-282` | T | `rock.band_hard_offset`, `rock.band_region` | no |
| K scale | 1.16 | `lithology.rs:325` | C | `rock.k_scale` (fixed in the Earth preset, ADR 0108 point 3; derived by `k_calib` over 12 seeds on the Earth mix of soft/hard) | no; hashed in `BakeInputs::id` |
| Uplift per province | craton 2, flat 24, rift 28, back-arc 32, belt 70; noise 0.2 at freq 3 | `uplift.rs:17-24` | E | `tectonics.uplift_u_over_k` (table), `tectonics.uplift_noise` (the numbers are U/K ratios fitted to the carve, not mm/yr) | no |

## 5. Carve, pass 1 and pass 2
| Name | Value | file:line | Kind | Proposed parameter | Moves output? |
|---|---|---|---|---|---|
| Uniform U/K | 20 | `carve.rs:47` | C | `CarveParams.u_over_k` (already) | no |
| k dt, steps, reroute | 0.05, 20 (pass 1 at 12,288), 5 | `carve.rs:48-50` | S | already `CarveParams` | no |
| Breach | slope 1e-3, max 500 m, minimum 50 m, scale 0 | `carve.rs:52-54`, `carve.rs:42` | C | already `CarveParams`; `BREACH_MIN_M` should join | no |
| Uplift taper | 60 m | `carve.rs:58` | C | already `CarveParams.uplift_taper_m` | no |
| Thermal angle of repose | 33 deg | `thermal.rs:33` | E | already `ThermalParams.theta_deg` (a rock property; world parameter) | no |
| Thermal perturb, k, passes | 0.25, 1/16 (pass 2: 1/8), 100 (pass 2: 300) | `thermal.rs:33`; `tile.rs:132` | S | already | no |
| Deposition | kc 1e5, zeta 0.5, local area 96 cells (1 km²), pit min 0.5 m, 32 passes | `tile.rs:133-138` | C | already `Pass2Params` | no |
| MFD exponent | 1.1 | `route.rs:31` | C | `routing.mfd_exponent` (const today) | no |
| Pit-spread rise | 0.8 depth over 8 | `deposit.rs:151-156` | S | none | no |
| Quantisation and codes | 0.25 m step, area steps 8, fill steps 4 per m, depth code 12 | `tile.rs:21,175`; `face.rs:152,165` | S | none (format) | no |
| Crease amplitudes | ocean 80 m, land 900 m, split 4, low -0.75, carve floor 0.15, from 0.01 to 0.2 | `erosion.rs:75-93` | T | `relief.crease_*` (runtime, not baked) | no |
| Crease octaves | 12 at 256, persistence 0.55 | `erosion.rs:70-73` | T | same | no |
| Coast fade | 3.0 m | `erosion_layer.rs:18` | T | `erosion.coast_fade_m` | no |

## 6. Runoff and vegetation (GEN-129)
| Name | Value | file:line | Kind | Proposed parameter | Moves output? |
|---|---|---|---|---|---|
| Budyko exponent n | 2.6 | `runoff.rs:9` | L | stays (Choudhury's form; n is a fitted soil/vegetation constant) | no |
| Reference runoff | 300 mm | `runoff.rs:11` | C | `runoff.ref_mm` | no; hashed in `constants_identity` |
| Weight bounds | 0.02, 8.0 | `runoff.rs:13-14` | C | `runoff.weight_min/max` | no |
| Vegetation peak, floor, peak value | 300 mm, 0.5, 1.5 | `runoff.rs:16-19` | E | `surface.veg_peak_mm`, `surface.veg_floor`, `surface.veg_peak` | no |

## 7. Other fixed constants (not Earth)
| Item | Where | Verdict |
|---|---|---|
| Hash layer ids (`LAYER_*`, `TAG_*`) | all modules | S: stay as literals |
| Lattice and cache sizes (`CLASS_LATTICE_N` 256, `OROGRAPHY_CELLS` 512, `LAND_CELLS` 64, `CHANNEL_CELLS`, `MAX_*`) | `classes.rs:36`, `orography.rs:20-22`, `relief_channels.rs`, `global.rs:487-561` | S |
| Reference (legacy) generator | `height.rs:26-29` (5 octaves, 4000 m), `terrain.rs:28-45` (detail 300 m, ocean 10 m, land 30 m) | Earth-like tuning of the legacy path; not read by the class-source bake; lift only if the legacy source stays |
| Crater stamp | `stamp.rs:39` (rim 0.85) | shape constant of the stamp stage; Moon-like worlds need it as a stamp parameter |

## Findings
1. **Three parameter blocks already exist** (arid, shelf, carve/pass-2/thermal) with the Earth value as `Default`; ADR 0108 point 1 holds for them. The world definition must hold them; today they are built inside the kernels' `default()`.
2. **The climate is the largest gap**: about 35 rows of Earth values sit as module consts or inline in `orography.rs` and `global.rs:405-423`. They are exactly the ADR's examples (rotation-driven wind bands, tilt-driven seasonal shift, lapse rate, the aridity law's PET numbers).
3. **The aridity law (PET) is in three places**: `orography.rs:355` is the one formula; `runoff.rs:22` and `arid.rs` inputs read it, so one parameter feeds all.
4. **One radius**: four copies of 6,371,000 m (`classes.rs:44`, `orography.rs:91`, `core`, the `BakeShape` field); the climate and class source read the constant, not the shape, so a non-Earth radius today changes the carve only.
5. **Calibration constants**: `ROCK_K_SCALE` (1.16), `BELT_AMPLITUDE_M`, the hill band, `RUNOFF_REF_MM` are Earth-preset calibration; per ADR 0108 point 3 they stay fixed constants of the preset, not recomputed.
6. **Identity**: `BakeInputs::id` hashes `ROCK_K_SCALE`, the versions and `runoff::constants_identity`; the arid and shelf values are in the class source's identity through their versions. Lifting a value with the same default must keep these hashes bit-identical, or the store re-keys (not a data change, but a rebake of any existing store). Test for it: `BakeInputs::id()` equal before and after the lift.
7. **Order of lifting** (proposal): (1) climate block (planet, wind, rain, lapse, PET) with the radius; (2) rock/uplift/tectonics tables; (3) class thresholds and relief; (4) runoff, vegetation, carve into the world definition. Each step: a test that the Earth preset gives the same `id()` and the same pinned checksums (`coupled.rs:742`, `coupledinput.rs:322`).

## Out of scope
- Runtime-only render code (ground detail, materials, `look` schema); ADR 0108 covers the generator. The render crates were not read.
- Values inside `crates/world-def` and `xtask` (formats, budgets).
