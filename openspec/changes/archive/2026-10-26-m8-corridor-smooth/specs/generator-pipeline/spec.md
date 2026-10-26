## ADDED Requirements

### Requirement: GEN-084 The routed corridor is smoothed
The routed polyline of a coarse segment SHALL be smoothed: each of its points moves to the average of the chain formed by that polyline with the polyline of the segment it continues from and the one it continues into, over a tent of half width `smooth_m` (256 m) along the path, by at most `smooth_max_m` (64 m). A river that begins, ends or meets another at a node keeps that node: the move fades to zero over `smooth_m` towards it. The smoothed polyline SHALL be a function of the segment alone (GEN-067), and the smoothing's bound SHALL be part of the routing's reach (GEN-078). Both constants are part of the definition hash.

Verify: A
Status: active
Source: look review 2b (river-r1 benches), opus-planner (2026-10-26), GEN-066, GEN-067, GEN-078

#### Scenario: A kink
- **WHEN** a chain with a 45 degree kink is smoothed with its neighbours, and a chain with no neighbours
- **THEN** the worst turn per 100 m is at most half of the raw one, no point moves more than 64 m, a straight chain stays and the ends of a chain with no neighbour stay

#### Scenario: A joint and a confluence
- **WHEN** the two segments of a joint the river runs through are smoothed, and the segments of a confluence
- **THEN** the joint is one point from both sides within a micrometre and moved, and every point of a confluence segment is within 64 m of its routed point

#### Scenario: Borders, halos and threads
- **WHEN** a fork and a ridge are built over two halos, over neighbouring chunks and on four threads with the smoothing on
- **THEN** the corridor, the beds and the links of shared cells are bit for bit equal

## MODIFIED Requirements

### Requirement: GEN-078 A chunk routes only the segments that can reach it
The A-net network build of a chunk SHALL route a segment of the ring of coarse cells (and, for its bed, the segments upstream of it within the window) only if the segment's coarse chord lies within the region's radius plus the routing's reach (the band, the slide, the meander, the smoothing and the raster's rounding) of the region's centre. A segment left out SHALL change no cell, bed, inflow or link of the region: the network is bit for bit the one that routes every segment of the ring.

Verify: A
Status: active
Source: opus-planner (2026-10-26), GEN-066, GEN-067

#### Scenario: Pruned and unpruned
- **WHEN** chunks of a fork of rivers are built with every segment of the ring routed and with only the segments that can reach the region, at strides 1 and 4 and two halos
- **THEN** the corridor, links, anchors, areas and inflows are bit for bit equal and fewer segments are routed in total

### Requirement: GEN-081 The read set of a chunk
`chunk_read_set` SHALL return the discs outside which no item can change a chunk's bits: the chunk's region (its square plus the halo of GEN-049) and the routed network it reads, that is the segments the pruning of GEN-078 keeps (one `region_segments`, one `segments_reaching`) and those upstream of them along the main stem up to the law's window (one more segment when the corridor is smoothed, GEN-084), each widened by `routing_reach_m` along its chord. The set SHALL be a function of the chunk key, the global run and the stage parameters only, never of routed paths or items, so it cannot depend on the edits it selects.

Verify: A
Status: active
Source: m8-chunk-version, GEN-049, GEN-059, GEN-078

#### Scenario: The set covers what the network reads
- **WHEN** an item's reach touches only the routing reach of a kept segment, outside the halo
- **THEN** it is inside the read set and the chunk's bits change with it; an item outside every disc changes no bit, and the set takes no item as input
