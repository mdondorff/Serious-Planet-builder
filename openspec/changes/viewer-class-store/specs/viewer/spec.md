## ADDED Requirements
### Requirement: VIEW-065 The viewer shows the world a store records
The viewer, `generate --plan-check` and `generate --coarse-horizon` SHALL open the world a store records (STRM-073): a class-source store as the class world, a legacy store as the legacy one, a store of both bakes only with `--class-source` or `--legacy-source` (refused naming both bakes otherwise). The viewer's title SHALL name the field source (`class source` or `legacy source`) and, with the erosion layer on, `erosion layer`; the log of the run SHALL print the description.

Verify: A
Status: planned
Source: Owner 2026-11-04 (the integrated preview: the viewer must show the class-source world), STRM-073, STRM-074, inbox #59

#### Scenario: The recorded world
- **WHEN** a class-source store, then a store of both bakes, are opened by the viewer's terrain, by the plan check and by the coarse horizon, with and without the options
- **THEN** the class store opens as the class world and says `class source`, the store of both is refused naming both bakes and opens with either option, the plan check and the horizon do not find "no bake" in a class store, and the title's world label names the source and the layer (the drawn title is tier D)

#### Scenario: The layer by its manifest
- **WHEN** a layer with a manifest is opened over the class store, with a manifest of another kernel version, with one of another field source, and with a replacement of the pass-1 cells
- **THEN** the layer opens without a marker flag, says it came from its manifest and bounds by the manifest's range, the other two are refused naming the version and both sources (also with `--erosion-unchecked` on a legacy world), and the replaced cells are refused by the layer's keys
