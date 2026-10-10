## MODIFIED Requirements

### Requirement: TEST-002 Per-adapter goldens, owner-only blessing
Golden images SHALL be stored per adapter under `tests/goldens/<adapter>/`; a test SHALL never create or overwrite a golden; when a golden is missing the test SHALL write a candidate to `target/review/candidates/<adapter>/` and report it as pending (failing in strict mode, and always when the adapter already has blessed goldens) unless its name is listed in `tests/goldens/pending.txt`; debug views SHALL compare exactly.

Verify: A
Status: active
Source: CON-18, ADR 0009

#### Scenario: Missing golden
- **WHEN** a golden test runs without a golden for the adapter
- **THEN** a candidate PNG is written, no file appears under `tests/goldens/`, and the test output says `PENDING BLESS`

#### Scenario: Strict mode
- **WHEN** `PLANET_GOLDEN_STRICT=1` and a golden is missing
- **THEN** the test fails

#### Scenario: Blessed adapter
- **WHEN** a golden is missing for an adapter that already has at least one blessed golden
- **THEN** the test fails even without strict mode

#### Scenario: Waiting for blessing
- **WHEN** a golden is missing but its name is listed in `tests/goldens/pending.txt`
- **THEN** the test reports it as pending and passes, and the line is removed when the owner blesses the golden

