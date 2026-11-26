# River graphs across scales --- G0 contract addendum and revised milestones

**Status:** Proposed addendum to draft 3 (2026-11-16)\
**Purpose:** Retire the highest-risk architectural assumptions before
implementing the integrated RG-2 chain.\
**Scope:** This addendum supplements the existing plan. It does not
select RG over B, change the prototype's algorithmic direction, or
authorise production ADRs.

## 1. Decision and working principle

Proceed with RG-2 as the fuller prototype, retaining B (the L-round
approach) as the measured benchmark and fallback. Before implementing
the integrated chain, establish executable contracts for graph identity,
basin semantics, rasterisation, terrain-change accounting, partition
invariance and incremental invalidation.

The programme should distinguish:

-   **A component working:** an isolated stage satisfies its own
    contract.
-   **The chain working:** the composed RG-2 pipeline satisfies the
    integrated gates.
-   **The architecture being selected:** RG demonstrates sufficient
    quality, preservation, determinism, editability and cost to justify
    production adoption.

Passing an earlier level does not imply passing a later one.

## 2. G0 --- contracts and test harness

G0 is a small architecture-and-test milestone, not a terrain-algorithm
implementation phase. It produces a concise contract note, executable
validators, and a minimal synthetic fixture.

### 2.1 Canonical graph model

Define the minimum canonical graph entities:

  -----------------------------------------------------------------------
  Entity                              Required semantics
  ----------------------------------- -----------------------------------
  Node                                Stable identity; node kind;
                                      location; optional
                                      elevation/profile constraint;
                                      domain reference

  Directed reach                      Stable identity; upstream and
                                      downstream node IDs; ordered
                                      geometry; direction; owning/storage
                                      domain

  Junction                            Explicit connection between
                                      incoming and outgoing reaches; no
                                      implicit connection based solely on
                                      raster-cell overlap

  Outlet                              Explicit terminal or receiving
                                      connection, including
                                      external/out-of-domain outlet
                                      semantics

  Exceptional reach                   Explicit status for D-2 or other
                                      unresolved profile/terrain
                                      conditions; never silently repaired

  Basin node                          Stable identity; parent/child
                                      relationship; spill point/edge and
                                      spill elevation; active outlet
                                      state
  -----------------------------------------------------------------------

At minimum, node kinds should distinguish source, confluence, outlet,
lake inlet/outlet, and exceptional profile points. The implementation
may use a more compact representation, but it must preserve these
semantics.

**Identity rule:** graph identity is independent of tile ownership,
raster-cell identity, and storage layout. Ownership determines where
data is stored or computed; it does not define which river feature it
is.

**Topology/attribute rule:** topology changes (node/reach creation,
deletion, split, merge, reattachment) must be distinguishable from
attribute changes (discharge, bed profile, order, or other derived
values). A topology diff must be observable.

### 2.2 Reach output contract

Retain the proposed KG-trib output fields, but make the contract
explicit for all graph producers:

-   stable reach ID;
-   upstream and downstream node IDs;
-   ordered geometry and direction;
-   drained area `A`, with its meaning and units;
-   discharge `Q`, with its meaning, units, and accumulation rule;
-   Strahler order and the rule used to derive it;
-   bed profile, including sample positions and endpoint treatment;
-   owning/storage domain;
-   exceptional status and diagnostic reason, if applicable.

Specify whether `A` is geometric contributing area, runoff-weighted
area, or another quantity. Specify how diffuse runoff enters the channel
graph and how area/discharge are conserved at confluences. Do not use
the same field name for quantities with different semantics.

### 2.3 Basin contract

For the prototype, basin semantics are topological, not a full
water-balance simulation.

Each basin node must identify:

-   stable basin ID;
-   depression/floor representation;
-   parent basin, if nested;
-   spill point and receiving edge/node;
-   spill elevation;
-   active outlet or explicitly endorheic status;
-   whether it is a protected basin/lake surface for terrain
    construction.

The prototype must distinguish a depression from a currently represented
lake. A flat surface at spill level is a terrain-construction rule, not
proof of a dynamically simulated water level. Defer water balance,
evaporation and time-varying lake levels.

### 2.4 Rasterisation contract

Graph-to-raster conversion is a deterministic derived representation.
Define:

-   how graph geometry maps to L cells;
-   how overlapping or adjacent reaches are resolved;
-   how junctions are represented;
-   how graph features crossing tile or cube-face boundaries are clipped
    and reconstructed;
-   how raster cells map back to graph IDs;
-   what constitutes acceptable graph/raster disagreement.

Raster routing of the final surface remains a validation gate, not the
source of graph topology.

