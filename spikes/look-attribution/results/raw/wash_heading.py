"""Lead #184k (O5): do step 1's trunk curves follow K5's washboard? Headings in the map convention (degrees counter-clockwise from east, north up; the washboard's
crests run at 124-135 deg, s0_split.py). Reported: the share of curve length (points inside the box) within 5 deg of 0, 45, 90 and 135 deg (uniform: 5.6 % each).
The washboard W: K7's read R = S0 - B high-passed (minus its Gaussian of 6 L cells; its ridges are about 18 L cells apart). A trough is W < 0, a deep trough
W below its p25 over the box's land. Reported per heading class: the share of curve length in troughs and in deep troughs, against every land cell in the box.
Usage: wash_heading.py <chain dir with s0.f32 and base.f32> <trunk-curves.bin>"""
import sys, struct, numpy as np
from scipy import ndimage
D, CB = sys.argv[1], sys.argv[2]
n, m = 8160, 1600
R = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n) - np.fromfile(f'{D}/base.f32', '<f4').reshape(n, n)
W = R - ndimage.gaussian_filter(R, 6.0)
del R
land = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n) > 0
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True
lw = W[box & land]
p25 = np.percentile(lw, 25)
b = open(CB, 'rb').read(); assert b[:4] == b'TRC1'
k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; H, Ls, Ws = [], [], []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    if npt < 6:
        continue
    d = p[4:] - p[:-4]; c = (p[4:] + p[:-4]) / 2
    H.append(np.degrees(np.arctan2(-d[:, 1], d[:, 0])) % 180); Ls.append(np.hypot(d[:, 0], d[:, 1]) / 4)
    Ws.append(ndimage.map_coordinates(W, [c[:, 1], c[:, 0]], order=1)); Ws[-1] = np.where((c[:, 0] >= m) & (c[:, 0] < n - m) & (c[:, 1] >= m) & (c[:, 1] < n - m), Ws[-1], np.nan)
H = np.concatenate(H); Ls = np.concatenate(Ls); Ws = np.concatenate(Ws)
ok = np.isfinite(Ws); H, Ls, Ws = H[ok], Ls[ok], Ws[ok]
tot = Ls.sum()
print(f'curve length in the box {tot:.0f} L cells; land cells: in a trough (W < 0) {100 * np.mean(lw < 0):.1f} %, in a deep trough (W < p25 = {p25:.2f} m) 25.0 %')
for a in (0, 45, 90, 135):
    off = np.abs((H - a + 90) % 180 - 90)
    s = off <= 5
    print(f'  within 5 deg of {a:3d} deg: {100 * Ls[s].sum() / tot:5.1f} % of curve length; of it in a trough {100 * Ls[s & (Ws < 0)].sum() / max(Ls[s].sum(), 1e-9):5.1f} %, '
          f'in a deep trough {100 * Ls[s & (Ws < p25)].sum() / max(Ls[s].sum(), 1e-9):5.1f} %')
rest = np.ones(len(H), bool)
for a in (0, 45, 90, 135):
    rest &= np.abs((H - a + 90) % 180 - 90) > 5
print(f'  other headings: {100 * Ls[rest].sum() / tot:5.1f} %; in a trough {100 * Ls[rest & (Ws < 0)].sum() / Ls[rest].sum():5.1f} %, in a deep trough {100 * Ls[rest & (Ws < p25)].sum() / Ls[rest].sum():5.1f} %')
print(f'  all curve length: in a trough {100 * Ls[Ws < 0].sum() / tot:5.1f} %, in a deep trough {100 * Ls[Ws < p25].sum() / tot:5.1f} %')
