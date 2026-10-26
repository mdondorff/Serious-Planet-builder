## ADDED Requirements

### Requirement: VIEW-058 The flight site
`planet editor --site LAT,LON` SHALL start the scripted flight (VIEW-005) over that point, in degrees; without it the flight descends over latitude 20, longitude 30. A latitude beyond 90 degrees, a longitude beyond 360 degrees or a value that is not a number SHALL be refused with a message naming the option.

Verify: A
Status: active
Source: m8-refresh-priority, VIEW-005

#### Scenario: A flight over another site
- **WHEN** `--site 45,-100` is given
- **THEN** the options hold the site (45, -100), the script of the flight starts over it and differs from the default site's, and `--site 95,0` is refused
