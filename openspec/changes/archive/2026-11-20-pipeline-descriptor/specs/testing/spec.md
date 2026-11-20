## ADDED Requirements

### Requirement: TEST-142 The runtime height-field chain matches its declared catalog
A tier-A contract test SHALL instantiate `field_chain_erosion` across every `HeightFieldKind`, `ErosionSwitch` option, ADR 0007 stamp stack, and `WaterBounded` combination and SHALL assert that walking `HeightField::upstream()` matches `pipeline_catalog::resolve_runtime_chain`, while unsupported `--erosion` combinations are refused by both paths. A tier-A check SHALL also fail whenever `docs/stages/pipeline.json` drifts from code constants or preview renderers.

Verify: A
Status: active
Source: ADR 0021 point 7, BUILD-063, owner (2026-11-20)

#### Scenario: Walked chain equivalence and drift detection
- **WHEN** `field_chain_erosion` builds a stack over any `HeightFieldKind`, `ErosionSwitch`, ADR 0007 stamps, and `WaterBounded` setting
- **THEN** `walk_height_field` equals `resolve_runtime_chain`, unsupported erosion switches fail on both paths, and any drift in `docs/stages/pipeline.json` or preview outputs fails tier A
