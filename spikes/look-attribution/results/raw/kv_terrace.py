"""Lead #184m, decision 9's check: does `--kv-wall-s0` (s_wall = min(s_wall, S0 slope)) shave flat terraces far from incised beds? Per chain: KV's own step (KV - BAND6+C)
on land in the box, on flat ground (|grad S0| < 0.05), split by the distance to the nearest step-1 curve raster cell (the defaults' chain is read on the same cells).
Reported: cells KV lowers by more than 1 / 5 m, and their area. Usage: kv_terrace.py <chain dir> <trunk-curves.bin>"""
import sys, struct, numpy as np
from scipy import ndimage
D, CB = sys.argv[1], sys.argv[2]
n, m, cell = 8160, 1600, 101.802
ld = lambda s: np.fromfile(f'{D}/{s}.f32', '<f4').reshape(n, n).astype(np.float64)
s0 = ld('s0'); gy, gx = np.gradient(s0, cell); flat = np.hypot(gx, gy) < 0.05; del gx, gy
step = ld('kv') - ld('band6c')
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; rast = np.zeros((n, n), bool)
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8 + 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4
    cc = np.frombuffer(b, np.dtype([('c', '<u4'), ('o', 'u1', 2)]), nr, k)['c'].astype(np.int64); k += 6 * nr
    rast[cc // n, cc % n] = True
dist = ndimage.distance_transform_edt(~rast)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True
base = box & (s0 > 0) & flat
print(f'{D.rstrip("/").split("/")[-1]}: flat land in the box {base.sum()} cells')
for lo, hi in ((0, 3), (3, 8), (8, 20), (20, 1e9)):
    s = base & (dist > lo) & (dist <= hi)
    a1 = (step[s] < -1).sum(); a5 = (step[s] < -5).sum()
    print(f'  {lo:>2}-{hi if hi < 1e8 else "inf":>3} cells from a curve: {s.sum():8d} flat cells; KV lowers > 1 m on {a1:7d} ({100 * a1 / max(s.sum(), 1):.2f} %, {a1 * cell * cell / 1e6:.1f} km2), > 5 m on {a5:6d}')
