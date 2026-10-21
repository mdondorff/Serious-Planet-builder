## ADDED Requirements

### Requirement: LOOK-009 The strength of the ground detail
The lit plan SHALL carry the strength of the procedural ground detail (REND-034) as a number from 0 to 1, default 0 (off). The strength SHALL be written in the plan's text only when it is not 0 (so a plan made with the default keeps its text and hash), SHALL round-trip exactly, and SHALL be refused unless it is 0 or from 1e-6 to 1 (a smaller one differs between adapters that flush subnormals), naming the key. Presentation data (ADR 0035).

Verify: A
Status: active
Source: M7 G2a (opus-planner, 2026-10-21), SPIKE-27, ADR 0035

#### Scenario: The default and a set strength
- **WHEN** a lit plan has the default strength, and one has the strength 0.5
- **THEN** the first writes no `detail` token and parses back equal; the second writes one, parses back equal and writes the same text again; a strength of 1.5 or NaN is refused naming `ground_detail`
