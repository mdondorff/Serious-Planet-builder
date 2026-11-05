## ADDED Requirements
### Requirement: GEN-123 The uplift field
An analytic uplift field SHALL give, at a direction, the uplift of a carve step in units of `k_dt` (the ratio `U / K` at K 1): CPU-canonical (`libm`, integer hashing), never stored, the same bits for any thread count, keyed by `UPLIFT_VERSION`. It SHALL be high in the active belts and the arc coasts, medium in the back-arc, about 0 on the cratons, from the province lattice of GEN-116 and a low-frequency noise. A consumer SHALL fold `UPLIFT_VERSION` into its keys.

Verify: A
Status: active
Source: planner inbox #70 (owner, 2026-11-05), ADR 0102, GEN-116

#### Scenario: The field
- **WHEN** the field is built on 1 and 8 threads and read at random points, at the province terms of a belt, a back-arc, a craton and a flat, and across a face edge
- **THEN** the bytes agree for any thread count and differ with the seed, the value is highest in a belt and lowest on a craton, the back-arc lies between, it is continuous across a face edge and finite and not negative everywhere

### Requirement: GEN-125 The uplift preview
`generate --uplift-preview DIR [--seed N | --world F] [--km N]` SHALL write, with no GPU, equirectangular maps of the uplift and of the uplift over the lithology's K at the surface, a section through a belt over a back-arc to a craton (the uplift and the base elevation), and `uplift.txt` with the land mean and quantiles and the mean by province. The text and the run's line SHALL say the images show the uplift field, an input, not terrain; the bytes SHALL not depend on the thread count.

Verify: A
Status: active
Source: planner inbox #72 (owner, 2026-11-05), GEN-123

#### Scenario: The preview
- **WHEN** the preview is made on 1 and 4 threads, written to a folder, and the option is combined with another generate command, a definition named twice, and `--km` alone or out of range
- **THEN** the images and the text agree for any thread count, the four files exist, the text carries the label, the mean and the shares, and each refusal names the option
