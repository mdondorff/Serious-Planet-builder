"""Lead #184m: where a chain's cut-through saddles against S0 sit, and which stage cuts them. The gate's measure (river_suite::ridge_report), re-run in Python on the box:
a ridge cell of S0 (land, at least 2 m above both cells 2 away along an axis or a diagonal), lowered more than 20 m against S0 by the stage surface, with a channel of 1 km2
or more of the stage's own routing in its 3 x 3 and none of S0's routing there; a saddle is an 8-connected group of such cells. Routing: priority-flood fill from the box
edge and the sea (epsilon 1e-4 m), D8 by the steepest drop (m3_d8.py's), not the gate's route rules, so the counts differ from the gate's; the attribution is the point.
Per saddle: the first stage (S_KH, KS0, KS1+C, BAND6+C, KV, final) on which it is a saddle, and its distance to the nearest step-1 curve raster cell.
Usage: saddles184.py <chain dir> [trunk-curves.bin]"""
import os, sys, struct, numpy as np
from scipy import ndimage
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numba, heapq
D = sys.argv[1]; CB = sys.argv[2] if len(sys.argv) > 2 else None
n, m, dx = 8160, 1600, 101.802
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'm3_d8.py'), encoding='utf-8').read()
a = src.index('@numba.njit(cache=True)\ndef flood_d8'); b = src.index('rec, area = flood_d8(z, sea, dx)')
exec(compile(src[a:b].replace('@numba.njit(cache=True)', '@numba.njit'), 'm3_d8.flood_d8', 'exec'))
ld = lambda s: np.fromfile(f'{D}/{s}.f32', '<f4').reshape(n, n).astype(np.float64)[m:n - m, m:n - m]
s0 = ld('s0'); sea = s0 <= 0.0; H, W = s0.shape


def chan(z):
    _, ar = flood_d8(z, sea, dx)
    c = (ar.reshape(H, W) >= 1e6) & ~sea
    return ndimage.binary_dilation(c, structure=np.ones((3, 3), bool))


ridge = np.zeros((H, W), bool)
for ox, oy in ((1, 0), (0, 1), (1, 1), (1, -1)):
    za = np.roll(s0, (2 * oy, 2 * ox), (0, 1)); zb = np.roll(s0, (-2 * oy, -2 * ox), (0, 1))
    ridge |= (s0 - za >= 2.0) & (s0 - zb >= 2.0)
ridge &= ~sea; ridge[:3] = ridge[-3:] = False; ridge[:, :3] = ridge[:, -3:] = False
c0 = chan(s0)
dist = None
if CB:
    bb = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', bb, 4)[0]
    rast = np.zeros((n, n), bool)
    for _ in range(cnt):
        _r, npt = struct.unpack_from('<II', bb, k); k += 8 + 8 * npt
        nr = struct.unpack_from('<I', bb, k)[0]; k += 4
        cc = np.frombuffer(bb, np.dtype([('c', '<u4'), ('o', 'u1', 2)]), nr, k)['c'].astype(np.int64); k += 6 * nr
        rast[cc // n, cc % n] = True
    dist = ndimage.distance_transform_edt(~rast[m:n - m, m:n - m])
print(f'{D.rstrip("/").split("/")[-1]}: S0 ridge cells in the box {ridge.sum()}')
first = None
for st in ('kh', 'ks0', 'ks1c', 'band6c', 'kv', 'final'):
    z = ld(st)
    cut = ridge & (s0 - z > 20.0) & chan(z) & ~c0
    lab, k = ndimage.label(cut, structure=np.ones((3, 3), bool))
    extra = ''
    if dist is not None and k:
        dmin = ndimage.minimum(dist, lab, index=np.arange(1, k + 1))
        extra = f'; nearest curve cell <= 2 cells: {int((dmin <= 2).sum())}, 2-8: {int(((dmin > 2) & (dmin <= 8)).sum())}, > 8: {int((dmin > 8).sum())}'
    new = ''
    if first is not None:
        # saddles of this stage that were not saddle cells on any earlier stage
        g = ndimage.maximum(first, lab, index=np.arange(1, k + 1)) if k else np.zeros(0)
        new = f'; new on this stage {int((g == 0).sum())}'
        first |= cut
    else:
        first = cut.copy()
    print(f'  {st:7s} saddles {k:5d} ({cut.sum()} cells){new}{extra}')
