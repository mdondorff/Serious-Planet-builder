# T4: image differences of the candidates against the current vertex path (adapter: Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU (DiscreteGpu), key vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu)
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
| orbit-20000km | morph | A (5x5) | 16 | 452 | 518400 | 255 | 1 | 1.56 |
| orbit-20000km | morph | A0 (3x3) | 16 | 18186 | 518400 | 255 | 106 | 2.83 |
| orbit-20000km | morph | B (floor 3) | 181 | 36305 | 518400 | 255 | 34278 | 104.84 |
| orbit-20000km | height | A (5x5) | 16 | 1300 | 518400 | 137 | 15 | 1.38 |
| orbit-20000km | height | A0 (3x3) | 16 | 28118 | 518400 | 203 | 5149 | 5.87 |
| orbit-20000km | height | B (floor 3) | 181 | 36539 | 518400 | 177 | 10515 | 10.42 |
| orbit-20000km | normals | A (5x5) | 16 | 231 | 518400 | 78 | 1 | 1.33 |
| orbit-20000km | normals | A0 (3x3) | 16 | 20029 | 518400 | 252 | 95 | 1.77 |
| orbit-20000km | normals | B (floor 3) | 181 | 9267 | 518400 | 252 | 17 | 1.34 |
| orbit-20000km | depth | A (5x5) | 16 | 42 | 518400 | 238 | 3 | 8.95 |
| orbit-20000km | depth | A0 (3x3) | 16 | 338 | 518400 | 239 | 138 | 57.59 |
| orbit-20000km | depth | B (floor 3) | 181 | 152 | 518400 | 240 | 58 | 33.07 |
| orbit-20000km | relief | A (5x5) | 16 | 1604 | 518400 | 218 | 11 | 1.26 |
| orbit-20000km | relief | A0 (3x3) | 16 | 30772 | 518400 | 229 | 3112 | 4.01 |
| orbit-20000km | relief | B (floor 3) | 181 | 35489 | 518400 | 224 | 7610 | 6.61 |
| orbit-20000km | morph-height | A (5x5) | 16 | 76 | 518400 | 128 | 1 | 2.67 |
| orbit-20000km | morph-height | A0 (3x3) | 16 | 7862 | 518400 | 135 | 104 | 2.80 |
| orbit-20000km | morph-height | B (floor 3) | 181 | 23715 | 518400 | 128 | 81 | 1.74 |
| high-2000km | face | A (5x5) | 92 | 15 | 518400 | 255 | 15 | 255.00 |
| high-2000km | face | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | face | B (floor 3) | 92 | 4 | 518400 | 255 | 4 | 255.00 |
| high-2000km | tile-id | A (5x5) | 92 | 15 | 518400 | 255 | 15 | 255.00 |
| high-2000km | tile-id | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | tile-id | B (floor 3) | 92 | 4 | 518400 | 255 | 4 | 255.00 |
| high-2000km | level | A (5x5) | 92 | 15 | 518400 | 255 | 15 | 255.00 |
| high-2000km | level | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | level | B (floor 3) | 92 | 4 | 518400 | 255 | 4 | 255.00 |
| high-2000km | morph | A (5x5) | 92 | 5184 | 518400 | 255 | 25 | 1.88 |
| high-2000km | morph | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | morph | B (floor 3) | 92 | 1372 | 518400 | 255 | 1068 | 27.40 |
| high-2000km | height | A (5x5) | 92 | 16481 | 518400 | 199 | 332 | 1.82 |
| high-2000km | height | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | height | B (floor 3) | 92 | 310 | 518400 | 168 | 42 | 6.60 |
| high-2000km | normals | A (5x5) | 92 | 1469 | 518400 | 220 | 15 | 2.58 |
| high-2000km | normals | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | normals | B (floor 3) | 92 | 19 | 518400 | 199 | 4 | 37.89 |
| high-2000km | depth | A (5x5) | 92 | 1879 | 518400 | 234 | 49 | 3.94 |
| high-2000km | depth | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | depth | B (floor 3) | 92 | 82 | 518400 | 227 | 20 | 16.84 |
| high-2000km | relief | A (5x5) | 92 | 31893 | 518400 | 210 | 269 | 1.33 |
| high-2000km | relief | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | relief | B (floor 3) | 92 | 349 | 518400 | 191 | 32 | 5.70 |
| high-2000km | morph-height | A (5x5) | 92 | 335 | 518400 | 128 | 15 | 6.69 |
| high-2000km | morph-height | A0 (3x3) | 92 | 0 | 518400 | 0 | 0 | 0.00 |
| high-2000km | morph-height | B (floor 3) | 92 | 679 | 518400 | 128 | 4 | 1.75 |
| aerial-200km | face | A (5x5) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | face | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | face | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | tile-id | A (5x5) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | tile-id | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | tile-id | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | level | A (5x5) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | level | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | level | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph | A (5x5) | 136 | 664 | 518400 | 255 | 1 | 1.38 |
| aerial-200km | morph | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | height | A (5x5) | 136 | 1666 | 518400 | 108 | 19 | 1.42 |
| aerial-200km | height | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | height | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | normals | A (5x5) | 136 | 32 | 518400 | 1 | 0 | 1.00 |
| aerial-200km | normals | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | normals | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | depth | A (5x5) | 136 | 669 | 518400 | 44 | 15 | 1.41 |
| aerial-200km | depth | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | depth | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | relief | A (5x5) | 136 | 4358 | 518400 | 56 | 11 | 1.11 |
| aerial-200km | relief | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | relief | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph-height | A (5x5) | 136 | 131 | 518400 | 1 | 0 | 1.00 |
| aerial-200km | morph-height | A0 (3x3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
| aerial-200km | morph-height | B (floor 3) | 136 | 0 | 518400 | 0 | 0 | 0.00 |
