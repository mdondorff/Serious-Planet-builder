## ADDED Requirements
### Requirement: GEN-162 A breach does not cut below its chain exit
Under the local clip of the round rules (the round solver's rules) a breach SHALL NOT lower a cell below the base level of its chain exit: for a receiver chain that ends in an outlet above the sea level, the base of every cell of the chain is the outlet's height, and the target of a breach is at least that base. A chain that ends at or below the sea level SHALL have no base, and every other rule set (the default rules, K7's and the sediment pass's canonical flood without the clip) SHALL keep today's breach.
- A depression whose exit is fixed (A3 sections 5.3 and 6) SHALL NOT get deeper from round to round through the breach.
- A build whose outlets are all at or below the sea level SHALL be bit-identical to the build without the rule.

Verify: A
Status: active
Source: Architect, 2026-11-10

#### Scenario: Lake behind a cut outlet
- **WHEN** a closed bowl whose rim is its only outlet runs four rounds
- **THEN** its median depth under the fill grows by at most 10 % of the starting depth in round 1 (the incision of the sweeps; a bowl without the breach grows by 4.6 %) and by at most 0.25 m in any later round

#### Scenario: Outlet at the sea
- **WHEN** a bowl whose rim is at the sea level runs one round
- **THEN** no cell is more than 1 m under the fill
