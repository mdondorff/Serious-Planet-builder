"""Lead #184t: agreement both ways, as a proxy. The final surface's own D8 channels over 75 km2 (m3_d8.py's routing on the box) against two H1 channel zones: the run's own
(h1-rivers.u8 of the chain) and a reference chain's (today's H1, e.g. r1h-kw). A channel cell agrees when its H1 cell, or one of that cell's 8 neighbours, is an H1
channel cell of the reference. The gate's own agreement at 75 km2 is a different, reach-based measure; read the proxy against the gate on the same row.
Usage: agree_proxy.py <chain dir> <reference chain dir> [more reference chain dirs]"""
import os, sys, numpy as np
from scipy import ndimage
D, refs = sys.argv[1], sys.argv[2:]
n, m, dx, nh = 8160, 1600, 101.802, 1020
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'm3_d8.py'), encoding='utf-8').read()
a = src.index('@numba.njit(cache=True)\ndef flood_d8'); b = src.index('rec, area = flood_d8(z, sea, dx)')
import numba, heapq  # noqa: E401
exec(compile(src[a:b].replace('@numba.njit(cache=True)', '@numba.njit'), 'm3_d8.flood_d8', 'exec'))
z = np.fromfile(f'{D}/final.f32', '<f4').reshape(n, n).astype(np.float64)[m:n - m, m:n - m]
sea = z <= 0.0
_, area = flood_d8(z, sea, dx)
ch = (area.reshape(z.shape) >= 75e6) & ~sea
ys, xs = np.nonzero(ch)
hy, hx = (ys + m) // 8, (xs + m) // 8
print(f'{D.rstrip("/").split("/")[-1]}: final D8 channel cells over 75 km2 in the box {len(ys)}')
for R in [D] + refs:
    riv = np.fromfile(f'{R}/h1-rivers.u8', 'u1').reshape(nh, nh) > 0
    zone = ndimage.binary_dilation(riv, structure=np.ones((3, 3), bool))
    print(f'  in the H1 channel zone of {R.rstrip("/").split("/")[-1]}: {100 * zone[hy, hx].mean():.1f} %')
