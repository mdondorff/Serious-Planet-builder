## ADDED Requirements
### Requirement: TEST-138 The corridor measure at the coast
A tier-A fixture SHALL check that the corridor measure (BUILD-057) snaps a non-mouth node and routes over land only, and ends a route at the first sea cell it meets.

Verify: A
Status: active
Source: Lead 2026-11-16 (explorer #163; review B F2), BUILD-057
Detail: on land falling to an open sea with a 0 m bay 450 m from a confluence (node radius and corridor 800 m, bar 5 m), the sea-blind measure snaps the confluence into the bay and records a false rise of the 10 m shore on the reach below; at sea level 0 the confluence keeps a land cell and both reaches pass with no vertex over; a raw reach crossing a 0 m strait onto land rising to 29.5 m is over by 29.5 m sea-blind and passes at sea level 0, its route ending at the strait.
