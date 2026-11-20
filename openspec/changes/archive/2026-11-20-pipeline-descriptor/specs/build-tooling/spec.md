## ADDED Requirements

### Requirement: BUILD-063 The code-generated pipeline catalog
`planet_generators::pipeline_catalog` SHALL publish version pins, K-labelled scoped (`global`, `region`, `tile`) pipeline stages, separated M9 layer and K5-K7 commit erosion stages (including K5 `window_cut`), exhaustive `bake::probe::Stage` bindings (`probe_hooks`), preview command contracts, and runtime switch predictions as pure, non-authoritative metadata (`CON-06`, `CON-07`). All cited `spec_ids` SHALL resolve live in `openspec/specs/`, without committing a derived JSON file or changing `cargo xtask stages`. `planet generate --pipeline-json OUT` SHALL resolve a `--seed` or `--world` definition (with `--erosion`, `--channels`, or `--commit DIR`) into its live `HeightField` stack and emit JSON holding `base_node_hash`, `definition_hash`, `active_stages`, `walked_chain`, and the embedded catalog.

Verify: A
Status: active
Source: owner (2026-11-20, Global Authoring & pipeline debugger MVP), ag2-antigravity, BUILD-047, TEST-142

#### Scenario: CLI resolution across erosion and commit modes
- **WHEN** `planet generate --pipeline-json OUT` runs with `--seed`, `--world`, `--erosion on --channels on`, `--commit DIR`, or clashing flags
- **THEN** `--pipeline-json` emits valid JSON naming `base_node_hash`, `definition_hash`, `erosion_mode`, active `implementation_id`s, and the embedded catalog, and clashing options are refused
