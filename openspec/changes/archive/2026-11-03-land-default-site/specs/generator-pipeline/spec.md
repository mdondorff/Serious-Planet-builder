## ADDED Requirements

### Requirement: GEN-108 The descent site of a world
A world definition MAY name the site the viewer's scripted descent opens over, `[world] site = [latitude, longitude]` in degrees (integers or floats, finite, latitude within 90 and longitude within 180), written back canonically and refused otherwise naming `world.site`. The site SHALL be presentation: it SHALL enter no hash (the definition's hash and base hash, and so every stored key, are those of the same definition without it). The viewer SHALL open the descent over the first of: the `--site` option, the definition's site, 20 N 30 E where the field there is at or above the sea level and carries no sea, the land 1 km inland of the first shore found by the shore walk of the lit scenes from 20 N 30 E (`find_coast`, a pure function of the world), 20 N 30 E.

Verify: A
Status: active
Source: Owner test 2026-11-03 (the default site is sea on m9r2), inbox #52, #53

#### Scenario: The definition's site
- **WHEN** a definition with `world.site` is parsed, written and parsed again, and hashed with and without the site
- **THEN** the site round-trips exactly, the text is canonical, and the hash and base hash are equal

#### Scenario: A bad site
- **WHEN** `world.site` has one or three numbers, a string, a latitude beyond 90 or a longitude beyond 180 degrees, or a NaN
- **THEN** loading is refused, naming `world.site`

#### Scenario: The order of the sources
- **WHEN** the descent site is asked with `--site`, with a definition site, and with neither on worlds where 20 N 30 E is sea
- **THEN** the option wins, then the definition's, and otherwise the result is land (the field above the sea level there) and the same on a second call
