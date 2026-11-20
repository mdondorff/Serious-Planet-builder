## ADDED Requirements

### Requirement: BUILD-063 The code-generated pipeline catalog
`planet_generators::pipeline_catalog` SHALL publish the version pins, curated stages, preview command contracts, and runtime switch matrix of the terrain and hydrology pipeline as pure, non-authoritative metadata (`CON-06`, `CON-07`), serialized deterministically to `docs/stages/pipeline.json`. `cargo xtask stages [--update]` SHALL verify or regenerate both `docs/stages/manifest.txt` and `docs/stages/pipeline.json`. `planet generate --pipeline-json OUT` SHALL resolve a `--seed` or `--world` definition (with optional `--erosion on` and `--channels on`) into its live `HeightField` stack and write the resolved chain JSON.

Verify: A
Status: active
Source: owner (2026-11-20, Global Authoring & pipeline debugger MVP), ag2-antigravity, BUILD-047, TEST-142

#### Scenario: Catalog sync and CLI resolution
- **WHEN** `cargo xtask stages` runs against a stale `docs/stages/pipeline.json`, or `planet generate --pipeline-json OUT` runs with `--seed`, `--world`, `--erosion on --channels on`, or clashing flags
- **THEN** a stale `pipeline.json` fails naming `cargo xtask stages --update`, `--pipeline-json` emits valid JSON naming the seed/definition and active `implementation_id`s, and clashing options are refused
