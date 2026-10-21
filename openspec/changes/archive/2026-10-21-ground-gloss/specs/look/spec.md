## ADDED Requirements

### Requirement: LOOK-010 The gloss of the ground
The lit plan SHALL carry the gloss table of the ground (REND-035) as look data: a roughness for each ground material (rock, sand, grass, soil; a number above 0 and up to 1), two roughness modifiers (the wet and the icy ground), each an input of the weights' vector, a `(lo, hi)` smoothstep pair on it and a factor above 0 and up to 1 that multiplies the roughness at response 1 (a pair with `lo == hi` leaves the modifier out), and the base reflectance `f0` from 0 to 1. The roughness of a ground point SHALL be the weights' mean of the materials' roughness (the weights of LOOK-008) times the modifiers, and none where no material has weight. The default SHALL be rock 0.85, sand 0.65, grass 0.75, soil 0.80, the wet modifier on the reserved moisture input (out until a driver map supplies it), the icy modifier on the temperature falling from 0 to -5 C with the factor 0.4, and `f0` 0.04. The table SHALL be written in the plan's text only when it is not the default (so a plan made with the defaults keeps its text and hash), SHALL round-trip exactly, and SHALL be refused naming the key when a value is not finite, a roughness or factor is not above 0 and up to 1, a modifier's input is not one of the inputs, `f0` is outside 0 to 1, or a pair differs only below `f32` (the shader decides on the `f32` words). Presentation data (ADR 0035).

Verify: A
Status: active
Source: M7 G2b (opus-planner, 2026-10-21; owner: roughness and shine are in M7), LOOK-008, ADR 0035

#### Scenario: The table and the roughness
- **WHEN** a lit plan has the default gloss and one with another rock roughness and `f0`, each is written as text and parsed back, and values outside the limits are given
- **THEN** the first writes no `gloss` token, the second writes one and round-trips, each bad value is refused naming its key, and the roughness of dry flat ground is that of sand, of icy ground the sand's times 0.4, with the wet modifier out by default and in when a look gives it a pair, and none for no weights
