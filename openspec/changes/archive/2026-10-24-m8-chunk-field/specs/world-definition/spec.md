## ADDED Requirements

### Requirement: WDEF-018 The chunk stage setting in the definition
The `chunk_stage` settings of the layers (GEN-039) that differ from the default `on-top` SHALL enter the definition hash after the items; a definition without such a setting SHALL hash exactly as it did before. The settings are set in code and are not carried by the TOML file yet.

Verify: A
Status: active
Source: GEN-039, WDEF-003

#### Scenario: Hash
- **WHEN** a layer is set to `under` and back to `on-top`
- **THEN** the hash differs while it is `under` and equals the plain definition's hash again afterwards
