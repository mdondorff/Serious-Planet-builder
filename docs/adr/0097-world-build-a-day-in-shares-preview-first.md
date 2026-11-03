# 0097 World build: one day in shares, and a preview before every full run
Status: accepted (owner, 2026-11-03: terms "work out", shares "make sense", "All 'day long builds' will require previews"); the share values of point 2 are proposed until the owner confirms them
Date: 2026-11-03
Source: owner, 2026-11-03; SPIKE-95 slot results (`spikes/perf-window-process/results/slot/results-2026-11-03.md`); GEN-014

## Context
- The offline run from a WorldDefinition to a store has several stages (field bake, erosion bake passes 1 and 2, wet floor, coupled run, pre-fill) and takes hours; the M9 field bake alone took 1020 s on 20 threads, 2.76x its 2026-10-12 cost.
- GEN-014 limited the field bake alone to ten minutes: a stage limit says nothing about the whole run and refuses a costlier generator that the rest of the run could afford.
- Later offline layers (fauna, instancing, cities) need time from the same day.
- A day-long run that fails or looks wrong late costs a day of the reference machine.
- "Global bake" read as the whole offline run; it names one stage.

## Decision
1. The **world build** is the whole offline run from a WorldDefinition to a store; it finishes within 24 h on the reference machine.
2. The day is split into shares: terrain 40 %, future content (fauna, instancing, cities) 40 %, margin 20 %. The share values live in BUDG-014.
3. Each stage's wall time is measured and reported against its share; a stage has no time limit of its own. GEN-014's ten-minute limit is retired; its memory limit stays.
4. A world-build algorithm that also runs at runtime stays within the runtime budgets (flight speed, BUDG-001 to BUDG-003).
5. Every stage offers a preview mode (a coarse planet or a region). A full world build starts only after the owner approves its preview.
6. Terms: world build; field bake (was "global bake"); erosion bake (passes 1 and 2); pre-fill; wet floor; store (the result). "Global bake" is retiring (`docs/domain.md`).

## Consequences
- A slower generator is acceptable while terrain's share holds.
- The perf harness's `global-bake` scene reports against BUDG-014 instead of failing at 600 s (a code change: `crates/app/src/globalperf.rs`).
- Identifiers, flags and comments that say "global bake" are renamed in the PRs that touch them (domain.md section 7).
