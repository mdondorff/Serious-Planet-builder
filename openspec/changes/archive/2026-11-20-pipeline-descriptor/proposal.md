## Why
- Provide a single, code-truthful pipeline interface before building the offline `cargo xtask cockpit` HTML debugger (`BUILD-064` in follow-up PR 2), avoiding prose drift between documentation (`docs/design/terrain-target.md`), feature toggles (`--erosion`, `--channels`, `world.height_field`, turned lattices, ADR 0007 stamp stages), and live version constants.
- Establish an initial Global Authoring debug-companion foundation (design overview and review questions in [docs/design/pipeline-cockpit.md](../../../docs/design/pipeline-cockpit.md)).

## What changes
- `BUILD-063`: Add `planet_generators::pipeline_catalog`, commit generated `docs/stages/pipeline.json`, include `pipeline_catalog` in `cargo xtask stages [--update]`, and add `planet generate --pipeline-json OUT [--seed N | --world F] [--erosion on|off] [--channels on|off]`.
- `TEST-142`: Add Tier-A contract tests verifying `HeightField::upstream()` equivalence against `field_chain_erosion` across all switches and catching catalog or preview file drift.
- Non-normative companion brief: `docs/design/pipeline-cockpit.md` explaining the two-PR split and review questions for `opus-architect`.

## Impact
`planet-generators` (new pure-metadata module `pipeline_catalog`, Tier-A test `pipeline_catalog`), `planet` CLI (`generate --pipeline-json`), `xtask` (`stages`), `docs/stages/pipeline.json`, `docs/design/pipeline-cockpit.md`, `openspec/specs/`. Zero impact on generator output bits, cache keys, or GPU rendering.
