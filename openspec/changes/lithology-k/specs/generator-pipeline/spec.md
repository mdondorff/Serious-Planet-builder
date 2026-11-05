## ADDED Requirements
### Requirement: GEN-116 The lithology field
An analytic lithology field SHALL give, at a direction and a height, the erodibility K a carve reads, a caprock flag and a soluble flag: CPU-canonical (`libm`, integer hashing), never stored, the same bits for any thread count, keyed by `LITHOLOGY_VERSION`. Its hardness SHALL be SPIKE-81's formula of the class source's plate-history terms, read from a lattice of the class lattice's geometry, plus noise. Its strata SHALL be bands 30 to 170 m thick with a hardness offset from the province's, tilted and folded by a smooth offset of the height; K SHALL be continuous in the height, a band above hardness 0.92 a caprock, and a carbonate band soluble where the province allows.

Verify: A
Status: active
Source: ADR 0102, SPIKE-102 F1, SPIKE-81, planner inbox #68 (2026-11-05)

#### Scenario: The field
- **WHEN** the field is built on 1 and 8 threads and read at random points and heights, its hardness is compared with SPIKE-81's formula, a column is scanned over 1.5 km in steps of a quarter metre, and the lattice is read at its nodes and across a face edge
- **THEN** the bytes agree for any thread count and differ with the seed, H and K stay in their ranges, the bands are 30 to 170 m thick, K steps by under 0.4 between samples, the stack's offset varies over the planet, the caprock flag follows the hardness, and the lattice reads the class source's terms within one code

### Requirement: GEN-122 The lithology preview
`generate --lithology-preview DIR [--seed N | --world F] [--km N]` SHALL write, with no GPU, equirectangular maps of the hardness, of K at the surface (caprock black) and 200 m below it, and of the soluble flag, a vertical cross-section through a mountain belt into a basin (K, caprock, the surface on top), and `lithology.txt` with the shares, the section's place and the band thickness against pass 1's per-step height change. The text and the run's line SHALL say the images show the lithology field, an input, not terrain; the bytes SHALL not depend on the thread count.

Verify: A
Status: active
Source: ADR 0102 point 4, planner inbox #68 and #69 (2026-11-05)

#### Scenario: The preview
- **WHEN** the preview is made on 1 and 4 threads, written to a folder, and the option is combined with another generate command, a definition named twice, and `--km` alone or out of range
- **THEN** the images and the text agree for any thread count, the six files exist, the text carries the label, the shares and the band thickness, and each refusal names the option
