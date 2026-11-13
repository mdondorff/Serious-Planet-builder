# k7-bed-coherence

Why: the owner flew R3a (r3a-e1) and saw water flowing uphill over a stepped wall (frame-33547, F2). The #124 diagnosis put it in the L data. The R3a line check (`k7_line_check`, K7 rerun on tiles 89/7, 83/7 and 83/8 with the bake's inputs) shows the stored lines are the lines K7 burnt, cell for cell, and the stored ground is K7's height within 0.125 m; K7's own height rises along about 0.3 % of the line steps, up to 42 m. The cause is the bounded carry of the profile (GEN-169): its target is a sliding minimum over the last 48 (burn) or 64 (final pass) line steps, so a low more than that far upstream drops out and the next cell's target jumps. The burn's cutoff binds 1,980 cells on tile 89/7 (up to 365 m).

What: the burn levels come from the whole channel network, once, before K7 (GEN-185): every vertex's level is the running minimum down the channel tree of its H1 level and of K7's start surface `B + regional` at the line cells up to it. The stored graph holds these levels, so the stored level stays K7's burn input (STRM-084, TEST-114 unchanged) and the burn needs no carry beyond the cells between two vertices: its cutoff binds no cell. The region bake reports how deep the levels lie under the start surface. GEN-176's "a cutoff over 0.25 m comes to the architect" becomes the assertion that the burn has none. Pass-2 kernel version 10 and graph version 3: stores of earlier versions are refused (owner, 2026-11-05).

Not here: F3, the bank beside a line (GEN-186, reserved; next change, the line placed on the valley floor at L); the read-time water level (REND-066); the viewer's base under `--commit` (VIEW-072, explorer).

Requirements: GEN-185 (ADDED), GEN-176 (MODIFIED), TEST-134 (ADDED). Source: lead 2026-11-13 (P2 E1, #125; owner "Items 1-4 yes").
