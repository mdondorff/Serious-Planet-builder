# 0108 Earth is the default preset, not the model: laws in kernels, planet constants as named parameters
Status: accepted (owner, 2026-11-05: "earth is the default preset is the right perspective"; "separating laws from constants is the way to go")
Date: 2026-11-05
Source: owner, 2026-11-05; ADR 0045 (bodies, proposed); ADR 0089 (drivers, not finished types); ADRs 0101-0107 (M9 drivers and processes)

## Context
- M9's generator is tuned against GLO-30 reference tiles; tuned values become Earth's values.
- Non-Earth worlds (other rock, climate, rotation, sea level) must stay creatable; a later editor sets their properties.
- Earth constants already sit inside kernels (wind bands from Earth's rotation, a 12 degree seasonal shift from Earth's tilt, the aridity formula's constants, a glacial cooling of about 6 degrees C, stratum thicknesses).
- A normalisation computed per world (for example rescaling rock erodibility to a mean) would undo a world's intended difference.
- No test runs a world far from Earth, so a process valid only near Earth's values goes unnoticed.

## Decision
1. Generator kernels hold physical laws; every Earth-specific value (planet state, calibration constant, class threshold) is a named parameter of the world definition, with Earth's value as the default preset.
2. Tuning against reference data sets defaults; it never adds a literal inside a kernel.
3. Calibration factors are constants of the default preset (fixed with the version that derived them), never recomputed per world.
4. Every generator design note lists its Earth-specific values: name, default, unit, source (law, Earth state or calibration) and the parameter that holds it.
5. The values already on main are audited and listed; lifting them into parameters follows the list.
6. The scorecard runs off-Earth presets, untuned, and checks that each process responds in the expected direction (softer rock, more erosion; colder, more ice) without failures or artefacts.

## Consequences
- A world definition grows a parameter set; changing it means a re-bake of that world.
- The editor of world properties is later work (ADR 0045); this keeps it possible.
- Reviews of generator changes (planner, numerics-reviewer) check point 1 and point 4.
