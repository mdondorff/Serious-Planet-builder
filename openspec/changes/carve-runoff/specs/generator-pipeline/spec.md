## ADDED Requirements
### Requirement: GEN-129 The carves read the runoff as discharge and the vegetation as a local erodibility factor
Both carves SHALL take an optional runoff weight per cell and a vegetation factor in their erodibility.
- The runoff of a place SHALL be `R = P - E` in mm a year, `E = P PET / (P^n + PET^n)^(1/n)`, `n` 2.6 (`BUDYKO_N`), `P` the annual precipitation and `PET` the potential evapotranspiration of the orographic climate (GEN-117); `R` SHALL be 0 for `P = 0`, increase with `P` and never exceed `P`.
- The weight of a cell SHALL be `w = clamp(R / RUNOFF_REF_MM, WEIGHT_MIN, WEIGHT_MAX)` with `RUNOFF_REF_MM` 300, `WEIGHT_MIN` 0.02, `WEIGHT_MAX` 8.
- A routing given weights SHALL start each cell with the area `w dx^2` and share it by the multiple flow direction as before: the area it returns is the discharge `Q = sum(w dA)`, equal to the area for weights of 1; the total at the outlets SHALL be the sum of the weighted areas.
- The carve's stream-power number SHALL be `k_dt K sqrt(Q) / dx`; a cell below a dry reach SHALL keep the discharge of the wet catchment above it. The area cap of pass 2 applies to `Q`. This amends GEN-110: the stored area channel of a bake with weights holds `round(8 log2(Q / cell area))`, an effective drainage area in cells of reference runoff; a cell whose `Q` is under one cell has code 0 (a desert cell reads as one cell) and a wet cell reads up to `log2(8)` = 3 steps of 8 (24 codes) above its area. Readers of the channel (the wetness driver of GEN-119, `drivers::AREA_LOG2_DRY` and `AREA_LOG2_WET`) read discharge; the channel is keyed through the `rock` byte, so no record of a bake without weights is read as one with them.
- The vegetation factor SHALL be `V_FLOOR + (V_PEAK - V_FLOOR) u e^(1-u)`, `u = P / VEG_PEAK_MM`, with `VEG_PEAK_MM` 300, `V_FLOOR` 0.5, `V_PEAK` 1.5: `V_PEAK` at `u = 1`, `V_FLOOR` at `P = 0` and for `P` much larger; `BakeInputs::climate_k` SHALL multiply the lithology's K by it.
- The `rock` key byte SHALL carry bit 2 (weights) and bit 3 (vegetation); `BakeInputs::id` SHALL fold `RUNOFF_VERSION` and a digest of the constants (`runoff::constants_identity`, pinned by a test). With no weights and no vegetation the carves SHALL be those of GEN-127 bit for bit.
- Every Earth-specific value of the runoff and the vegetation SHALL be a named constant of `generators::runoff` with Earth as the default (ADR 0108); the PET law is the orographic climate's (GEN-117), which `pet_mm` reads.

Verify: A
Status: active
Source: ADR 0104 point 3 (amended), ADR 0108, GEN-117, GEN-127, GEN-110

#### Scenario: The runoff in the carve
- **WHEN** the runoff, the weight and the vegetation factor are evaluated over a grid of `P` and `T`, a routing is run with weights of 1, with a wet catchment above a dry reach and with random weights, a cone is carved with wet and dry weights, with weights of 1 and with none, a face is carved with a halo and a tile with weights read at the cells' directions, the climate erodibility and weight of a bake are read on 1 and 8 threads, and a store written with other bits is run
- **THEN** the runoff has the limits above, the weighted area equals the unweighted for weights of 1 bit for bit and its outlet total is the weighted sum, the discharge below the dry reach is at least the wet catchment's, the dry ramp incises less than the wet one, the bytes agree for any thread count, and a mismatch of the key bits is refused
