## ADDED Requirements
### Requirement: GEN-141 The round kernels make a catchment's round a function of its inputs
The routing SHALL offer two rules, off by default and a key byte only when on: the multiple-flow-direction area clipped to the D8 catchment (a cell shares its area among its own catchment's lower neighbours only, so the area at a catchment's outlet is the sum of its members' true areas), and a flood that visits cells by (level, cell id) with no FIFO queue.
- A carve SHALL be able to visit a round's cells one catchment after the other (either catchment order); the result SHALL be the same bits as the region-order carve (G-ROUND), with today's rules and with the round rules.
- With both rules off the routing and the carve SHALL be today's, bit for bit; pass 2 SHALL refuse the rules.
- A catchment routed alone is NOT required to equal its part of the region routing: a cell on a flat may be first reached from a neighbour in another catchment. Routing locality is gated as the tile-partitioned route against the whole-region route (G-PART).

Verify: A
Status: planned
Source: A3 §6.2 to §6.4, inbox #98 step 1, ADR 0112 (proposed)

#### Scenario: the round kernels
- **WHEN** lands of ridges and of terraces are routed with the clip and carved for 20 steps one catchment after the other in both orders
- **THEN** the outlet of every catchment holds the area of its members, the carve is the region-order carve bit for bit, the default rules give today's routing, and a land without flats routed catchment by catchment alone equals the region routing
