# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives on its spike branch and is never merged into the main crates as-is. **Each spike's report, numbers and renders live in `spikes/<name>/results/`** (`results.md` with a Visuals section; the `spike` skill has the rule). `docs/spikes/` holds only the frozen images of SPIKE-03 to 07.

Planned spikes from the background report still to open: city HLOD with 10^5 buildings (with Cities).

Per-spike register sections of M5 and M6: `docs/archive/M6/spikes.md`. Closed spikes up to M7 (SPIKE-08 to 32 and the explorations): `docs/archive/M7/spikes.md`. Closed spikes of earlier milestones: `docs/archive/M0-M3/spikes.md` (SPIKE-01 to 04), `docs/archive/M4/spikes.md` (SPIKE-05 to 07). SPIKE-10 was never used.

## Index of open and M8 spikes
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-36 | Co-change analysis across the crates and specs since M4: PR width, co-change matrix, the widest PRs, recurring concepts, churn hotspots and the built dependency graph against CON-02 (architecture review before M8; opus-planner, 2026-10-23; owner asked) | closed 2026-10-23 (facts and tables, no judgement) | `spikes/architecture-review/results/` |
| SPIKE-34 | A block stage in the engine: streaming, cache key, plan bounds, edits and memory for a regional field per block of a fixed grid (ADR 0077; opus-planner, 2026-10-23; owner asked) | closed 2026-10-24 (a block field fits streaming, the cache key and the plan with a level gate, a band-limited delta and plan-side requests; blocks of 5 km at about 6 s each are the size that works; edits wait seconds, not milliseconds) | `spikes/block-stage/results/` |
| SPIKE-33 | Landscape evolution over time: does coupled stream-power erosion and drainage (a coarse global run plus capped fine steps per fixed block) fit the stage model and beat droplet erosion and erosion noise in look and cost (M8 stage model; opus-planner, 2026-10-23; owner asked) | closed 2026-10-24 (dynamic coupled evolution has no bounded light cone: p99 height error 28 m at N = 30 and a 5 km halo, border flux and heights are not the cause; a frozen coarse network (A-net) gives exact block seams at 0.06 s per km2 and 4 m cells, 7x cheaper; open: A-net halo below 5 km, the breach rule, engine renders) | `spikes/landscape-evolution/results/` |

Every exploration gets a `SPIKE-NN` entry.
