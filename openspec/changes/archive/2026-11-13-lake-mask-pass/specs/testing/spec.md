## ADDED Requirements

### Requirement: TEST-133 The lake mask scene
A tier-C scene SHALL draw the aerial coast's water as a lake with a mask wet everywhere, and with a mask of diagonal stripes 8 cells wide, in the class view, and check: the full mask changes no pixel against the sea; the striped mask's water pixels are a subset of the sea's and between a fifth and four fifths of them; the striped picture is a golden per adapter.
- The first lake mask golden goes to the owner's /bless (ADR 0017); until then it is listed in `tests/goldens/pending.txt`.

Verify: C
Status: planned
Source: Lead 2026-11-13 (E1), REND-065, ADR 0017

#### Scenario: Stripes
- **WHEN** the coast is drawn as a lake with the striped mask
- **THEN** the water ends at the stripes' edges and is a strict part of the sea's
