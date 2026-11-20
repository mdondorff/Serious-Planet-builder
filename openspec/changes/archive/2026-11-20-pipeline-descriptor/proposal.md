## Why
- Provide a single, code-truthful pipeline interface before building the offline `cargo xtask cockpit` HTML debugger (`BUILD-064` in follow-up PR 2), avoiding prose drift between documentation (`docs/design/terrain-target.md`), feature toggles (`--erosion`, `--channels`, `--commit`, `world.height_field`, turned lattices, ADR 0007 stamp stages), and live version constants.
- Establish an initial Global Authoring debug-companion foundation (design overview and review notes in [docs/design/pipeline-cockpit.md](../../../docs/design/pipeline-cockpit.md)).

## What changes
- `BUILD-063`: Add `planet_generators::pipeline_catalog` (`SCHEMA_FORMAT_VERSION = 2`) with K-labelled (`K1`..`K11`, `M9-P1`/`M9-P2`), `StageScope`-tagged (`global`, `region`, `tile`) stages separating M9 layer passes from K5-K7 commit stages (including K5 `window_cut`) and exhaustively binding `bake::probe::Stage` via `stage_id_for_probe`, plus `planet generate --pipeline-json OUT [--seed N | --world F] [--erosion on|off] [--channels on|off] [--commit DIR]` emitting `base_node_hash`, `definition_hash`, `active_stages`, `walked_chain`, and the embedded catalog.
- `TEST-142`: Add Tier-A contract tests verifying `HeightField::upstream()` equivalence against `field_chain_erosion`, 1-to-1 `probe::Stage` and `docs/stages/manifest.txt` hook parity, live `openspec/specs/` resolution of every cited `spec_ids`, preview filename parity, deterministic generation of `target/review/stages/pipeline.json`, and absence of any committed `docs/stages/pipeline.json`.
- Non-normative companion brief: `docs/design/pipeline-cockpit.md` explaining the two-PR split and recording `opus-architect`'s review decisions.

## Impact
`planet-generators` (`bake::probe::Stage::ALL_KINDS`/`kind_id`, new pure-metadata module `pipeline_catalog`, Tier-A test `pipeline_catalog`), `planet` CLI (`generate --pipeline-json`, `--commit` support), `docs/design/pipeline-cockpit.md`, `openspec/specs/`. Zero impact on generator output bits, cache keys, `xtask stages`, or GPU rendering.
