## ADDED Requirements
### Requirement: TEST-121 The basin breach tests
The basin breach SHALL be tested at tier A: (a) a closed noise basin is drained by the cheapest path (the cells and the depth cut are stated), nothing is raised and a second pass changes nothing; (b) a basin deeper than the breach limit stays a lake, untouched, and drains within a limit that covers it; (c) the path stays inside the halo with the basin (the same basin drains at halo 9 and stays a lake at halo 8) and a basin on the border ring stays a lake; (d) a closed bowl with no lower cell is unchanged; (e) the whole-region carve, the round solver on 1, 3 and 8 threads and the memoised solve on two tile sizes are the same bits with the breach on, and the breach moves the result; (f) more passes leave no more closed basins than fewer, raise no cell and repeat to the same bits; the fixed point is the result of N passes after which pass N+1 drains nothing, and a cap below N is reported; (g) the layer key changes with the parameters; (h) a sill breached by three donors in one round is lowered by at most the cap in the whole-region carve and the round solver (and the test fails without the cap).

Verify: A
Status: planned
Source: GEN-174, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_breach` and the layer store run
- **THEN** each of (a) to (g) has an assertion that cites GEN-174
