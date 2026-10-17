# T6: wide texel (32-bit height, 15-bit normals) against the 16-bit texel, CPU mirror against tile_mesh_field, cells 32
Per level: 6 random tiles. relief = largest relief of the sampled tiles; step = largest height step (hr_step); h err = largest |height of the mirror - height of the mesh| (metres, raw: includes the f32 rounding of both); n err = largest angle between the decoded normal and the mesh normal (degrees, CON-14 bound 0.01); CON-13 = step <= 1 mm.

| field | level | texel | relief m | step mm | h err mm | n err deg | CON-13 | CON-14 |
|---|---|---|---|---|---|---|---|---|
| steep | 9 | 16-bit | 1875.1 | 28.6118 | 14.404 | 0.00339 | FAIL (step) | ok |
| steep | 9 | wide | 1875.1 | 0.0004 | 0.244 | 0.00681 | ok | ok |
| steep | 12 | 16-bit | 804.8 | 12.2806 | 6.348 | 0.00358 | FAIL (step) | ok |
| steep | 12 | wide | 804.8 | 0.0002 | 0.488 | 0.00712 | ok | ok |
| steep | 16 | 16-bit | 141.9 | 2.1655 | 1.221 | 0.00347 | FAIL (step) | ok |
| steep | 16 | wide | 141.9 | 0.0000 | 0.244 | 0.00679 | ok | ok |
| steep | 22 | 16-bit | 6.7 | 0.1018 | 0.244 | 0.00348 | ok | ok |
| steep | 22 | wide | 6.7 | 0.0000 | 0.244 | 0.00698 | ok | ok |
| reference | 9 | 16-bit | 71.9 | 1.0973 | 0.610 | 0.00335 | FAIL (step) | ok |
| reference | 9 | wide | 71.9 | 0.0000 | 0.122 | 0.00646 | ok | ok |
| reference | 12 | 16-bit | 14.8 | 0.2252 | 0.183 | 0.00356 | ok | ok |
| reference | 12 | wide | 14.8 | 0.0000 | 0.122 | 0.00718 | ok | ok |
| reference | 16 | 16-bit | 0.6 | 0.0098 | 0.122 | 0.00358 | ok | ok |
| reference | 16 | wide | 0.6 | 0.0000 | 0.122 | 0.00455 | ok | ok |
| reference | 22 | 16-bit | 0.0 | 0.0002 | 0.061 | 0.00231 | ok | ok |
| reference | 22 | wide | 0.0 | 0.0000 | 0.061 | 0.00479 | ok | ok |
