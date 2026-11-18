## Why
- Reading 28 (lead #170, 2026-11-18): the budget's relief field is computed on `S0`; the lowering it limits is measured against `S_KH`. #1599's `--reference` computed the field from `S_KH`.
- Lead #169 (owner, 2026-11-18): every run's `chain.txt` names its commit, switches and stage forms, and the gate table prints it per row.

## What changes
- BUILD-060: the budget field always from the gate's start surface; `--reference`, `#reference=` and the `b_reference` column removed.
- BUILD-060: `#chain=FILE` per table row: the row's label from the chain, and a `Chains` section under the markdown table.
- BUILD-060: the chain sheet's manifest keeps `commit`, `switch` and `stage` lines; the commit is printed in the title band.

## Impact
cache (`river_gate`, `river_gate_graph`, the `river_gate` example, `stage_sheet`), xtask (`rivergate.rs`, help); tests.
