"""Lead #184 (sinuosity item): a meander's lateral amplitude against the valley floor's width. The amplitude of a curve point is its distance to the same reach's curve
without meanders (the two share base path and smoothing); the floor's width is the gate's width10 (PLANET_WIDTH_DUMP: the valley's width 10 m above the river cell) at
the nearest river cell within 3 L cells. Reported over the meandering points (amplitude over 0.25 L cells) inside the box: amplitude and width10 percentiles, and the
share of points whose peak-to-peak swing (2 x amplitude) is under the floor's width. Usage: amp_width.py <curves with meanders> <curves without> <width csv>"""
import sys, struct, numpy as np
from scipy import spatial
n, m, cell = 8160, 1600, 101.802


def read(p):
    b = open(p, 'rb').read(); assert b[:4] == b'TRC1'
    k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; out = {}
    for _ in range(cnt):
        r, npt = struct.unpack_from('<II', b, k); k += 8
        out[r] = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
        nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    return out


wm, w0 = read(sys.argv[1]), read(sys.argv[2])
amp, pts = [], []
for r, p in wm.items():
    if r not in w0 or len(p) < 3:
        continue
    d, _ = spatial.cKDTree(w0[r]).query(p)
    amp.append(d); pts.append(p)
amp = np.concatenate(amp); pts = np.concatenate(pts)
inb = (pts[:, 0] >= m) & (pts[:, 0] < n - m) & (pts[:, 1] >= m) & (pts[:, 1] < n - m)
wav = inb & (amp > 0.25)
a = np.genfromtxt(sys.argv[3], delimiter=',', skip_header=1)
cxy = np.column_stack([a[:, 0] % n, a[:, 0] // n])
dist, idx = spatial.cKDTree(cxy).query(pts[wav], distance_upper_bound=3.0)
ok = np.isfinite(dist)
A = amp[wav][ok] * cell; Wd = a[idx[ok], 2]
q = lambda x: ' / '.join(f'{np.percentile(x, p):.0f}' for p in (10, 50, 90))
print(f'{wav.sum()} meandering points in the box ({100 * wav.sum() / inb.sum():.1f} % of the curve points there); {ok.sum()} with a river cell within 3 L cells')
print(f'  amplitude (m) p10 / p50 / p90: {q(A)}; width10 (m) there: {q(Wd)}')
print(f'  swing (2 x amplitude) under the floor width10: {100 * np.mean(2 * A < Wd):.1f} %; amplitude under 1 L cell: {100 * np.mean(A < cell):.1f} %')
