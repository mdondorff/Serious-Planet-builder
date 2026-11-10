# 0116 Constitution v1.7: rules for the simulation architecture
Status: accepted (owner, 2026-11-11: "adopt as suggested", from an external review of constitution v1.6)
Date: 2026-11-11
Source: external review of constitution v1.6 (owner, 2026-11-11; the PDF is not committed); spikes' ADR review pass (`spikes/prototype-gates/results/adr-review-pass.md` §1 row 1); A3 (`docs/design/terrain-target.md`); ADRs 0077, 0079, 0110-0115

## Context
- The constitution was written for a staged, tile-oriented generator. The prototype computes globally (H0), per landmass region (H1, K5 rounds), per catchment and per partition, iterates rounds and re-solves after edits.
- CON-08 required "a fixed grid with a bounded halo"; ADR 0113 point 7 called the regional solver a reading of it. A reading cannot supply a bounded halo (ADR review pass, row 1).
- CON-06 named `tile_id` as the computation identity; CON-07 and CON-14 implied GPU parity for authoritative fields, although the design is CPU-authoritative.
- Fresh/incremental equivalence and partition invariance were tested (G-EQ, G-PART, G-INC) but had no constitutional basis.
- The constitution takes precedence over accepted ADRs, so ADRs 0110-0115 could not be accepted while they conflicted with it.

## Decision
1. CON-05: canonical derived state is reproducible from its declared identities and may be reused when its complete dependency identity matches.
2. CON-06: the computation identity is general (planet, region, partition, catchment, tile, point); purity does not imply spatial locality.
3. CON-07: one selected implementation per authoritative field per cache identity; the CPU is canonical; versioned preview, production and reference implementations are allowed under distinct identities.
4. CON-08: stages declare dependencies and may run globally, regionally, over dynamic domains or partitions; iterative stages may read their own earlier iterations; spatial locality is demonstrated, not assumed.
5. CON-14: bit-exact CPU determinism covers all authoritative state and partitionings; the mechanism (`libm`) is policy, the outcome is the rule; parallel reductions are allowed when the result is scheduling-independent.
6. Added: CON-28 (authoritative simulation is CPU-only), CON-29 (incremental equals fresh build), CON-30 (partitioning does not change results), CON-31 (no weakening of requirements or tests to pass; reports separate run, unrun, unwritten and inferred results).
7. Refined: CON-04, CON-11, CON-13, CON-15, CON-18 to CON-23, CON-25 to CON-27 as in the review (runtime against build memory, VRAM named, adapter where relevant, ADR errata records, scripts as implementation details, Linux CI runs the determinism tests, provenance for code, assets and data).
8. The constitution's size cap (ADR 0079, BUILD-023) becomes 12,817 bytes, its size after this amendment.

## Consequences
- ADRs 0110-0115 no longer conflict with CON-08; ADR 0113 point 7's "reading" is replaced by a citation of CON-08 v1.7 in the architect's text pass (P0). Their acceptance stays with the owner.
- G-EQ, G-PART, G-DET and G-INC now verify CON-29 and CON-30; requirements cite them.
- CON-25 no longer forbids every script; `CLAUDE.md` keeps its stricter rule (no `.sh` or `.ps1` files) until the owner changes it.
- Not in the constitution, by the review's advice: hydrology architecture, routing choices, f32/f64 conversion points, exact budgets; they stay in ADRs and specs.
