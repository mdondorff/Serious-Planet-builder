## ADDED Requirements

### Requirement: TEST-053 Look override and views in the poses file
The poses file (TEST-050) SHALL take the keys of the table; a bad value SHALL be refused naming the key and the value. The override is for a sample look and is never stored or keyed.

| Key | Meaning | Default |
|---|---|---|
| `[[pose]]` `view` | a view of the viewer's J cycle (`lit`, `soil`, `moisture`, `insolation`, ...), drawn instead of the lit one | `lit` |
| `[look]` `ground_detail` | ground detail strength, 0 to 1 | the environment's (0) |
| `[[look.curve]]` `material`, `input`, `lo`, `hi` | one curve of the ground materials (LOOK-008), replacing the pair; materials `rock`, `sand`, `grass`, `soil`; inputs `slope_cos`, `height`, `temperature`, `precipitation`, `surface_type`, `soil_depth`, `moisture`, `insolation` | the look's |

Verify: A
Status: active
Source: opus-planner (2026-10-26), look review 2, TEST-050

#### Scenario: A look table and a bad name
- **WHEN** the file gives a `[look]` table and a pose `view`, or a `view`, `material` or `input` names nothing, or `ground_detail` lies outside 0 to 1
- **THEN** the plan carries the detail, the curves and the view, and a bad name or value is refused naming the key and the value