### 2.5 Terrain-change budget contract

The stated `c_cut` limit applies cumulatively to all lowering by KS, KV
and KC relative to `S0`.

For each cell `x`, define:

`budget(x) = c_cut(x)`

and account for lowering consumed by each stage:

-   `KS_used(x)`;
-   `KV_used(x)`;
-   `KC_used(x)`.

Require:

`KS_used(x) + KV_used(x) + KC_used(x) <= budget(x)`

No stage may assume the full budget remains available after an earlier
stage has consumed part of it.

The current KS expression, `C1(z_s) + min(r, α·H)` (or its `H_f` form),
does not by itself demonstrate that the lowering relative to `S0` is
bounded by `c_cut`. The implementation must therefore either enforce the
remaining budget explicitly or report the required correction as an
incompatibility/residual. It must not silently exceed the cap.

Record both the total and per-stage terrain delta, including cells where
the desired graph conditioning cannot be achieved within the budget.

### 2.6 Determinism, partition and edit contracts

G0 must define tests for:

-   same inputs and same configuration produce identical canonical graph
    and surface outputs;
-   one-region and multi-region builds produce identical shared outputs;
-   tile and cube-face boundaries do not create missing or duplicate
    graph features;
-   fresh and incremental builds agree for a representative edit;
-   graph identity remains stable when only derived attributes change;
-   topology changes are explicitly reported and invalidate the
    dependent products.

The exact bitwise or tolerance requirements must follow the project's
existing constitution and numeric contracts. Do not introduce a weaker
tolerance merely to make a new stage pass.

### 2.7 Minimum fixture

Create a small deterministic fixture containing:

1.  one trunk and two tributaries;
2.  one explicit confluence;
3.  one antecedent river crossing a ridge;
4.  one closed basin with a protected lake/basin node;
5.  one divide close to a region boundary;
6.  one reach crossing a tile boundary;
7.  one reach crossing a cube-face boundary.

The fixture should support isolated tests of graph topology, basin
hierarchy, rasterisation, terrain budget, partition invariance and
incremental invalidation. Add harder cases only after this base fixture
is useful.

### G0 exit criteria

G0 passes when:

-   the contracts above are written and agreed;
-   canonical graph and basin data can be serialised and compared;
-   validators detect deliberately injected invalid edges, cycles,
    duplicate IDs, unresolved endpoints and budget overruns;
-   the fixture runs under at least two partition layouts;
-   test thresholds and measurement definitions are fixed before RG
    results are inspected.

## 3. Revised milestone sequence

The original G1a--G1f work is retained but separated into
risk-retirement gates. Existing work may be reused; the purpose is to
make the evidence independently inspectable.

  -------------------------------------------------------------------------------
  Milestone               Scope                   Required evidence / exit
                                                  decision
  ----------------------- ----------------------- -------------------------------
  Pre                     Coastal snap fix and    Baselines and known issues
                          reread; density and     recorded; no RG implementation
                          `R_b` baseline;         assumed
                          existing review         
                          evidence reread         

  G0                      Contracts and harness   Graph, basin, rasterisation,
                                                  budget, partition and edit
                                                  contracts pass their validators

  G1a                     Sills of `S0`; H1       Graph topology diff; D-2
                          rerouting on five       exceptions; K5-channel
                          windows, R1h first      agreement; sill cost;
                                                  sensitivity report

  G1b                     KS0/KS1 in isolation    Per-cell and cumulative budget
                                                  accounting; drainage and
                                                  terrain-preservation deltas;
                                                  unresolved incompatibilities

  G1c                     KG-trunk and fine       Continuous trunk graph and
                          rasterisation, no       profiles; stable IDs; junction
                          tributaries             and boundary correctness;
                                                  partition equivalence

  G1d                     KG-trib on KS0, current No invalid/dangling/duplicate
                          extractor behind output reaches; area and discharge
                          contract                conservation; hierarchy and
                                                  density statistics;
                                                  halo-boundary tests

  G1e                     Basin hierarchy and     Nested/shared-spill/endorheic
                          protected basin         cases; stable basin IDs and
                          surfaces                spill relationships across
                                                  partitions

  G1f                     KV and KC, separately   KV-only, KC-only and combined
                          switchable              results; cumulative budget
                                                  enforced; fixture passes

  G2                      Integrated RG-2         All five windows, quantitative
                          comparison against B    gates, cost/memory, partition
                                                  and incremental tests, owner
                                                  flight

  G3                      Architecture decision   Adopt RG, run a targeted second
                                                  prototype, partially adopt
                                                  successful components, or
                                                  retain B with refinements
  -------------------------------------------------------------------------------

