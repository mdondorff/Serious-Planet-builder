## ADDED Requirements

### Requirement: TEST-142 The runtime height-field chain matches its declared catalog
Tier-A contract tests SHALL instantiate `field_chain_erosion` across every `HeightFieldKind`, `ErosionSwitch` option (both M9 layer and K5-K7 commit), ADR 0007 stamp stack, and `WaterBounded` combination and SHALL assert that walking `HeightField::upstream()` equals `pipeline_catalog::resolve_runtime_chain`, while unsupported `--erosion` combinations fail on both paths. Tier-A checks SHALL also verify live `openspec/specs/` resolution of every catalog `spec_ids`, 1-to-1 `probe::Stage` and `docs/stages/manifest.txt` hook parity, preview filename parity, deterministic emission to `target/review/stages/pipeline.json`, and absence of a committed `docs/stages/pipeline.json`.

Verify: A
Status: active
Source: ADR 0021 point 7, BUILD-063, owner (2026-11-20)

#### Scenario: Walked chain equivalence and catalog contracts
- **WHEN** `field_chain_erosion` builds a stack or `pipeline_catalog` exports its schema, probe hooks, and cited spec IDs
- **THEN** `walk_height_field` equals `resolve_runtime_chain`, every `probe::Stage` and cited `spec_id` resolves cleanly, `target/review/stages/pipeline.json` is written deterministically, and no committed JSON copy exists
