## MODIFIED Requirements

### Requirement: WDEF-018 The chunk stage setting in the definition
The `chunk_stage` settings of the layers (GEN-039) that differ from the default `under` SHALL enter the definition hash after the items. A definition with items SHALL also hash the chain-order era (`CHAIN_ORDER_ERA`, 2 since `under` became the default), so a world made under the `on-top` default has another key. A definition without items and without settings SHALL hash as it did before. The settings are set in code and are not carried by the TOML file yet. The setting acts on no build since ADR 0086; it is read into the hash so that the hash of a stored world does not change.

Verify: A
Status: active
Source: GEN-039, WDEF-003, m8-chunk-version

#### Scenario: Hash
- **WHEN** a layer is set to `on-top` and back to `under`
- **THEN** the hash differs while it is `on-top` and equals the plain definition's hash again afterwards