### G1a --- H1 graph and sill validity

Retain the existing sill experiment. In addition to the current D-2 and
K5-channel agreement measures, report:

-   topology diff: changed reaches, junctions, outlets and affected
    catchments;
-   sensitivity to small perturbations in `S0` and sill heights;
-   spatial distribution and terrain-type concentration of D-2
    exceptions;
-   whether changed topology affects major trunk connectivity or only
    local reaches.

**Stop/decision:** if major trunk topology is unstable or materially
inconsistent with K5 channels, investigate K5' before proceeding to the
full graph chain.

### G1b --- KS conditioning in isolation

Evaluate `RG-0` (`S0`), `RG-1` (KS0) and `RG-2` (KS1) independently.

Report:

-   lowering distribution against `c_cut`, not only a pass/fail count;
-   cumulative budget consumption by stage;
-   positive-band clipping and remaining positive-band dams;
-   newly created pits or basins;
-   drainage connectivity along and between graph lines;
-   effects on nearby divides and adjacent valleys;
-   residuals where the graph cannot be represented within the permitted
    budget.

**Stop/decision:** if KS requires broad or deep changes, prototype
constrained reconstruction separately rather than hiding the problem in
KV or KC.

### G1c --- Trunk graph to fine terrain

Implement trunk centrelines, stable reach/junction identities, bed
profiles, spill levels, exceptional reaches, deterministic rasterisation
and storage/reconstruction across domains.

Test:

-   profile continuity through junctions and across reach boundaries;
-   no unintended uphill steps outside explicitly represented
    exceptions;
-   no missing or duplicated features at tile or cube-face boundaries;
-   stable IDs after neighbouring tile rebuilds;
-   one-region versus multi-region equivalence.

**Exit:** a continuous fine-scale trunk representation exists without an
L erosion round.

### G1d --- Tributary extraction

Treat canonical flood plus clipped MFD on KS0 as the first extractor
implementation, not as the definition of tributary generation.

Test:

-   attachment to intended trunk, lake or outlet nodes;
-   no dangling reaches, duplicate reaches or cycles;
-   area/discharge conservation at confluences;
-   Strahler consistency;
-   partition-invariant topology across tile boundaries;
-   behaviour where catchments exceed the 13 km halo and depend on H1
    diffuse-area seeding;
-   catchments just below and above the effective halo range, including
    sources outside a tile with confluences inside it;
-   density, source-to-outlet length, reach length between junctions,
    order by contributing area, and junction-angle distributions.

**Stop/decision:** if topology or hierarchy is poor, try RG-3 behind the
same output contract. Do not tune valley construction to compensate for
a poor network.

### G1e --- Basin and lake topology

Test nested basins, adjacent depressions sharing a spill, a closed
endorheic basin, a single-outlet lake, multiple inlets, a basin spilling
into another basin, and a basin crossing a tile boundary.

**Exit:** basin identities and spill relationships are stable under
partition changes. Dynamic water balance remains out of scope.

### G1f --- KV and KC

Keep KV and KC independently switchable:

1.  KV only: construct valleys around graph beds; no pit repair.
2.  KC only: repair non-basin pits within the remaining budget; no
    valley stamping.
3.  KV + KC: integrated result.

Inspect valley-floor transitions, tributary confluences, divides, lakes
and ridge crossings. Report budget consumed by each stage and the
residual failures.

**Exit:** hard correctness, partition and budget gates pass before the
five-window integrated comparison.

## 4. Gate taxonomy and additional metrics

Keep the existing §4.1 criteria, but classify every result.

  -----------------------------------------------------------------------
  Gate class              Examples                Interpretation
  ----------------------- ----------------------- -----------------------
  Correctness             valid graph, no cycles, Internally valid result
                          valid endpoints, basin  
                          semantics, budget       

  Partition invariance    one vs multiple         Execution layout does
                          regions, tile/cube-face not change output
                          crossings               

  Incremental correctness edit vs fresh build;    Rebuild semantics are
                          correct dirty set       valid

  Preservation            incision, saddle cuts,  Unrelated terrain is
                          crest rounding, relief  not excessively damaged
                          spectrum                

  Geomorphology           tributary hierarchy,    River system is
                          confluences, profiles,  plausible
                          valley widths           

  Product                 cost, memory, runtime   Usable application
                          read path               architecture

  Owner judgement         visual flight           Overall result is
                                                  acceptable
  -----------------------------------------------------------------------

Add graph-specific reporting:

