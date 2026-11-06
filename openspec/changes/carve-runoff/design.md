## Context
- ADR 0104 point 3 as amended: runoff enters the carve as discharge `Q = sum(runoff dA)` over the routing, not as a local scaling of K; the vegetation curve stays a local factor.
- The orographic climate gives `P(d)` (annual mm, two seasons summed) and `T(d)` (deg C at the base elevation); PET is `300 + 45 T` mm, at least 150 (GEN-117, SPIKE-81).

## Decisions
1. Runoff is precipitation minus evapotranspiration, not a fixed ratio: a ratio would give a desert and a rain forest the same share. Evapotranspiration follows Budyko's curve in Choudhury's (1999) form `E = P PET / (P^n + PET^n)^(1/n)` with `n` 2.6: `E` tends to `P` where `P << PET` (no runoff in a desert) and to `PET` where `P >> PET`.
2. The weight is `w = clamp(R / R_ref, W_MIN, W_MAX)`: `R_ref` 300 mm a year is Earth's mean continental runoff (about 40,000 km3 a year over 1.5e8 km2), so a mean land cell has weight about 1 and the stream-power number keeps its scale; `W_MIN` keeps a desert cell's own area non-zero.
3. `Q` replaces the area in the carve's `f`: the routing starts each cell with `w dx^2`, so the discharge is a sum along the flow paths and a wet river keeps its power across a dry reach. The area cap of pass 2 applies to `Q`.
4. The vegetation factor is `v(P) = V_FLOOR + (V_PEAK - V_FLOOR) u e^(1-u)`, `u = P / P_PEAK`: 1 at the peak value `V_PEAK` (1.5) at `P_PEAK` 300 mm (the peak of sediment yield, Langbein and Schumm 1958), `V_FLOOR` (0.5) for bare desert and dense cover. It multiplies K in the caller's erodibility closure (`climate_k`), so the carve's `k` pass reads it with K.
5. Keys: bit 2 (runoff weights) and bit 3 (vegetation) of the `rock` byte; `RUNOFF_VERSION` and the constants in `BakeInputs::id` (the orography's version is already in it through `climate_id`).
6. The stored area channel holds the discharge `Q / dx^2` of a bake with runoff (an effective drainage area at reference runoff); readers of the channel see wet rivers as larger.

## Earth-specific values (ADR 0108 point 4)
| Name | Default | Unit | Source | Held by |
|---|---|---|---|---|
| `BUDYKO_N` | 2.6 | 1 | calibration (Choudhury 1999 fit to catchments) | `runoff::BUDYKO_N` |
| `RUNOFF_REF_MM` | 300 | mm a year | Earth state (mean continental runoff) | `runoff::RUNOFF_REF_MM` |
| `WEIGHT_MIN`, `WEIGHT_MAX` | 0.02, 8 | 1 | numeric bounds | `runoff` |
| `VEG_PEAK_MM` | 300 | mm a year | Earth state (Langbein and Schumm 1958) | `runoff::VEG_PEAK_MM` |
| `VEG_FLOOR`, `VEG_PEAK` | 0.5, 1.5 | 1 | calibration (start values) | `runoff` |
| PET law | `300 + 45 T`, at least 150 | mm | law with Earth's constants (SPIKE-81) | `orography::aridity_index` |
The constants are module constants until the audit of ADR 0108 point 5 lifts them into the world definition.
