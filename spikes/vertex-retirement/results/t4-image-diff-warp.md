# T4: image differences of the candidates against the current vertex path (adapter: Dx12 Microsoft Basic Render Driver (Cpu), key dx12-microsoft-basic-render-driver)
Reference field, 960x540, tau 6, cells 32. Candidates: A = 5x5 base surface, levels 0-2 compact (same plan as the vertex path); A0 = 3x3 base surface, levels 0-2 compact (why 5x5 is needed); B = render floor 3 (a different plan: level-3 tiles instead of levels 0-2, compared with the vertex path's own plan). diff = pixels with any channel different; max = largest channel difference (0-255); over8 = pixels differing by more than 8; mean = mean difference of the differing pixels.

| view | debug view | candidate | frame nodes | diff px | of | max | over8 | mean |
|---|---|---|---|---|---|---|---|---|
| orbit-20000km | face | A (5x5) | 16 | 1 | 518400 | 255 | 1 | 255.00 |
| orbit-20000km | face | A0 (3x3) | 16 | 95 | 518400 | 255 | 95 | 255.00 |
| orbit-20000km | face | B (floor 3) | 181 | 17 | 518400 | 255 | 17 | 255.00 |
| orbit-20000km | tile-id | A (5x5) | 16 | 1 | 518400 | 255 | 1 | 255.00 |
| orbit-20000km | tile-id | A0 (3x3) | 16 | 95 | 518400 | 255 | 95 | 255.00 |
| orbit-20000km | tile-id | B (floor 3) | 181 | 17 | 518400 | 255 | 17 | 255.00 |
| orbit-20000km | level | A (5x5) | 16 | 1 | 518400 | 255 | 1 | 255.00 |
| orbit-20000km | level | A0 (3x3) | 16 | 95 | 518400 | 255 | 95 | 255.00 |
| orbit-20000km | level | B (floor 3) | 181 | 17 | 518400 | 255 | 17 | 255.00 |
| orbit-20000km | morph | A (5x5) | 16 | 224 | 518400 | 255 | 1 | 2.13 |
| orbit-20000km | morph | A0 (3x3) | 16 | 16507 | 518400 | 255 | 103 | 2.99 |
| orbit-20000km | morph | B (floor 3) | 181 | 36304 | 518400 | 255 | 34272 | 104.81 |
| orbit-20000km | height | A (5x5) | 16 | 1246 | 518400 | 137 | 15 | 1.39 |
| orbit-20000km | height | A0 (3x3) | 16 | 28088 | 518400 | 203 | 5144 | 5.87 |
| orbit-20000km | height | B (floor 3) | 181 | 36530 | 518400 | 177 | 10504 | 10.42 |
| orbit-20000km | normals | A (5x5) | 16 | 215 | 518400 | 78 | 1 | 1.36 |
| orbit-20000km | normals | A0 (3x3) | 16 | 19918 | 518400 | 252 | 95 | 1.77 |
| orbit-20000km | normals | B (floor 3) | 181 | 9240 | 518400 | 252 | 17 | 1.34 |
| orbit-20000km | depth | A (5x5) | 16 | 32 | 518400 | 238 | 3 | 11.47 |
| orbit-20000km | depth | A0 (3x3) | 16 | 319 | 518400 | 239 | 136 | 60.95 |
| orbit-20000km | depth | B (floor 3) | 181 | 135 | 518400 | 240 | 58 | 37.11 |
| orbit-20000km | relief | A (5x5) | 16 | 1708 | 518400 | 218 | 11 | 1.25 |
| orbit-20000km | relief | A0 (3x3) | 16 | 30756 | 518400 | 229 | 3119 | 4.01 |
| orbit-20000km | relief | B (floor 3) | 181 | 35513 | 518400 | 224 | 7621 | 6.60 |
| orbit-20000km | morph-height | A (5x5) | 16 | 98 | 518400 | 128 | 1 | 2.30 |
| orbit-20000km | morph-height | A0 (3x3) | 16 | 7866 | 518400 | 135 | 104 | 2.80 |
| orbit-20000km | morph-height | B (floor 3) | 181 | 23710 | 518400 | 128 | 81 | 1.74 |
| high-2000km | face | A (5x5) | 92 | 15 | 518400 | 255 | 15 | 255.00 |
| high-2000km | face | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | face | B (floor 3) | 92 | 4 | 518400 | 255 | 4 | 255.00 |
| high-2000km | tile-id | A (5x5) | 92 | 15 | 518400 | 255 | 15 | 255.00 |
| high-2000km | tile-id | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | tile-id | B (floor 3) | 92 | 4 | 518400 | 255 | 4 | 255.00 |
| high-2000km | level | A (5x5) | 92 | 15 | 518400 | 255 | 15 | 255.00 |
| high-2000km | level | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | level | B (floor 3) | 92 | 4 | 518400 | 255 | 4 | 255.00 |
| high-2000km | morph | A (5x5) | 92 | 3036 | 518400 | 255 | 24 | 2.41 |
| high-2000km | morph | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | morph | B (floor 3) | 92 | 1372 | 518400 | 255 | 1067 | 27.37 |
| high-2000km | height | A (5x5) | 92 | 16535 | 518400 | 199 | 332 | 1.82 |
| high-2000km | height | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | height | B (floor 3) | 92 | 324 | 518400 | 168 | 42 | 6.41 |
| high-2000km | normals | A (5x5) | 92 | 1556 | 518400 | 220 | 15 | 2.49 |
| high-2000km | normals | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | normals | B (floor 3) | 92 | 22 | 518400 | 199 | 4 | 32.86 |
| high-2000km | depth | A (5x5) | 92 | 1871 | 518400 | 235 | 50 | 3.98 |
| high-2000km | depth | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | depth | B (floor 3) | 92 | 86 | 518400 | 230 | 19 | 16.34 |
| high-2000km | relief | A (5x5) | 92 | 31789 | 518400 | 209 | 268 | 1.34 |
| high-2000km | relief | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | relief | B (floor 3) | 92 | 362 | 518400 | 191 | 33 | 5.55 |
| high-2000km | morph-height | A (5x5) | 92 | 433 | 518400 | 128 | 15 | 5.40 |
| high-2000km | morph-height | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | morph-height | B (floor 3) | 92 | 675 | 518400 | 128 | 4 | 1.75 |
| aerial-200km | face | A (5x5) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | face | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | face | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | tile-id | A (5x5) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | tile-id | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | tile-id | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | level | A (5x5) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | level | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | level | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph | A (5x5) | 136 | 404 | 518400 | 255 | 1 | 1.63 |
| aerial-200km | morph | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | height | A (5x5) | 136 | 1694 | 518400 | 108 | 20 | 1.40 |
| aerial-200km | height | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | height | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | normals | A (5x5) | 136 | 43 | 518400 | 1 | 0 | 1.00 |
| aerial-200km | normals | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | normals | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | depth | A (5x5) | 136 | 663 | 518400 | 44 | 14 | 1.38 |
| aerial-200km | depth | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | depth | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | relief | A (5x5) | 136 | 4330 | 518400 | 56 | 12 | 1.11 |
| aerial-200km | relief | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | relief | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph-height | A (5x5) | 136 | 494 | 518400 | 1 | 0 | 1.00 |
| aerial-200km | morph-height | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph-height | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