-   trunk continuity and outlet connectivity;
-   junction area/discharge conservation;
-   bed-profile residual and exceptional-reach count;
-   graph-to-raster agreement;
-   tributary attachment and hierarchy;
-   basin parent/child and spill consistency;
-   boundary graph diff;
-   stable-ID diff after rebuild and edit.

Define operationally before results are inspected:

-   what counts as a basin for G-DRAIN and its denominator;
-   how cut-through saddle area is measured;
-   how valley width is measured for asymmetric sections;
-   how GLO-30 references are matched to synthetic windows;
-   acceptable graph/raster agreement tolerance;
-   whether each metric is per-cell, per-reach, per-catchment or
    area-weighted.

Keep raster G-DRAIN separate from graph validity: a draining raster does
not prove the graph is correct, and a valid graph does not prove the
raster drains.

## 5. Decision table and partial-adoption policy

  -----------------------------------------------------------------------
  Failure signal          Interpretation          Next action
  ----------------------- ----------------------- -----------------------
  H1 rerouting materially K5/H1 coupling is       Investigate K5'
  changes major trunk     fundamental             
  topology                                        

  Sills are too expensive Current edge-sill       Test sparse evaluation,
  or unstable             representation may not  caching or an
                          scale                   alternative
                                                  representation

  KS exceeds the terrain  Graph and terrain are   Test constrained
  budget                  incompatible under      reconstruction; do not
                          current constraints     raise the cap silently

  Trunk graph is sound,   Trunk thesis may still  Try RG-3 behind the
  tributary extraction is be valid                same contract
  poor                                            

  Graph is sound, valleys KV is the weak          Revise valley
  look stamped            component               construction
                                                  independently

  Basin topology is       Basin semantics are     Keep basin work
  unstable                insufficient            separate; do not
                                                  classify all pits as
                                                  lakes

  Partition invariance    Ownership or boundary   Stop integration and
  fails                   contracts are wrong     fix contracts

  RG-2 fails preservation Full chain is not yet   Retain successful
  or visual gates         justified               components as
                                                  experiments; compare
                                                  against B
  -----------------------------------------------------------------------

A failure of RG-2 does not automatically reject the graph architecture.
The decision record should say which components passed, which failed,
and whether a targeted follow-up experiment is justified.

## 6. Work organisation and parallelism

Use parallel lanes only after the relevant contracts are agreed.

  -----------------------------------------------------------------------
  Lane                    Responsibility          Start condition
  ----------------------- ----------------------- -----------------------
  Lead                    Contracts, interfaces,  Immediately
                          integration, gate       
                          ownership               

  Explorer                Sills/H1, KS            G0 contracts agreed
                          experiments and         
                          quantitative reports    

  Graph lane              Graph types,            G0 graph contract
                          identities, trunk       agreed
                          graph, rasterisation    

  Hydrology lane          Basin hierarchy and     G0 graph and basin
                          tributary extraction    contracts agreed

  Terrain lane            Synthetic               Bed and budget
                          cross-sections and      contracts agreed
                          KV/KC prototypes        
  -----------------------------------------------------------------------

The terrain lane may prototype cross-sections using synthetic graph
inputs before KG is ready. It must not independently redefine graph or
bed semantics.

The lead owns shared contracts and integration. Agents may propose
changes, but must not weaken existing constitution requirements or gates
to make a stage pass. Any conflict should be surfaced as a proposed
change with evidence and owner approval.

## 7. Explicitly deferred

Keep the existing scope restraint. Defer:

-   dynamic lake water balance, evaporation and time-varying levels;
-   a full Horton-style generator unless the current tributary extractor
    fails;
-   detailed meandering and channel migration;
-   advanced floodplain deposition and sediment transport;
-   runtime optimisation beyond proving the representation can be read
    correctly;
-   general incremental graph editing beyond one representative edit and
    a clear invalidation contract.

Do design graph identity and invalidation semantics now. A one-time bake
is not sufficient evidence for an editable-world architecture.

## 8. Amendments to draft 3

1.  Insert G0 before the current pre-phase.
2.  Split the current G1 chain into independently gated trunk,
    tributary, basin and valley stages.
3.  Resolve the KS/cumulative-lowering-budget inconsistency before KS
    implementation.
4.  Define graph identity independently of tile ownership and
    raster-cell identity.
5.  Add graph-specific correctness and boundary metrics.
6.  Make G2 an explicit architecture decision gate with partial adoption
    as a valid outcome.
7.  Preserve B as benchmark and fallback; do not select a production
    architecture from a single successful drainage metric or visual
    flight.

**Recommended immediate action:** approve G0 and continue the existing
pre-work and G1a experiments. Do not start KS, KG, KV or KC
implementation until the graph and budget contracts are agreed.
